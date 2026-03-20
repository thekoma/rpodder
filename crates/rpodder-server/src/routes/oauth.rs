//! OAuth2/OIDC authentication routes.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
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

/// GET /auth/sso/login — redirect to OIDC provider
pub async fn sso_login(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    let config = &state.config;
    if !config.oauth_configured() {
        return Err(StatusCode::NOT_FOUND);
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

    let state_param = Uuid::now_v7().to_string();

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

    Ok(Redirect::temporary(&auth_url))
}

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: String,
    #[allow(dead_code)]
    pub state: Option<String>,
}

/// GET /auth/sso/callback — handle OIDC callback, create/login user
pub async fn sso_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse, StatusCode> {
    let config = &state.config;
    if !config.oauth_configured() {
        return Err(StatusCode::NOT_FOUND);
    }

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
            ("code", &params.code),
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
            let hash = crate::middleware::auth::hash_password(&Uuid::now_v7().to_string())
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
    let token = format!("sso-{}", Uuid::now_v7());
    let session = Session {
        id: Uuid::now_v7(),
        user_id: user.id,
        token: token.clone(),
        expires_at: Utc::now() + Duration::hours(24),
        created_at: Utc::now(),
    };

    with_repo!(state, |repo| SessionRepo::create(&repo, &session).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Set session cookie and also store username in localStorage via a redirect page
    let cookie = format!("sessionid={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400");

    // Determine actual admin status (may have been updated above)
    let is_admin_now = if !config.oauth_admin_group.is_empty() {
        should_be_admin
    } else {
        user.is_admin
    };

    // Redirect with a small HTML page that sets localStorage before going to /
    let html = format!(
        r#"<html><script>localStorage.setItem('rpodder_user','{}');localStorage.setItem('rpodder_admin','{}');window.location='/';</script></html>"#,
        username, is_admin_now
    );

    Ok((
        [(axum::http::header::SET_COOKIE, cookie)],
        axum::response::Html(html),
    ))
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
