use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rpodder_core::repo::{DeviceRepo, EpisodeActionRepo, EpisodeRepo, PodcastRepo};
use rpodder_core::types::{EpisodeAction, EpisodeActionType};

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{postgres::PgRepo, sqlite::SqliteRepo, Db};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EpisodeActionUpload {
    pub podcast: String,
    pub episode: String,
    pub device: Option<String>,
    pub action: String,
    pub timestamp: Option<String>,
    pub started: Option<i32>,
    pub position: Option<i32>,
    pub total: Option<i32>,
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
    pub aggregated: Option<bool>,
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
    Json(body): Json<UploadBody>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json_suffix(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let update_urls: Vec<Vec<String>> = Vec::new();

    for action_input in &body.actions {
        let action_type = parse_action_type(&action_input.action)
            .ok_or(StatusCode::BAD_REQUEST)?;

        // Validate play action fields
        if action_type == EpisodeActionType::Play {
            if action_input.position.is_none() {
                return Err(StatusCode::BAD_REQUEST);
            }
        }

        // Resolve podcast
        let (podcast, _) = with_repo!(state, |repo| {
            PodcastRepo::get_or_create_for_url(&repo, &action_input.podcast).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Resolve episode
        let (episode, _) = with_repo!(state, |repo| {
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &action_input.episode).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Resolve device (optional)
        let device_uuid = if let Some(device_str) = &action_input.device {
            let device = with_repo!(state, |repo| {
                DeviceRepo::find_by_uid(&repo, auth_user.0.id, device_str).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            device.map(|d| d.id)
        } else {
            None
        };

        // Parse timestamp
        let timestamp = action_input
            .timestamp
            .as_deref()
            .and_then(|ts| {
                // Try ISO 8601 formats
                ts.parse().ok()
                    .or_else(|| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S").ok().map(|ndt| ndt.and_utc()))
                    .or_else(|| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").ok().map(|ndt| ndt.and_utc()))
            })
            .unwrap_or_else(Utc::now);

        let ep_action = EpisodeAction {
            id: Uuid::now_v7(),
            user_id: auth_user.0.id,
            device_id: device_uuid,
            episode_id: episode.id,
            action: action_type,
            podcast_ref_url: Some(action_input.podcast.clone()),
            episode_ref_url: Some(action_input.episode.clone()),
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
        timestamp: Utc::now().timestamp(),
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

    let since = params
        .since
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single());

    // Resolve device filter
    let device_uuid = if let Some(device_str) = &params.device {
        let device = with_repo!(state, |repo| {
            DeviceRepo::find_by_uid(&repo, auth_user.0.id, device_str).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        device.map(|d| d.id)
    } else {
        None
    };

    // Resolve podcast filter
    let podcast_uuid = if let Some(podcast_url) = &params.podcast {
        let podcast = with_repo!(state, |repo| {
            PodcastRepo::find_by_url(&repo, podcast_url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        podcast.map(|p| p.id)
    } else {
        None
    };

    let actions = with_repo!(state, |repo| {
        EpisodeActionRepo::list(&repo, auth_user.0.id, device_uuid, podcast_uuid, since, 1000)
            .await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Look up device_id strings for the response
    let response_actions: Vec<EpisodeActionResponse> = actions
        .into_iter()
        .map(|a| {
            EpisodeActionResponse {
                podcast: a.podcast_ref_url.unwrap_or_default(),
                episode: a.episode_ref_url.unwrap_or_default(),
                action: action_type_str(a.action).to_string(),
                device: None, // TODO: resolve device_id → device_uid
                timestamp: Some(a.timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()),
                started: a.started,
                position: a.position,
                total: a.total,
            }
        })
        .collect();

    Ok(Json(DownloadResponse {
        actions: response_actions,
        timestamp: Utc::now().timestamp(),
    }))
}
