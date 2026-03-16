use axum::{
    Extension,
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rpodder_core::repo::{ChapterRepo, EpisodeRepo, PodcastRepo};
use rpodder_core::types::Chapter;

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

#[derive(Deserialize)]
pub struct ChapterQuery {
    pub podcast: String,
    pub episode: String,
    pub since: Option<i64>,
}

#[derive(Deserialize)]
pub struct ChapterInput {
    pub podcast: String,
    pub episode: String,
    pub start: i32,
    pub end: i32,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub advertisement: bool,
}

#[derive(Deserialize)]
pub struct ChapterDeleteInput {
    pub podcast: String,
    pub episode: String,
    pub start: i32,
    pub end: i32,
}

#[derive(Deserialize)]
pub struct ChapterUpload {
    pub add: Option<Vec<ChapterInput>>,
    pub remove: Option<Vec<ChapterDeleteInput>>,
}

#[derive(Serialize)]
pub struct ChapterResponse {
    pub start: i32,
    pub end: i32,
    pub label: String,
    pub advertisement: bool,
}

#[derive(Serialize)]
pub struct ChaptersResponse {
    pub chapters: Vec<ChapterResponse>,
    pub timestamp: i64,
}

/// GET /api/2/chapters/{username}.json?podcast=X&episode=Y&since=T
pub async fn get_chapters(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<ChapterQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let podcast = with_repo!(state, |repo| {
        PodcastRepo::find_by_url(&repo, &params.podcast).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let (episode, _) = with_repo!(state, |repo| {
        EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &params.episode).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let since = params
        .since
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single());

    let chapters = with_repo!(state, |repo| {
        ChapterRepo::list_for_episode(&repo, auth_user.0.id, episode.id, since).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<ChapterResponse> = chapters
        .into_iter()
        .map(|c| ChapterResponse {
            start: c.start_sec,
            end: c.end_sec,
            label: c.label,
            advertisement: c.advertisement,
        })
        .collect();

    Ok(Json(ChaptersResponse {
        chapters: results,
        timestamp: Utc::now().timestamp(),
    }))
}

/// POST /api/2/chapters/{username}.json
pub async fn update_chapters(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<ChapterUpload>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_json(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    // Add chapters
    if let Some(adds) = &body.add {
        for input in adds {
            let podcast = with_repo!(state, |repo| {
                PodcastRepo::find_by_url(&repo, &input.podcast).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

            let (episode, _) = with_repo!(state, |repo| {
                EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &input.episode).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let chapter = Chapter {
                id: Uuid::now_v7(),
                user_id: auth_user.0.id,
                episode_id: episode.id,
                start_sec: input.start,
                end_sec: input.end,
                label: input.label.clone(),
                advertisement: input.advertisement,
                created_at: Utc::now(),
            };

            with_repo!(state, |repo| ChapterRepo::upsert(&repo, &chapter).await)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    // Remove chapters
    if let Some(removes) = &body.remove {
        for input in removes {
            let podcast = with_repo!(state, |repo| {
                PodcastRepo::find_by_url(&repo, &input.podcast).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if let Some(podcast) = podcast {
                let (episode, _) = with_repo!(state, |repo| {
                    EpisodeRepo::get_or_create_for_url(&repo, podcast.id, &input.episode).await
                })
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                with_repo!(state, |repo| {
                    ChapterRepo::delete(&repo, auth_user.0.id, episode.id, input.start, input.end)
                        .await
                })
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }
    }

    Ok(Json(
        serde_json::json!({ "timestamp": Utc::now().timestamp() }),
    ))
}
