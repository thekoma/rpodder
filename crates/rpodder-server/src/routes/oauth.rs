//! OAuth2/OIDC authentication routes.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{AppendHeaders, IntoResponse, Redirect, Response},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use uuid::Uuid;

use rpodder_core::repo::{SessionRepo, UserRepo};
use rpodder_core::types::Session;

use crate::state::AppState;
use rpodder_db::{Db, postgres::PgRepo, sqlite::SqliteRepo};

macro_rules! with_repo {
    ($state:expr, |$repo:ident| $body:expr) => {
        match &*$state.db {
            Db::Postgres(pool) => {
                let $repo = PgRepo::new(pool.clone());
                $body
            }
            Db::Sqlite(pool) => {
                let $repo = SqliteRepo::new(pool.clone());
                $body
            }
        }
    };
}

/// Cookie binding an SSO login to the browser that started it: `sso_login`
/// stores the `state` it sends to the provider, `sso_callback` requires the
/// provider to echo the same value back.
const SSO_STATE_COOKIE: &str = "rpodder_sso_state";

fn sso_state_cookie(value: &str, max_age: u32) -> String {
    format!("{SSO_STATE_COOKIE}={value}; Path=/auth/sso; HttpOnly; SameSite=Lax; Max-Age={max_age}")
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| c.trim().strip_prefix(name)?.strip_prefix('='))
        .map(str::to_string)
}

/// Whether the request reached the host of `base_url`. Unknown (no Host
/// header, unparsable URL) counts as yes.
fn on_base_url_host(headers: &HeaderMap, base_url: &str) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return true;
    };
    let Ok(url) = reqwest::Url::parse(base_url) else {
        return true;
    };
    let Some(base_host) = url.host_str() else {
        return true;
    };
    let base = match url.port() {
        Some(port) => format!("{base_host}:{port}"),
        None => base_host.to_string(),
    };
    host.eq_ignore_ascii_case(&base)
}

#[derive(Deserialize)]
pub struct LoginParams {
    pub canonical: Option<String>,
}

