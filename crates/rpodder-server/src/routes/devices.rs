use axum::{
    Extension,
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rpodder_core::repo::{DeviceRepo, SubscriptionRepo};
use rpodder_core::types::{Device, DeviceType};

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

    with_repo!(state, |repo| DeviceRepo::upsert(&repo, &device).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

/// GET /api/2/devices/{username}.json
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

    let devices = with_repo!(state, |repo| {
        DeviceRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut response: Vec<DeviceResponse> = Vec::new();
    for d in devices {
        let sub_count = with_repo!(state, |repo| {
            SubscriptionRepo::list_for_device(&repo, auth_user.0.id, d.id).await
        })
        .map(|subs| subs.len() as i64)
        .unwrap_or(0);

        response.push(DeviceResponse {
            id: d.device_id,
            caption: d.caption,
            device_type: device_type_str(d.device_type).to_string(),
            subscriptions: sub_count,
        });
    }

    Ok(Json(response))
}

/// DELETE /api/2/devices/{username}/{deviceid}.json
pub async fn delete_device(
    State(state): State<AppState>,
    Path((username, deviceid_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let deviceid = deviceid_json
        .strip_suffix(".json")
        .unwrap_or(&deviceid_json);

    // Find the device
    let device = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, auth_user.0.id, deviceid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Delete device (cascades to subscriptions via FK)
    match &*state.db {
        Db::Postgres(pool) => {
            sqlx::query("DELETE FROM devices WHERE id = $1")
                .bind(device.id)
                .execute(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
        Db::Sqlite(pool) => {
            sqlx::query("DELETE FROM devices WHERE id = ?")
                .bind(device.id.to_string())
                .execute(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    Ok(StatusCode::OK)
}
