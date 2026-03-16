use axum::http::StatusCode;
use chrono::Utc;
use uuid::Uuid;

use rpodder_core::repo::DeviceRepo;
use rpodder_core::types::{Device, DeviceType};

use crate::state::AppState;
use rpodder_db::{postgres::PgRepo, sqlite::SqliteRepo, Db};

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

/// Find a device by user_id + device_id string, or auto-create it.
/// This matches gpodder.net behavior where devices are created on first use.
pub async fn find_or_create_device(
    state: &AppState,
    user_id: Uuid,
    device_uid: &str,
) -> Result<Device, StatusCode> {
    let existing = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, user_id, device_uid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(device) = existing {
        return Ok(device);
    }

    // Auto-create device
    let now = Utc::now();
    let device = Device {
        id: Uuid::now_v7(),
        user_id,
        device_id: device_uid.to_string(),
        caption: device_uid.to_string(),
        device_type: DeviceType::Other,
        sync_group_id: None,
        created_at: now,
        updated_at: now,
    };

    let created = with_repo!(state, |repo| {
        DeviceRepo::upsert(&repo, &device).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(created)
}
