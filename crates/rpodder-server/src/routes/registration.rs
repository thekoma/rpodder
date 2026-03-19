//! Public registration endpoint with mode control.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use crate::routes::admin::CreateUserRequest;
use crate::state::AppState;

/// POST /api/2/register — public registration endpoint
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let config = &state.config;

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

    crate::routes::admin::create_user(State(state), Json(body)).await
}
