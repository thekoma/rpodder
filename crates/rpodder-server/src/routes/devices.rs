use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rpodder_core::repo::DeviceRepo;
use rpodder_core::types::{Device, DeviceType};

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{postgres::PgRepo, sqlite::SqliteRepo, Db};

#[derive(Debug, Deserialize)]
pub struct DeviceUpdateRequest {
    pub caption: Option<String>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeviceResponse {
    pub id: String,
    pub caption: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub subscriptions: i64,
}

fn parse_device_type(s: &str) -> DeviceType {
    match s {
        "desktop" => DeviceType::Desktop,
        "laptop" => DeviceType::Laptop,
        "mobile" => DeviceType::Mobile,
        "server" => DeviceType::Server,
        "tablet" => DeviceType::Tablet,
        _ => DeviceType::Other,
    }
}

fn device_type_str(dt: DeviceType) -> &'static str {
    match dt {
        DeviceType::Desktop => "desktop",
        DeviceType::Laptop => "laptop",
        DeviceType::Mobile => "mobile",
        DeviceType::Server => "server",
        DeviceType::Tablet => "tablet",
        DeviceType::Other => "other",
    }
}

/// POST /api/2/devices/{username}/{deviceid}.json
///
/// Create or update a device. The gpodder API sends caption and type in the body.
pub async fn update_device(
    State(state): State<AppState>,
    Path((username, deviceid_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<DeviceUpdateRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let deviceid = deviceid_json
        .strip_suffix(".json")
        .unwrap_or(&deviceid_json)
        .to_string();

    let now = Utc::now();
    let dt = body
        .device_type
        .as_deref()
        .map(parse_device_type)
        .unwrap_or(DeviceType::Other);
    let caption = body.caption.unwrap_or_default();

    let device = Device {
        id: Uuid::now_v7(),
        user_id: auth_user.0.id,
        device_id: deviceid,
        caption,
        device_type: dt,
        sync_group_id: None,
        created_at: now,
        updated_at: now,
    };

    let result = match &*state.db {
        Db::Postgres(pool) => {
            let repo = PgRepo::new(pool.clone());
            DeviceRepo::upsert(&repo, &device).await
        }
        Db::Sqlite(pool) => {
            let repo = SqliteRepo::new(pool.clone());
            DeviceRepo::upsert(&repo, &device).await
        }
    };

    result.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

/// GET /api/2/devices/{username}.json
///
/// List all devices for a user.
pub async fn list_devices(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = username_json
        .strip_suffix(".json")
        .unwrap_or(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let devices = match &*state.db {
        Db::Postgres(pool) => {
            let repo = PgRepo::new(pool.clone());
            DeviceRepo::list_for_user(&repo, auth_user.0.id).await
        }
        Db::Sqlite(pool) => {
            let repo = SqliteRepo::new(pool.clone());
            DeviceRepo::list_for_user(&repo, auth_user.0.id).await
        }
    };

    let devices = devices.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<DeviceResponse> = devices
        .into_iter()
        .map(|d| DeviceResponse {
            id: d.device_id,
            caption: d.caption,
            device_type: device_type_str(d.device_type).to_string(),
            subscriptions: 0, // TODO: count actual subscriptions in Phase 1.4
        })
        .collect();

    Ok(Json(response))
}