/// GET /auth/sso/login — redirect to OIDC provider
pub async fn sso_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<LoginParams>,
) -> Result<Response, StatusCode> {
    let config = &state.config;
    if !config.oauth_configured() {
        return Err(StatusCode::NOT_FOUND);
    }

    // The state cookie lands on the host serving this request, but the
    // provider sends the browser back to `{base_url}/auth/sso/callback`.
    // Start over on that host, or the callback arrives without the cookie.
    // `canonical` stops a redirect loop behind a proxy that rewrites Host.
    if !config.base_url.is_empty()
        && params.canonical.is_none()
        && !on_base_url_host(&headers, &config.base_url)
    {
        let login = format!(
            "{}/auth/sso/login?canonical=1",
            config.base_url.trim_end_matches('/')
        );
        return Ok(Redirect::temporary(&login).into_response());
    }

    let base_url = if config.base_url.is_empty() {
        format!("http://{}:{}", config.host, config.port)
    } else {
        config.base_url.clone()
    };

    let redirect_uri = format!("{base_url}/auth/sso/callback");

    // Build authorization URL manually (simpler than full OIDC discovery for now)
    // For generic OIDC, discover the authorization endpoint
    let http_client = reqwest::Client::new();
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.oauth_issuer_url.trim_end_matches('/')
    );

    let discovery: serde_json::Value = http_client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "OIDC discovery failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .json()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_endpoint = discovery["authorization_endpoint"]
        .as_str()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let state_param = crate::routes::auth::generate_token();

    // Request groups scope if admin group mapping is configured
    let scope = if config.oauth_admin_group.is_empty() {
        "openid+profile+email".to_string()
    } else {
        "openid+profile+email+groups".to_string()
    };

    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={scope}&state={state_param}",
        auth_endpoint,
        urlencoding::encode(&config.oauth_client_id),
        urlencoding::encode(&redirect_uri),
    );

    Ok((
        [(header::SET_COOKIE, sso_state_cookie(&state_param, 600))],
        Redirect::temporary(&auth_url),
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct CallbackParams {
    /// Absent when the provider reports an `error` (e.g. the user denied access).
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// GET /auth/sso/callback — handle OIDC callback, create/login user
pub async fn sso_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Response {
    if !state.config.oauth_configured() {
        return StatusCode::NOT_FOUND.into_response();
    }

    // A callback this browser did not start (no or another state cookie) is
    // refused: otherwise a crafted link logs the victim into the attacker's
    // account (login CSRF). The state is single-use, so it is cleared on
    // every outcome.
    let clear_state = AppendHeaders([(header::SET_COOKIE, sso_state_cookie("", 0))]);
    let expected = cookie_value(&headers, SSO_STATE_COOKIE);
    let state_matches = matches!(
        (&expected, &params.state),
        (Some(expected), Some(got)) if !expected.is_empty() && expected == got
    );
    if !state_matches {
        tracing::warn!("SSO callback with missing or mismatched state");
        return (StatusCode::BAD_REQUEST, clear_state).into_response();
    }

    let Some(code) = params.code.as_deref() else {
        tracing::warn!(error = ?params.error, "SSO provider returned no code");
        return (StatusCode::UNAUTHORIZED, clear_state).into_response();
    };

    match complete_sso_login(&state, code).await {
        Ok(resp) => (clear_state, resp).into_response(),
        Err(status) => (status, clear_state).into_response(),
    }
}

/// Exchange the authorization code, find or create the user and open a session.
async fn complete_sso_login(state: &AppState, code: &str) -> Result<Response, StatusCode> {
    let config = &state.config;

    let base_url = if config.base_url.is_empty() {
        format!("http://{}:{}", config.host, config.port)
    } else {
        config.base_url.clone()
    };
    let redirect_uri = format!("{base_url}/auth/sso/callback");

    let http_client = reqwest::Client::new();

    // Discover token endpoint
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.oauth_issuer_url.trim_end_matches('/')
    );
    let discovery: serde_json::Value = http_client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .json()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let token_endpoint = discovery["token_endpoint"]
        .as_str()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let userinfo_endpoint = discovery["userinfo_endpoint"].as_str();

    // Exchange code for token
    let token_resp: serde_json::Value = http_client
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &redirect_uri),
            ("client_id", &config.oauth_client_id),
            ("client_secret", &config.oauth_client_secret),
        ])
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "token exchange failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .json()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let access_token = token_resp["access_token"].as_str().ok_or_else(|| {
        tracing::error!(response = %token_resp, "no access_token in response");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get user info
    let (username, email, groups) = if let Some(userinfo_url) = userinfo_endpoint {
        let userinfo: serde_json::Value = http_client
            .get(userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .json()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let username = userinfo["preferred_username"]
            .as_str()
            .or_else(|| userinfo["name"].as_str())
            .or_else(|| userinfo["sub"].as_str())
            .unwrap_or("sso-user")
            .to_string();

        let email = userinfo["email"].as_str().map(|s| s.to_string());
        let groups = extract_groups(&userinfo);

        (username, email, groups)
    } else {
        // Decode ID token JWT payload (base64 middle part)
        if let Some(id_token) = token_resp["id_token"].as_str() {
            let parts: Vec<&str> = id_token.split('.').collect();
            if parts.len() >= 2 {
                use base64::Engine;
                if let Ok(payload) =
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1])
                {
                    if let Ok(claims) = serde_json::from_slice::<serde_json::Value>(&payload) {
                        let username = claims["preferred_username"]
                            .as_str()
                            .or_else(|| claims["sub"].as_str())
                            .unwrap_or("sso-user")
                            .to_string();
                        let email = claims["email"].as_str().map(|s| s.to_string());
                        let groups = extract_groups(&claims);
                        (username, email, groups)
                    } else {
                        ("sso-user".to_string(), None, vec![])
                    }
                } else {
                    ("sso-user".to_string(), None, vec![])
                }
            } else {
                ("sso-user".to_string(), None, vec![])
            }
        } else {
            ("sso-user".to_string(), None, vec![])
        }
    };

    // Determine if user should be admin based on group membership
    let should_be_admin = !config.oauth_admin_group.is_empty()
        && groups.iter().any(|g| g == &config.oauth_admin_group);

    tracing::info!(username = %username, email = ?email, groups = ?groups, is_admin = should_be_admin, "SSO login");

    // Find or create user
    let user = with_repo!(state, |repo| {
        UserRepo::find_by_username(&repo, &username).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = match user {
        Some(u) => u,
        None => {
            // Auto-create user from SSO
            let hash =
                crate::middleware::auth::hash_password(&crate::routes::auth::generate_token())
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            with_repo!(state, |repo| {
                UserRepo::create(&repo, &username, &hash, email.as_deref()).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
    };

    // Update admin status based on SSO group membership (on every login)
    if !config.oauth_admin_group.is_empty() && user.is_admin != should_be_admin {
        let _ = with_repo!(state, |repo| {
            UserRepo::set_admin(&repo, user.id, should_be_admin).await
        });
    }

    if !user.is_active {
        return Err(StatusCode::FORBIDDEN);
    }

    // Create session
    let token = format!(
        "{}{}",
        crate::middleware::auth::SSO_SESSION_PREFIX,
        crate::routes::auth::generate_token()
    );
    let duration_days = config.session_duration_days;
    let session = Session {
        id: Uuid::now_v7(),
        user_id: user.id,
        token: token.clone(),
        expires_at: Utc::now() + Duration::days(i64::from(duration_days)),
        created_at: Utc::now(),
    };

    with_repo!(state, |repo| SessionRepo::create(&repo, &session).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Set session cookie and also store username in localStorage via a redirect page
    let max_age = u64::from(duration_days) * 24 * 3600;
    let cookie = format!("sessionid={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}");

    // Determine actual admin status (may have been updated above)
    let is_admin_now = if !config.oauth_admin_group.is_empty() {
        should_be_admin
    } else {
        user.is_admin
    };

    Ok((
        [(header::SET_COOKIE, cookie)],
        axum::response::Html(sso_redirect_html(&username, is_admin_now)),
    )
        .into_response())
}

/// Small HTML page that stores the username in localStorage before going to /.
/// The username comes from the IdP, so it is embedded as a JSON string literal
/// with `<`, `>` and `&` escaped: it can neither close the string nor the
/// `<script>` element.
fn sso_redirect_html(username: &str, is_admin: bool) -> String {
    let username = serde_json::to_string(username)
        .unwrap_or_else(|_| "\"\"".to_string())
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    format!(
        r#"<html><script>localStorage.setItem('rpodder_user',{username});localStorage.setItem('rpodder_admin','{is_admin}');window.location='/';</script></html>"#
    )
}

/// GET /auth/sso/info — check if SSO is configured (public, for UI)
pub async fn sso_info(State(state): State<AppState>) -> impl IntoResponse {
    let config = &state.config;
    axum::Json(serde_json::json!({
        "enabled": config.oauth_configured(),
        "provider_name": config.oauth_provider_name,
        "registration": config.registration,
        "podcastindex": config.podcastindex_configured(),
    }))
}

/// Extract groups from OIDC claims (supports "groups" as array of strings)
fn extract_groups(claims: &serde_json::Value) -> Vec<String> {
    claims["groups"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::sso_redirect_html;

    #[test]
    fn sso_redirect_html_escapes_username() {
        let html = sso_redirect_html("x');alert(1);//</script><script>alert(2)", false);
        assert!(
            html.contains(r#"setItem('rpodder_user',"x');alert(1);//\u003c/script\u003e\u003cscript\u003ealert(2)")"#),
            "{html}"
        );
        assert_eq!(html.matches("<script>").count(), 1, "{html}");
        assert_eq!(html.matches("</script>").count(), 1, "{html}");
    }

    #[test]
    fn sso_redirect_html_plain_username() {
        let html = sso_redirect_html("alice", true);
        assert!(
            html.contains(r#"setItem('rpodder_user',"alice")"#),
            "{html}"
        );
        assert!(html.contains("setItem('rpodder_admin','true')"), "{html}");
    }
}
