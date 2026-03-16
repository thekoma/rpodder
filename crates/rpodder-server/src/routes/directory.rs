use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{EpisodeRepo, PodcastRepo};

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

fn strip_format_suffix(s: &str) -> &str {
    s.strip_suffix(".json")
        .or_else(|| s.strip_suffix(".opml"))
        .or_else(|| s.strip_suffix(".txt"))
        .unwrap_or(s)
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct PodcastResponse {
    pub url: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    pub subscribers: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EpisodeResponse {
    pub title: String,
    pub url: String,
    pub podcast_title: String,
    pub podcast_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub released: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filesize: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mimetype: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(rename = "scale_logo")]
    pub _scale_logo: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct PodcastDataQuery {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeDataQuery {
    pub podcast: String,
    pub url: String,
}

// ---------------------------------------------------------------------------
// GET /search.json?q=query
// ---------------------------------------------------------------------------

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let podcasts = with_repo!(state, |repo| {
        PodcastRepo::search(&repo, &params.q, 20).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<PodcastResponse> = podcasts
        .into_iter()
        .map(|p| podcast_to_response(p, ""))
        .collect();

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// GET /toplist/{count}.json
// ---------------------------------------------------------------------------

pub async fn toplist(
    State(state): State<AppState>,
    Path(count_raw): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let count_str = strip_format_suffix(&count_raw);
    let count: i64 = count_str.parse().unwrap_or(50);
    let count = count.min(100).max(1);

    let podcasts = with_repo!(state, |repo| {
        PodcastRepo::toplist(&repo, count, None).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<PodcastResponse> = podcasts
        .into_iter()
        .map(|p| podcast_to_response(p, ""))
        .collect();

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// GET /api/2/data/podcast.json?url=X
// ---------------------------------------------------------------------------

pub async fn podcast_data(
    State(state): State<AppState>,
    Query(params): Query<PodcastDataQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let podcast = with_repo!(state, |repo| {
        PodcastRepo::find_by_url(&repo, &params.url).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(podcast_to_response(podcast, &params.url)))
}

// ---------------------------------------------------------------------------
// GET /api/2/data/episode.json?podcast=X&url=Y
// ---------------------------------------------------------------------------

pub async fn episode_data(
    State(state): State<AppState>,
    Query(params): Query<EpisodeDataQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let podcast = with_repo!(state, |repo| {
        PodcastRepo::find_by_url(&repo, &params.podcast).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Find episode by URL in episode_urls table
    let episode = match &*state.db {
        Db::Postgres(pool) => {
            let row: Option<(String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>)> = sqlx::query_as(
                "SELECT e.title, eu.url, e.description, e.link, e.released::text, e.duration, e.filesize, e.mimetype
                 FROM episodes e
                 JOIN episode_urls eu ON eu.episode_id = e.id
                 WHERE eu.url = $1 AND e.podcast_id = $2",
            )
            .bind(&params.url)
            .bind(podcast.id)
            .fetch_optional(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            row
        }
        Db::Sqlite(pool) => {
            let row: Option<(String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>)> = sqlx::query_as(
                "SELECT e.title, eu.url, e.description, e.link, e.released, e.duration, e.filesize, e.mimetype
                 FROM episodes e
                 JOIN episode_urls eu ON eu.episode_id = e.id
                 WHERE eu.url = ? AND e.podcast_id = ?",
            )
            .bind(&params.url)
            .bind(podcast.id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            row
        }
    };

    let (title, url, description, _link, released, duration, filesize, mimetype) =
        episode.ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(EpisodeResponse {
        title,
        url,
        podcast_title: podcast.title,
        podcast_url: params.podcast,
        description: if description.is_empty() {
            None
        } else {
            Some(description)
        },
        released,
        duration,
        filesize,
        mimetype,
    }))
}

fn podcast_to_response(p: rpodder_core::types::Podcast, url_hint: &str) -> PodcastResponse {
    PodcastResponse {
        url: if url_hint.is_empty() {
            String::new()
        } else {
            url_hint.to_string()
        },
        title: p.title,
        description: p.description,
        website: p.link,
        subscribers: p.subscribers,
        logo_url: p.logo_url,
        author: p.author,
        language: p.language,
    }
}
