//! Public registration endpoint with mode control.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use rpodder_core::repo::UserRepo;

use crate::routes::admin::CreateUserRequest;
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

/// POST /api/2/register — public registration endpoint
/// If no active users exist, always allows registration (first user becomes admin).
/// Otherwise, respects the registration mode setting.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let config = &state.config;

    // Check if any active users exist
    let active_count = with_repo!(state, |repo| UserRepo::count_active(&repo).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If users exist, enforce registration mode
    if active_count > 0 {
        if config.registration == "closed" {
            return Ok((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "Registration is closed" })),
            ));
        }

        if config.registration_invite() && body.email.is_none() {
            return Ok((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Email is required for account activation" })),
            ));
        }
    }

    crate::routes::admin::create_user(State(state), Json(body)).await
}
