use axum::http::StatusCode;
use chrono::Utc;
use uuid::Uuid;

use rpodder_core::repo::{DeviceRepo, SubscriptionRepo, SyncGroupRepo};
use rpodder_core::types::{Device, DeviceType};

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

    let created = with_repo!(state, |repo| DeviceRepo::upsert(&repo, &device).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(created)
}

/// Propagate a subscription change to all devices in the same sync group.
/// If the device is not in a sync group, this is a no-op.
pub async fn propagate_to_sync_group(
    state: &AppState,
    user_id: Uuid,
    source_device: &Device,
    podcast_id: Uuid,
    ref_url: &str,
    subscribe: bool,
) {
    let Some(sync_group_id) = source_device.sync_group_id else {
        return;
    };

    // Get all devices in the same sync group
    let groups = with_repo!(state, |repo| {
        SyncGroupRepo::get_groups_for_user(&repo, user_id).await
    });

    let Ok(groups) = groups else {
        return;
    };

    for (group, devices) in &groups {
        if group.id != sync_group_id {
            continue;
        }
        for device in devices {
            if device.id == source_device.id {
                continue; // Skip the source device
            }
            let result = if subscribe {
                with_repo!(state, |repo| {
                    SubscriptionRepo::subscribe(&repo, user_id, device.id, podcast_id, ref_url)
                        .await
                })
            } else {
                with_repo!(state, |repo| {
                    SubscriptionRepo::unsubscribe(&repo, user_id, device.id, podcast_id).await
                })
            };
            if let Err(e) = result {
                tracing::warn!(
                    device_id = %device.device_id,
                    error = %e,
                    "failed to propagate subscription to synced device"
                );
            }
        }
    }
}
