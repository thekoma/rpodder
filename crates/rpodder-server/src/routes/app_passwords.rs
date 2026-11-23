//! App passwords and the browser side of the Nextcloud Login Flow v2
//! (`/api/2/me/app-passwords`, `/api/2/me/login-flows/*`).

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rpodder_core::repo::AppPasswordRepo;
use rpodder_core::types::AppPassword;

use super::admin::app_password_refused;
use crate::middleware::auth::{AuthMethod, AuthUser, AuthVia, hash_token, verify_password_pub};
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

const MAX_NAME_LEN: usize = 100;

#[derive(Debug, Serialize)]
pub struct AppPasswordInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl From<AppPassword> for AppPasswordInfo {
    fn from(p: AppPassword) -> Self {
        AppPasswordInfo {
            id: p.id,
            name: p.name,
            created_at: p.created_at,
            last_used_at: p.last_used_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub name: String,
    pub current_password: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreatedAppPassword {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub password: String,
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

// ---------------------------------------------------------------------------
// GET /api/2/me/app-passwords
// ---------------------------------------------------------------------------

pub async fn list(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Extension(AuthVia(method)): Extension<AuthVia>,
) -> Result<Response, StatusCode> {
    if method.is_app() {
        return Ok(app_password_refused().into_response());
    }
    let passwords = with_repo!(state, |repo| {
        AppPasswordRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let info: Vec<AppPasswordInfo> = passwords.into_iter().map(Into::into).collect();
    Ok(Json(info).into_response())
}

// ---------------------------------------------------------------------------
// POST /api/2/me/app-passwords
// ---------------------------------------------------------------------------

pub async fn create(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Extension(AuthVia(method)): Extension<AuthVia>,
    Json(body): Json<CreateRequest>,
) -> Result<Response, StatusCode> {
    if method.is_app() {
        return Ok(app_password_refused().into_response());
    }

    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_LEN {
        return Ok(error(
            StatusCode::BAD_REQUEST,
            "name must be 1 to 100 characters",
        ));
    }

    // Like `change_password`, a stolen session cookie must not be enough to
    // mint a credential. Only an SSO login skips it, its password is random.
    if method != AuthMethod::SsoSession {
        match &body.current_password {
            None => {
                return Ok(error(StatusCode::FORBIDDEN, "current password required"));
            }
            Some(pw) if !verify_password_pub(pw, &auth_user.0.password_hash) => {
                return Ok(error(StatusCode::FORBIDDEN, "wrong password"));
            }
            Some(_) => {}
        }
    }

    let password = super::auth::generate_token();
    let app_password = AppPassword {
        id: Uuid::now_v7(),
        user_id: auth_user.0.id,
        name: name.to_string(),
        token_hash: hash_token(&password),
        created_at: Utc::now(),
        last_used_at: None,
    };
    with_repo!(state, |repo| AppPasswordRepo::create(&repo, &app_password)
        .await)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedAppPassword {
            id: app_password.id,
            name: app_password.name,
            created_at: app_password.created_at,
            password,
        }),
    )
        .into_response())
}

// ---------------------------------------------------------------------------
// DELETE /api/2/me/app-passwords/{id}
// ---------------------------------------------------------------------------

pub async fn delete(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Extension(AuthVia(method)): Extension<AuthVia>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    if method.is_app() {
        return Ok(app_password_refused().into_response());
    }
    let id: Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let deleted = with_repo!(state, |repo| {
        AppPasswordRepo::delete(&repo, auth_user.0.id, id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if deleted {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// ---------------------------------------------------------------------------
// POST /api/2/me/login-flows/lookup and /answer
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct FlowLookupRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct FlowAnswerRequest {
    pub token: String,
    pub approve: bool,
}

#[derive(Debug, Serialize)]
pub struct FlowInfo {
    pub client_name: String,
    pub created_at: DateTime<Utc>,
    /// Granted by the caller. A flow granted by someone else is not shown.
    pub approved: bool,
    pub username: String,
}

pub async fn flow_lookup(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Extension(AuthVia(method)): Extension<AuthVia>,
    Json(body): Json<FlowLookupRequest>,
) -> Result<Response, StatusCode> {
    if method.is_app() {
        return Ok(app_password_refused().into_response());
    }
    let hash = hash_token(&body.token);
    let flow = with_repo!(state, |repo| {
        AppPasswordRepo::find_flow_by_login_hash(&repo, &hash, Utc::now()).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let approved = match flow.user_id {
        None => false,
        Some(owner) if owner == auth_user.0.id => true,
        Some(_) => return Err(StatusCode::NOT_FOUND),
    };
    Ok(Json(FlowInfo {
        client_name: flow.client_name,
        created_at: flow.created_at,
        approved,
        username: auth_user.0.username,
    })
    .into_response())
}

pub async fn flow_answer(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Extension(AuthVia(method)): Extension<AuthVia>,
    Json(body): Json<FlowAnswerRequest>,
) -> Result<Response, StatusCode> {
    if method.is_app() {
        return Ok(app_password_refused().into_response());
    }
    let hash = hash_token(&body.token);
    let user_id = auth_user.0.id;
    let now = Utc::now();

    if body.approve {
        let owner = with_repo!(state, |repo| {
            AppPasswordRepo::approve_flow(&repo, &hash, user_id, now).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        match owner {
            Some(owner) if owner == user_id => Ok(StatusCode::NO_CONTENT.into_response()),
            Some(_) => Err(StatusCode::CONFLICT),
            None => Err(StatusCode::NOT_FOUND),
        }
    } else {
        let denied = with_repo!(state, |repo| {
            AppPasswordRepo::deny_flow(&repo, &hash, user_id, now).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if denied {
            Ok(StatusCode::NO_CONTENT.into_response())
        } else {
            Err(StatusCode::NOT_FOUND)
        }
    }
}
