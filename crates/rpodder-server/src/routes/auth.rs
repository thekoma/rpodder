use axum::{
    Extension,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use rpodder_core::repo::SessionRepo;
use rpodder_core::types::Session;

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{Db, postgres::PgRepo, sqlite::SqliteRepo};

/// POST /api/2/auth/{username}/login.json
///
/// The actual authentication is done by the auth middleware (Basic Auth).
/// This handler just creates a session and returns the cookie.
pub async fn login(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Response, StatusCode> {
    // Verify the path username matches the authenticated user
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let duration_days = state.config.session_duration_days;
    let session = Session {
        id: Uuid::now_v7(),
        user_id: auth_user.0.id,
        token: generate_token(),
        expires_at: Utc::now() + Duration::days(i64::from(duration_days)),
        created_at: Utc::now(),
    };

    let create_result = match &*state.db {
        Db::Postgres(pool) => {
            let repo = PgRepo::new(pool.clone());
            SessionRepo::create(&repo, &session).await
        }
        Db::Sqlite(pool) => {
            let repo = SqliteRepo::new(pool.clone());
            SessionRepo::create(&repo, &session).await
        }
    };

    create_result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let max_age = u64::from(duration_days) * 24 * 3600;
    let cookie = format!(
        "sessionid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}",
        session.token,
    );

    Ok((StatusCode::OK, [(header::SET_COOKIE, cookie)], "").into_response())
}

/// POST /api/2/auth/{username}/logout.json
pub async fn logout(State(state): State<AppState>, req: Request) -> StatusCode {
    // Extract session token from cookie
    let token = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v: &header::HeaderValue| v.to_str().ok())
        .and_then(|cookies: &str| {
            cookies
                .split(';')
                .filter_map(|c: &str| c.trim().strip_prefix("sessionid="))
                .next()
                .map(|t: &str| t.to_string())
        });

    if let Some(token) = token {
        let _ = match &*state.db {
            Db::Postgres(pool) => {
                let repo = PgRepo::new(pool.clone());
                SessionRepo::delete(&repo, &token).await
            }
            Db::Sqlite(pool) => {
                let repo = SqliteRepo::new(pool.clone());
                SessionRepo::delete(&repo, &token).await
            }
        };
    }

    StatusCode::OK
}

fn generate_token() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..64)
        .map(|_| {
            let idx: u8 = rng.random_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect()
}
