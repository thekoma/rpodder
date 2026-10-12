use axum::{
    Extension,
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rpodder_core::repo::{DeviceRepo, EpisodeActionRepo, EpisodeRepo, PodcastRepo};
use rpodder_core::types::{EpisodeAction, EpisodeActionType};
use rpodder_core::url::normalize_url;

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{Db, postgres::PgRepo, sqlite::SqliteRepo};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EpisodeActionUpload {
    pub podcast: String,
    #[serde(default)]
    pub episode: String,
    pub device: Option<String>,
    pub action: String,
    pub timestamp: Option<serde_json::Value>, // Accept both string and integer
    pub started: Option<i32>,
    pub position: Option<i32>,
    pub total: Option<i32>,
    // Accept any extra fields Kasts may send (guid, etc.)
    #[serde(flatten)]
    pub _extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct EpisodeActionResponse {
    pub podcast: String,
    pub episode: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UploadBody {
    pub actions: Vec<EpisodeActionUpload>,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub timestamp: i64,
    pub update_urls: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    pub actions: Vec<EpisodeActionResponse>,
    pub timestamp: i64,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeQuery {
    pub since: Option<i64>,
    pub podcast: Option<String>,
    pub device: Option<String>,
    /// Only return the latest action for each episode.
    #[serde(default)]
    pub aggregated: bool,
}

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

/// gpodder device ids: `[\w.-]+`, and `devices.device_id` is VARCHAR(64) on PostgreSQL.
fn is_valid_device_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn strip_json_suffix(s: &str) -> &str {
    s.strip_suffix(".json").unwrap_or(s)
}

fn parse_action_type(s: &str) -> Option<EpisodeActionType> {
    match s {
        "download" => Some(EpisodeActionType::Download),
        "play" => Some(EpisodeActionType::Play),
        "delete" => Some(EpisodeActionType::Delete),
        "new" => Some(EpisodeActionType::New),
        _ => None,
    }
}

fn action_type_str(a: EpisodeActionType) -> &'static str {
    match a {
        EpisodeActionType::Download => "download",
        EpisodeActionType::Play => "play",
        EpisodeActionType::Delete => "delete",
        EpisodeActionType::New => "new",
    }
}

// ---------------------------------------------------------------------------
// POST /api/2/episodes/{username}.json
// ---------------------------------------------------------------------------

pub async fn upload_episode_actions(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    raw_body: String,
) -> Result<impl IntoResponse, StatusCode> {
    // Accept both {"actions": [...]} and bare [...] array (Kasts sends the latter)
    let body: UploadBody = match serde_json::from_str::<UploadBody>(&raw_body) {
        Ok(b) => b,
        Err(_) => match serde_json::from_str::<Vec<EpisodeActionUpload>>(&raw_body) {
            Ok(actions) => UploadBody { actions },
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    body = &raw_body[..raw_body.len().min(500)],
                    "failed to parse episode actions upload"
                );
                return Err(StatusCode::BAD_REQUEST);
            }
        },
    };
    let username = strip_json_suffix(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let timestamp = super::helpers::sync_cursor();
    let update_urls: Vec<Vec<String>> = Vec::new();
    let mut devices: HashMap<String, Uuid> = HashMap::new();

    for action_input in &body.actions {
        let action_type = parse_action_type(&action_input.action).ok_or(StatusCode::BAD_REQUEST)?;

        // Validate play action fields
        if action_type == EpisodeActionType::Play && action_input.position.is_none() {
            return Err(StatusCode::BAD_REQUEST);
        }

        // Normalize URLs
        let podcast_url = normalize_url(&action_input.podcast);
        let episode_url = if action_input.episode.is_empty() {
            continue; // Skip actions without episode URL
        } else {
            normalize_url(&action_input.episode)
        };

        // Resolve podcast
        let (podcast, _) = with_repo!(state, |repo| {
            PodcastRepo::get_or_create_for_url(&repo, &podcast_url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Resolve episode
        let (episode, _) = with_repo!(state, |repo| {
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &episode_url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Resolve device (optional): an existing device always matches; an
        // unknown one is auto-created like gpodder.net does, unless its id
        // could never be a valid device (then the action is stored without one).
        let device_uuid = match &action_input.device {
            Some(device_str) if !device_str.is_empty() => {
                if let Some(id) = devices.get(device_str) {
                    Some(*id)
                } else {
                    let existing = with_repo!(state, |repo| {
                        DeviceRepo::find_by_uid(&repo, auth_user.0.id, device_str).await
                    })
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    let id = match existing {
                        Some(d) => Some(d.id),
                        None if is_valid_device_id(device_str) => Some(
                            super::helpers::find_or_create_device(
                                &state,
                                auth_user.0.id,
                                device_str,
                            )
                            .await?
                            .id,
                        ),
                        None => None,
                    };
                    if let Some(id) = id {
                        devices.insert(device_str.clone(), id);
                    }
                    id
                }
            }
            _ => None,
        };

        // Parse timestamp — accept string (ISO 8601) or integer (unix epoch)
        let timestamp = match &action_input.timestamp {
            Some(serde_json::Value::String(ts)) => ts
                .parse()
                .ok()
                .or_else(|| {
                    chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                        .ok()
                        .map(|ndt| ndt.and_utc())
                })
                .or_else(|| {
                    chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
                        .ok()
                        .map(|ndt| ndt.and_utc())
                })
                .unwrap_or_else(Utc::now),
            Some(serde_json::Value::Number(n)) => n
                .as_i64()
                .and_then(|ts| TimeZone::timestamp_opt(&Utc, ts, 0).single())
                .unwrap_or_else(Utc::now),
            _ => Utc::now(),
        };

        let ep_action = EpisodeAction {
            id: Uuid::now_v7(),
            user_id: auth_user.0.id,
            device_id: device_uuid,
            episode_id: episode.id,
            action: action_type,
            podcast_ref_url: Some(podcast_url.clone()),
            episode_ref_url: Some(episode_url.clone()),
            started: action_input.started,
            position: action_input.position,
            total: action_input.total,
            timestamp,
            created_at: Utc::now(),
        };

        with_repo!(state, |repo| {
            EpisodeActionRepo::create(&repo, &ep_action).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // update_urls: pairs of [original_url, server_url] for URL normalization
        // For now we return empty (no URL rewriting)
    }

    Ok(Json(UploadResponse {
        timestamp,
        update_urls,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/2/episodes/{username}.json
// ---------------------------------------------------------------------------

pub async fn download_episode_actions(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<EpisodeQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json_suffix(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    // Taken before querying, with slack (see helpers::SYNC_CURSOR_SLACK_SECS).
    let timestamp = super::helpers::sync_cursor();
    let empty = || {
        Json(DownloadResponse {
            actions: Vec::new(),
            timestamp,
        })
    };

    let since = params
        .since
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single());

    // Resolve device filter; an unknown device has no actions
    let device_uuid = if let Some(device_str) = &params.device {
        let device = with_repo!(state, |repo| {
            DeviceRepo::find_by_uid(&repo, auth_user.0.id, device_str).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        match device {
            Some(d) => Some(d.id),
            None => return Ok(empty()),
        }
    } else {
        None
    };

    // Resolve podcast filter; an unknown podcast has no actions
    let podcast_uuid = if let Some(podcast_url) = &params.podcast {
        let podcast_url = normalize_url(podcast_url);
        let podcast = with_repo!(state, |repo| {
            PodcastRepo::find_by_url(&repo, &podcast_url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        match podcast {
            Some(p) => Some(p.id),
            None => return Ok(empty()),
        }
    } else {
        None
    };

    // Unbounded on purpose: the gpodder API has no pagination and `since` has
    // one-second granularity, so a page cursor would wedge a client whose
    // backlog shares a second (e.g. one bulk first-sync upload). mygpo also
    // returns every action since the cursor.
    let mut actions = with_repo!(state, |repo| {
        EpisodeActionRepo::list(
            &repo,
            auth_user.0.id,
            device_uuid,
            podcast_uuid,
            since,
            None,
        )
        .await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if params.aggregated {
        actions = latest_per_episode(actions);
    }

    let device_uids: HashMap<Uuid, String> = with_repo!(state, |repo| {
        DeviceRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .map(|d| (d.id, d.device_id))
    .collect();

    let response_actions: Vec<EpisodeActionResponse> = actions
        .into_iter()
        .map(|a| EpisodeActionResponse {
            podcast: a.podcast_ref_url.unwrap_or_default(),
            episode: a.episode_ref_url.unwrap_or_default(),
            action: action_type_str(a.action).to_string(),
            device: a.device_id.and_then(|id| device_uids.get(&id).cloned()),
            timestamp: Some(a.timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()),
            started: a.started,
            position: a.position,
            total: a.total,
        })
        .collect();

    Ok(Json(DownloadResponse {
        actions: response_actions,
        timestamp,
    }))
}

/// Keep only the most recent action (by client timestamp) for each episode,
/// preserving the receive order of the survivors.
fn latest_per_episode(actions: Vec<EpisodeAction>) -> Vec<EpisodeAction> {
    let mut latest: HashMap<Uuid, usize> = HashMap::new();
    for (i, a) in actions.iter().enumerate() {
        let newer = latest
            .get(&a.episode_id)
            .is_none_or(|&j| a.timestamp >= actions[j].timestamp);
        if newer {
            latest.insert(a.episode_id, i);
        }
    }
    let mut keep: Vec<usize> = latest.into_values().collect();
    keep.sort_unstable();
    let mut actions: Vec<Option<EpisodeAction>> = actions.into_iter().map(Some).collect();
    keep.into_iter().filter_map(|i| actions[i].take()).collect()
}
