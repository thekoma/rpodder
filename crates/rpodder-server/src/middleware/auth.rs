use axum::{
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use rpodder_core::repo::{SessionRepo, UserRepo};
use rpodder_core::types::User;

use crate::state::AppState;
use rpodder_db::{Db, postgres::PgRepo, sqlite::SqliteRepo};

/// Key used to store the authenticated user in request extensions.
#[derive(Clone)]
pub struct AuthUser(pub User);

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
    let session_token = extract_session_token(&req);
    let basic_auth = extract_basic_auth(&req);

    // 1. Try session cookie first
    if let Some(token) = &session_token
        && let Some(user) = resolve_session(&state, token).await
    {
        req.extensions_mut().insert(AuthUser(user));
        return next.run(req).await;
    }

    // 2. Try HTTP Basic Auth
    if let Some((username, password)) = &basic_auth
        && let Some(user) = resolve_basic_auth(&state, username, password).await
    {
        req.extensions_mut().insert(AuthUser(user));
        return next.run(req).await;
    }

    StatusCode::UNAUTHORIZED.into_response()
}

fn extract_session_token(req: &Request) -> Option<String> {
    let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    cookie_header
        .split(';')
        .filter_map(|c| c.trim().strip_prefix("sessionid="))
        .next()
        .map(|s| s.to_string())
}

fn extract_basic_auth(req: &Request) -> Option<(String, String)> {
    let auth_header = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let encoded = auth_header.strip_prefix("Basic ")?;
    let decoded = String::from_utf8(base64_decode(encoded)?).ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some((username.to_string(), password.to_string()))
}

async fn resolve_session(state: &AppState, token: &str) -> Option<User> {
    let session = match &*state.db {
        Db::Postgres(pool) => {
            let repo = PgRepo::new(pool.clone());
            SessionRepo::find_by_token(&repo, token).await.ok()?
        }
        Db::Sqlite(pool) => {
            let repo = SqliteRepo::new(pool.clone());
            SessionRepo::find_by_token(&repo, token).await.ok()?
        }
    }?;

    match &*state.db {
        Db::Postgres(pool) => {
            let repo = PgRepo::new(pool.clone());
            UserRepo::find_by_id(&repo, session.user_id).await.ok()?
        }
        Db::Sqlite(pool) => {
            let repo = SqliteRepo::new(pool.clone());
            UserRepo::find_by_id(&repo, session.user_id).await.ok()?
        }
    }
}

async fn resolve_basic_auth(state: &AppState, username: &str, password: &str) -> Option<User> {
    let user = match &*state.db {
        Db::Postgres(pool) => {
            let repo = PgRepo::new(pool.clone());
            UserRepo::find_by_username(&repo, username).await.ok()?
        }
        Db::Sqlite(pool) => {
            let repo = SqliteRepo::new(pool.clone());
            UserRepo::find_by_username(&repo, username).await.ok()?
        }
    }?;

    if !user.is_active {
        return None;
    }

    if verify_password(password, &user.password_hash) {
        Some(user)
    } else {
        None
    }
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
    use argon2::password_hash::SaltString;
    use argon2::password_hash::rand_core::OsRng;
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(input).ok()
}
