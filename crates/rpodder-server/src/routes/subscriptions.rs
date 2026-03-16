use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{DeviceRepo, PodcastRepo, SubscriptionRepo};
use rpodder_core::types::SubscriptionAction;

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{postgres::PgRepo, sqlite::SqliteRepo, Db};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DeltaUploadRequest {
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DeltaResponse {
    pub add: Vec<String>,
    pub remove: Vec<String>,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct SinceQuery {
    pub since: Option<i64>,
}

// ---------------------------------------------------------------------------
// Helper: resolve device_id string → Device UUID
// ---------------------------------------------------------------------------

/// Macro to avoid repeating the Db::Postgres/Sqlite match for every repo call.
/// Returns the result of the expression for the appropriate backend.
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

fn strip_json_suffix(s: &str) -> &str {
    s.strip_suffix(".json").unwrap_or(s)
}

// ---------------------------------------------------------------------------
// Simple API: GET /subscriptions/{username}/{deviceid}.json
// ---------------------------------------------------------------------------

/// Returns the list of subscription URLs for a device.
pub async fn get_device_subscriptions(
    State(state): State<AppState>,
    Path((username_json, deviceid_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json_suffix(&username_json);
    let deviceid = strip_json_suffix(&deviceid_json);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let device = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, auth_user.0.id, deviceid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_device(&repo, auth_user.0.id, device.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let urls: Vec<String> = subs.into_iter().map(|s| s.ref_url).collect();
    Ok(Json(urls))
}

// ---------------------------------------------------------------------------
// Simple API: PUT /subscriptions/{username}/{deviceid}.json
// ---------------------------------------------------------------------------

/// Replace the full subscription list for a device.
pub async fn put_device_subscriptions(
    State(state): State<AppState>,
    Path((username, deviceid_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Json(urls): Json<Vec<String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let deviceid = strip_json_suffix(&deviceid_json);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let device = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, auth_user.0.id, deviceid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Get current subscriptions
    let current_subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_device(&repo, auth_user.0.id, device.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let current_urls: std::collections::HashSet<String> =
        current_subs.iter().map(|s| s.ref_url.clone()).collect();
    let new_urls: std::collections::HashSet<String> = urls.into_iter().collect();

    // Unsubscribe from URLs no longer in the list
    for sub in &current_subs {
        if !new_urls.contains(&sub.ref_url) {
            with_repo!(state, |repo| {
                SubscriptionRepo::unsubscribe(&repo, auth_user.0.id, device.id, sub.podcast_id)
                    .await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    // Subscribe to new URLs
    for url in &new_urls {
        if !current_urls.contains(url) {
            let (podcast, _) = with_repo!(state, |repo| {
                PodcastRepo::get_or_create_for_url(&repo, url).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            with_repo!(state, |repo| {
                SubscriptionRepo::subscribe(&repo, auth_user.0.id, device.id, podcast.id, url)
                    .await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Simple API: GET /subscriptions/{username}.json
// ---------------------------------------------------------------------------

/// Returns all subscription URLs for a user (across all devices, deduplicated).
pub async fn get_user_subscriptions(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json_suffix(&username_json);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let urls: Vec<String> = subs.into_iter().map(|s| s.ref_url).collect();
    Ok(Json(urls))
}

// ---------------------------------------------------------------------------
// Advanced API: POST /api/2/subscriptions/{username}/{deviceid}.json
// ---------------------------------------------------------------------------

/// Upload subscription changes (add/remove URLs).
pub async fn upload_subscription_changes(
    State(state): State<AppState>,
    Path((username, deviceid_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<DeltaUploadRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let deviceid = strip_json_suffix(&deviceid_json);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let device = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, auth_user.0.id, deviceid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Process removals first (a URL in both add and remove should end as subscribed)
    for url in &body.remove {
        // Find the podcast for this URL
        let podcast = with_repo!(state, |repo| {
            PodcastRepo::find_by_url(&repo, url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some(podcast) = podcast {
            with_repo!(state, |repo| {
                SubscriptionRepo::unsubscribe(&repo, auth_user.0.id, device.id, podcast.id).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    // Process additions
    for url in &body.add {
        let (podcast, _) = with_repo!(state, |repo| {
            PodcastRepo::get_or_create_for_url(&repo, url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        with_repo!(state, |repo| {
            SubscriptionRepo::subscribe(&repo, auth_user.0.id, device.id, podcast.id, url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    let timestamp = Utc::now().timestamp();

    Ok(Json(DeltaResponse {
        add: body.add,
        remove: body.remove,
        timestamp,
    }))
}

// ---------------------------------------------------------------------------
// Advanced API: GET /api/2/subscriptions/{username}/{deviceid}.json?since=T
// ---------------------------------------------------------------------------

/// Download subscription changes since a timestamp.
pub async fn download_subscription_changes(
    State(state): State<AppState>,
    Path((username, deviceid_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<SinceQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let deviceid = strip_json_suffix(&deviceid_json);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let device = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, auth_user.0.id, deviceid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let since = params
        .since
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap());

    let changes = with_repo!(state, |repo| {
        SubscriptionRepo::changes_since(&repo, auth_user.0.id, device.id, since).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut add = Vec::new();
    let mut remove = Vec::new();

    for change in changes {
        match change.action {
            SubscriptionAction::Subscribe => add.push(change.ref_url),
            SubscriptionAction::Unsubscribe => remove.push(change.ref_url),
        }
    }

    let timestamp = Utc::now().timestamp();

    Ok(Json(DeltaResponse {
        add,
        remove,
        timestamp,
    }))
}
