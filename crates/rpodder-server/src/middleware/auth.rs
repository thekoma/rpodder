use axum::{
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use chrono::Utc;
use uuid::Uuid;

use rpodder_core::repo::{AppPasswordRepo, SessionRepo, UserRepo};
use rpodder_core::types::User;

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

/// Prefixes of the one-time tokens (account activation, password reset) that
/// share the `sessions` table with login sessions. They never authenticate a
/// request: the activation and reset endpoints look them up themselves.
pub const ACTIVATE_TOKEN_PREFIX: &str = "activate-";
pub const RESET_TOKEN_PREFIX: &str = "reset-";
/// Prefix of login sessions created by the SSO callback.
pub const SSO_SESSION_PREFIX: &str = "sso-";
/// Prefix of login sessions created with an app password, followed by the
/// app password's id and `-`. Revoking the app password deletes them.
pub const APP_SESSION_PREFIX: &str = "app-";

fn is_one_time_token(token: &str) -> bool {
    token.starts_with(ACTIVATE_TOKEN_PREFIX) || token.starts_with(RESET_TOKEN_PREFIX)
}

/// Key used to store the authenticated user in request extensions.
#[derive(Clone)]
pub struct AuthUser(pub User);

/// How the request authenticated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthMethod {
    Password,
    AppPassword(Uuid),
    Session,
    SsoSession,
    AppSession(Uuid),
}

impl AuthMethod {
    /// Authenticated by an app password, directly or through a session created with one.
    pub fn is_app(self) -> bool {
        self.app_password_id().is_some()
    }

    pub fn app_password_id(self) -> Option<Uuid> {
        match self {
            AuthMethod::AppPassword(id) | AuthMethod::AppSession(id) => Some(id),
            _ => None,
        }
    }
}

/// Stored next to `AuthUser` in request extensions.
#[derive(Clone, Copy)]
pub struct AuthVia(pub AuthMethod);

/// SHA-256 of a random token, lowercase hex, as stored for app passwords and login flows.
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether `secret` has the shape of a token from `routes::auth::generate_token`.
fn looks_like_token(secret: &str) -> bool {
    secret.len() == 64
        && secret
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

/// Creates an auth middleware closure that captures the AppState.
pub fn require_auth_layer(
    state: AppState,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
+ Clone
+ Send {
    move |req, next| {
        let state = state.clone();
        Box::pin(require_auth(state, req, next))
    }
}

async fn require_auth(state: AppState, mut req: Request, next: Next) -> Response {
    // Extract header values upfront (before any async work) since Request is not Sync
    let headers = req.headers().clone();
    match authenticate(&state, &headers).await {
        Some((user, method)) => {
            req.extensions_mut().insert(AuthUser(user));
            req.extensions_mut().insert(AuthVia(method));
            next.run(req).await
        }
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// The user a request authenticates as (session cookie, then HTTP Basic), if
/// any. For public routes that do more for a logged-in caller.
pub async fn authenticate(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Option<(User, AuthMethod)> {
    // 1. Try session cookie first
    if let Some(token) = session_token(headers)
        && let Some(found) = resolve_session(state, &token).await
    {
        return Some(found);
    }

    // 2. Try HTTP Basic Auth
    if let Some((username, password)) = extract_basic_auth(headers) {
        return resolve_basic_auth(state, &username, &password).await;
    }

    None
}

/// The `sessionid` cookie of the request, if any.
pub fn session_token(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    cookie_header
        .split(';')
        .filter_map(|c| c.trim().strip_prefix("sessionid="))
        .next()
        .map(|s| s.to_string())
}

fn extract_basic_auth(headers: &axum::http::HeaderMap) -> Option<(String, String)> {
    let auth_header = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let encoded = auth_header.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(base64_decode(encoded)?).ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some((username.to_string(), password.to_string()))
}

/// The app password id in an `app-{id}-…` session token.
fn app_session_id(token: &str) -> Option<Uuid> {
    let rest = token.strip_prefix(APP_SESSION_PREFIX)?;
    rest.get(..36)?.parse().ok()
}

async fn resolve_session(state: &AppState, token: &str) -> Option<(User, AuthMethod)> {
    if is_one_time_token(token) {
        return None;
    }

    let session = with_repo!(state, |repo| SessionRepo::find_by_token(&repo, token)
        .await
        .ok()?)?;

    let method = if token.starts_with(SSO_SESSION_PREFIX) {
        AuthMethod::SsoSession
    } else if token.starts_with(APP_SESSION_PREFIX) {
        // Checked on every request. A login verified with an app password can
        // insert its session just after that password was revoked.
        let id = app_session_id(token)?;
        let exists = with_repo!(state, |repo| {
            AppPasswordRepo::exists_for_user(&repo, id, session.user_id)
                .await
                .ok()?
        });
        if !exists {
            return None;
        }
        AuthMethod::AppSession(id)
    } else {
        AuthMethod::Session
    };

    let user = with_repo!(state, |repo| UserRepo::find_by_id(&repo, session.user_id)
        .await
        .ok()?)?;

    user.is_active.then_some((user, method))
}

async fn resolve_basic_auth(
    state: &AppState,
    username: &str,
    password: &str,
) -> Option<(User, AuthMethod)> {
    let user = with_repo!(state, |repo| UserRepo::find_by_username(&repo, username)
        .await
        .ok()?)?;

    if !user.is_active {
        return None;
    }

    if looks_like_token(password) {
        let hash = hash_token(password);
        let app_password = with_repo!(state, |repo| {
            AppPasswordRepo::find_by_token_hash(&repo, &hash)
                .await
                .ok()?
        });
        if let Some(app_password) = app_password.filter(|p| p.user_id == user.id) {
            let touched = with_repo!(state, |repo| {
                AppPasswordRepo::touch(&repo, app_password.id, Utc::now()).await
            });
            if let Err(e) = touched {
                tracing::debug!(error = %e, "recording app password use failed");
            }
            return Some((user, AuthMethod::AppPassword(app_password.id)));
        }
    }

    if verify_password(password, &user.password_hash) {
        Some((user, AuthMethod::Password))
    } else {
        None
    }
}
pub fn verify_password_pub(password: &str, hash: &str) -> bool {
    verify_password(password, hash)
}

fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn hash_password(password: &str) -> std::result::Result<String, argon2::password_hash::Error> {
    use argon2::{Argon2, PasswordHash, PasswordHasher};
    // Generates a random 16-byte salt.
    let hash: PasswordHash = Argon2::default().hash_password(password.as_bytes())?;
    Ok(hash.to_string())
}

/// Middleware that requires the authenticated user to be an admin.
/// Must be applied after require_auth_layer. Refuses app-password callers.
pub fn require_admin_layer()
-> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
+ Clone
+ Send {
    move |req, next| Box::pin(require_admin(req, next))
}

async fn require_admin(req: Request, next: Next) -> Response {
    let app = req
        .extensions()
        .get::<AuthVia>()
        .is_some_and(|via| via.0.is_app());
    if let Some(auth_user) = req.extensions().get::<AuthUser>()
        && auth_user.0.is_admin
        && !app
    {
        return next.run(req).await;
    }
    StatusCode::FORBIDDEN.into_response()
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(input).ok()
}
