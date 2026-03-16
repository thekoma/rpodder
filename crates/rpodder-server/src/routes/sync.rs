use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{DeviceRepo, SyncGroupRepo};

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{postgres::PgRepo, sqlite::SqliteRepo, Db};

macro_rules! with_repo {
    ($state:expr, |$repo:ident| $body:expr) => {
        match &*$state.db {
            Db::Postgres(pool) => { let $repo = PgRepo::new(pool.clone()); $body }
            Db::Sqlite(pool) => { let $repo = SqliteRepo::new(pool.clone()); $body }
        }
    };
}

fn strip_json(s: &str) -> &str {
    s.strip_suffix(".json").unwrap_or(s)
}

#[derive(Serialize)]
pub struct SyncStatusResponse {
    pub synchronized: Vec<Vec<String>>,
    #[serde(rename = "not-synchronized")]
    pub not_synchronized: Vec<String>,
}

#[derive(Deserialize)]
pub struct SyncUpdateRequest {
    pub synchronize: Vec<Vec<String>>,
    #[serde(rename = "stop-synchronize")]
    pub stop_synchronize: Option<Vec<String>>,
}

/// GET /api/2/sync-devices/{username}.json
pub async fn get_sync_status(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let groups = with_repo!(state, |repo| {
        SyncGroupRepo::get_groups_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let all_devices = with_repo!(state, |repo| {
        DeviceRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut synchronized: Vec<Vec<String>> = Vec::new();
    let mut synced_device_ids = std::collections::HashSet::new();

    for (_, devices) in &groups {
        if devices.len() > 1 {
            let group_devices: Vec<String> = devices.iter().map(|d| d.device_id.clone()).collect();
            for d in devices {
                synced_device_ids.insert(d.device_id.clone());
            }
            synchronized.push(group_devices);
        }
    }

    let not_synchronized: Vec<String> = all_devices
        .iter()
        .filter(|d| !synced_device_ids.contains(&d.device_id))
        .map(|d| d.device_id.clone())
        .collect();

    Ok(Json(SyncStatusResponse {
        synchronized,
        not_synchronized,
    }))
}

/// POST /api/2/sync-devices/{username}.json
pub async fn update_sync_status(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<SyncUpdateRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    // Stop synchronizing devices
    if let Some(stop) = &body.stop_synchronize {
        for device_uid in stop {
            let device = with_repo!(state, |repo| {
                DeviceRepo::find_by_uid(&repo, auth_user.0.id, device_uid).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if let Some(device) = device {
                with_repo!(state, |repo| {
                    SyncGroupRepo::remove_device_from_group(&repo, device.id).await
                })
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
    }

    // Create sync groups
    for group_uids in &body.synchronize {
        let mut device_ids = Vec::new();
        for uid in group_uids {
            let device = with_repo!(state, |repo| {
                DeviceRepo::find_by_uid(&repo, auth_user.0.id, uid).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if let Some(d) = device {
                device_ids.push(d.id);
            }
        }

        if device_ids.len() > 1 {
            with_repo!(state, |repo| {
                SyncGroupRepo::create_group(&repo, auth_user.0.id, &device_ids).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    Ok(StatusCode::OK)
}
