use axum::{
    Extension,
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use rpodder_core::repo::SettingsRepo;
use rpodder_core::types::{SettingsScope, UserSettings};

use crate::middleware::auth::AuthUser;
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

fn strip_json(s: &str) -> &str {
    s.strip_suffix(".json").unwrap_or(s)
}

fn parse_scope(s: &str) -> Option<SettingsScope> {
    match s {
        "account" => Some(SettingsScope::Account),
        "device" => Some(SettingsScope::Device),
        "podcast" => Some(SettingsScope::Podcast),
        "episode" => Some(SettingsScope::Episode),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct SettingsUpdateRequest {
    pub set: Option<serde_json::Map<String, serde_json::Value>>,
    pub remove: Option<Vec<String>>,
}

/// GET /api/2/settings/{username}/{scope}.json
pub async fn get_settings(
    State(state): State<AppState>,
    Path((username, scope_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let scope_str = strip_json(&scope_json);
    let scope = parse_scope(scope_str).ok_or(StatusCode::BAD_REQUEST)?;

    let settings = with_repo!(state, |repo| {
        SettingsRepo::get(&repo, auth_user.0.id, scope, None).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match settings {
        Some(s) => Ok(Json(s.settings)),
        None => Ok(Json(serde_json::json!({}))),
    }
}

/// POST /api/2/settings/{username}/{scope}.json
pub async fn update_settings(
    State(state): State<AppState>,
    Path((username, scope_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<SettingsUpdateRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let scope_str = strip_json(&scope_json);
    let scope = parse_scope(scope_str).ok_or(StatusCode::BAD_REQUEST)?;

    // Get existing settings or create empty
    let existing = with_repo!(state, |repo| {
        SettingsRepo::get(&repo, auth_user.0.id, scope, None).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut current = existing
        .as_ref()
        .and_then(|s| s.settings.as_object().cloned())
        .unwrap_or_default();

    // Apply "set" operations
    if let Some(set) = body.set {
        for (key, value) in set {
            current.insert(key, value);
        }
    }

    // Apply "remove" operations
    if let Some(remove) = body.remove {
        for key in remove {
            current.remove(&key);
        }
    }

    let settings = UserSettings {
        id: existing.map(|s| s.id).unwrap_or_else(Uuid::now_v7),
        user_id: auth_user.0.id,
        scope,
        scope_id: None,
        settings: serde_json::Value::Object(current.clone()),
        updated_at: Utc::now(),
    };

    with_repo!(state, |repo| SettingsRepo::save(&repo, &settings).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::Value::Object(current)))
}
