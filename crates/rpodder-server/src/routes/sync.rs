use axum::{
    Extension,
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{DeviceRepo, SubscriptionRepo, SyncGroupRepo};

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

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SyncGroupInfo {
    pub id: String,
    pub name: String,
    pub devices: Vec<String>,
}

#[derive(Serialize)]
pub struct SyncStatusResponse {
    /// gpodder compat: list of device-id groups
    pub synchronized: Vec<Vec<String>>,
    #[serde(rename = "not-synchronized")]
    pub not_synchronized: Vec<String>,
    /// Extended: named groups with IDs
    pub groups: Vec<SyncGroupInfo>,
}

#[derive(Deserialize)]
pub struct SyncUpdateRequest {
    pub synchronize: Vec<SyncGroupCreate>,
    #[serde(rename = "stop-synchronize")]
    pub stop_synchronize: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum SyncGroupCreate {
    /// Extended format: { "devices": [...], "name": "..." }
    Named {
        devices: Vec<String>,
        name: Option<String>,
    },
    /// gpodder compat: bare array of device IDs
    DeviceList(Vec<String>),
}

impl SyncGroupCreate {
    fn devices(&self) -> &[String] {
        match self {
            SyncGroupCreate::Named { devices, .. } => devices,
            SyncGroupCreate::DeviceList(ids) => ids,
        }
    }

    fn name(&self) -> &str {
        match self {
            SyncGroupCreate::Named { name: Some(n), .. } => n,
            _ => "",
        }
    }
}

#[derive(Deserialize)]
pub struct RenameRequest {
    pub name: String,
}

// ---------------------------------------------------------------------------
// GET /api/2/sync-devices/{username}.json
// ---------------------------------------------------------------------------

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
    let mut group_infos: Vec<SyncGroupInfo> = Vec::new();
    let mut synced_device_ids = std::collections::HashSet::new();

    for (group, devices) in &groups {
        if devices.len() > 1 {
            let device_ids: Vec<String> = devices.iter().map(|d| d.device_id.clone()).collect();
            for d in devices {
                synced_device_ids.insert(d.device_id.clone());
            }
            synchronized.push(device_ids.clone());
            group_infos.push(SyncGroupInfo {
                id: group.id.to_string(),
                name: group.name.clone(),
                devices: device_ids,
            });
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
        groups: group_infos,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/2/sync-devices/{username}.json
// ---------------------------------------------------------------------------

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
    for group_spec in &body.synchronize {
        let mut device_ids = Vec::new();
        for uid in group_spec.devices() {
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
                SyncGroupRepo::create_group(&repo, auth_user.0.id, &device_ids, group_spec.name())
                    .await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // Merge existing subscriptions across all devices in the group
            merge_subscriptions(&state, auth_user.0.id, &device_ids).await?;
        }
    }

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// POST /api/2/sync-group/{group_id}/rename
// ---------------------------------------------------------------------------

pub async fn rename_sync_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<RenameRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let group_uuid: uuid::Uuid = group_id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    with_repo!(state, |repo| {
        SyncGroupRepo::rename_group(&repo, group_uuid, auth_user.0.id, &body.name).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Merge subscriptions so all devices in a sync group share the same set.
async fn merge_subscriptions(
    state: &AppState,
    user_id: uuid::Uuid,
    device_ids: &[uuid::Uuid],
) -> Result<(), StatusCode> {
    let mut all_subs: std::collections::HashMap<uuid::Uuid, String> =
        std::collections::HashMap::new();

    for &device_id in device_ids {
        let subs = with_repo!(state, |repo| {
            SubscriptionRepo::list_for_device(&repo, user_id, device_id).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        for sub in subs {
            all_subs.entry(sub.podcast_id).or_insert(sub.ref_url);
        }
    }

    for &device_id in device_ids {
        let device_subs = with_repo!(state, |repo| {
            SubscriptionRepo::list_for_device(&repo, user_id, device_id).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let device_podcast_ids: std::collections::HashSet<uuid::Uuid> =
            device_subs.iter().map(|s| s.podcast_id).collect();

        for (podcast_id, ref_url) in &all_subs {
            if !device_podcast_ids.contains(podcast_id) {
                with_repo!(state, |repo| {
                    SubscriptionRepo::subscribe(&repo, user_id, device_id, *podcast_id, ref_url)
                        .await
                })
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                tracing::info!(
                    device_id = %device_id,
                    podcast_url = %ref_url,
                    "sync group merge: propagated subscription"
                );
            }
        }
    }

    Ok(())
}
