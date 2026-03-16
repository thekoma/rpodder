use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use rpodder_core::privacy::is_likely_private_url;
use rpodder_core::repo::{EpisodeRepo, PodcastRepo, SubscriptionRepo, TagRepo};

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
        PodcastRepo::search(&repo, &params.q, 60).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut results = podcasts_to_responses(&state, podcasts).await;
    results.truncate(20);

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
    let count = count.clamp(1, 100);

    // Fetch more than needed because privacy filter may remove some
    let podcasts = with_repo!(state, |repo| {
        PodcastRepo::toplist(&repo, count * 3, None).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut results = podcasts_to_responses(&state, podcasts).await;
    results.truncate(count as usize);

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

    // On-demand fetch if no metadata yet
    let podcast = if podcast.title == params.url || podcast.description.is_empty() {
        let fetcher = rpodder_feed::FeedFetcher::new();
        let _ = crate::feed_updater::update_podcast_feed(&state.db, &fetcher, &params.url).await;
        with_repo!(state, |repo| {
            PodcastRepo::find_by_url(&repo, &params.url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or(podcast)
    } else {
        podcast
    };

    Ok(Json(podcast_to_response(podcast, &params.url)))
}

// ---------------------------------------------------------------------------
// GET /api/2/data/podcast/episodes.json?url=X&page=0&per_page=50
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PodcastEpisodesQuery {
    pub url: String,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct EpisodeListItem {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub released: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mimetype: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PodcastEpisodesResponse {
    pub podcast: PodcastResponse,
    pub episodes: Vec<EpisodeListItem>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

pub async fn podcast_episodes(
    State(state): State<AppState>,
    Query(params): Query<PodcastEpisodesQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let podcast = with_repo!(state, |repo| {
        PodcastRepo::find_by_url(&repo, &params.url).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // On-demand fetch: if podcast has no metadata yet (title == url), fetch it now
    let podcast = if podcast.title == params.url || podcast.description.is_empty() {
        let fetcher = rpodder_feed::FeedFetcher::new();
        let _ = crate::feed_updater::update_podcast_feed(&state.db, &fetcher, &params.url).await;
        // Re-read with updated metadata
        with_repo!(state, |repo| {
            PodcastRepo::find_by_url(&repo, &params.url).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or(podcast)
    } else {
        podcast
    };

    let page = params.page.unwrap_or(0);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 100);
    let offset = page * per_page;

    let episodes = with_repo!(state, |repo| {
        EpisodeRepo::list_for_podcast(&repo, podcast.id, per_page, offset).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let episode_items: Vec<EpisodeListItem> = episodes
        .into_iter()
        .map(|e| EpisodeListItem {
            title: e.title,
            description: if e.description.is_empty() {
                None
            } else {
                Some(e.description)
            },
            released: e.released.map(|r| r.to_rfc3339()),
            duration: e.duration,
            mimetype: e.mimetype,
        })
        .collect();

    Ok(Json(PodcastEpisodesResponse {
        podcast: podcast_to_response(podcast, &params.url),
        episodes: episode_items,
        total: 0, // TODO: count total episodes
        page,
        per_page,
    }))
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
    type EpRow = (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );
    let episode: Option<EpRow> = match &*state.db {
        Db::Postgres(pool) => {
            sqlx::query_as(
                "SELECT e.title, eu.url, e.description, e.link, e.released::text, e.duration, e.filesize, e.mimetype
                 FROM episodes e
                 JOIN episode_urls eu ON eu.episode_id = e.id
                 WHERE eu.url = $1 AND e.podcast_id = $2",
            )
            .bind(&params.url)
            .bind(podcast.id)
            .fetch_optional(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
        Db::Sqlite(pool) => {
            sqlx::query_as(
                "SELECT e.title, eu.url, e.description, e.link, e.released, e.duration, e.filesize, e.mimetype
                 FROM episodes e
                 JOIN episode_urls eu ON eu.episode_id = e.id
                 WHERE eu.url = ? AND e.podcast_id = ?",
            )
            .bind(&params.url)
            .bind(podcast.id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
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

// ---------------------------------------------------------------------------
// GET /api/2/tags/{count}.json
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct TagResponse {
    pub tag: String,
    pub usage: i64,
}

pub async fn top_tags(
    State(state): State<AppState>,
    Path(count_raw): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let count_str = strip_format_suffix(&count_raw);
    let count: i64 = count_str.parse().unwrap_or(50);
    let count = count.clamp(1, 200);

    let tags = with_repo!(state, |repo| TagRepo::top_tags(&repo, count).await)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<TagResponse> = tags
        .into_iter()
        .map(|(tag, usage)| TagResponse { tag, usage })
        .collect();

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// GET /api/2/tag/{tag}/{count}.json
// ---------------------------------------------------------------------------

pub async fn podcasts_for_tag(
    State(state): State<AppState>,
    Path((tag, count_raw)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let count_str = strip_format_suffix(&count_raw);
    let count: i64 = count_str.parse().unwrap_or(50);
    let count = count.clamp(1, 100);

    let podcasts = with_repo!(state, |repo| {
        TagRepo::podcasts_for_tag(&repo, &tag, count).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results = podcasts_to_responses(&state, podcasts).await;

    Ok(Json(results))
}

// ---------------------------------------------------------------------------
// GET /suggestions/{count}.json
// ---------------------------------------------------------------------------

pub async fn suggestions(
    State(state): State<AppState>,
    Path(count_raw): Path<String>,
    axum::Extension(auth_user): axum::Extension<crate::middleware::auth::AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let count_str = strip_format_suffix(&count_raw);
    let count: i64 = count_str.parse().unwrap_or(10);
    let count = count.clamp(1, 100);

    // Get user's subscribed podcast IDs
    let user_subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let subscribed_ids: std::collections::HashSet<uuid::Uuid> =
        user_subs.iter().map(|s| s.podcast_id).collect();

    if subscribed_ids.is_empty() {
        // No subscriptions — suggest top podcasts
        let podcasts = with_repo!(state, |repo| {
            PodcastRepo::toplist(&repo, count, None).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let results = podcasts_to_responses(&state, podcasts).await;

        return Ok(Json(results));
    }

    // Get tags from user's subscribed podcasts
    let tags: Vec<String> = match &*state.db {
        Db::Postgres(pool) => {
            let sub_ids: Vec<uuid::Uuid> = subscribed_ids.iter().copied().collect();
            let rows: Vec<(String,)> =
                sqlx::query_as("SELECT DISTINCT tag FROM tags WHERE podcast_id = ANY($1)")
                    .bind(&sub_ids)
                    .fetch_all(pool)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            rows.into_iter().map(|(t,)| t).collect()
        }
        Db::Sqlite(pool) => {
            // SQLite doesn't have ANY, build IN clause
            let placeholders: Vec<String> =
                subscribed_ids.iter().map(|_| "?".to_string()).collect();
            let sql = format!(
                "SELECT DISTINCT tag FROM tags WHERE podcast_id IN ({})",
                placeholders.join(",")
            );
            let mut q = sqlx::query_as::<_, (String,)>(&sql);
            for id in &subscribed_ids {
                q = q.bind(id.to_string());
            }
            let rows = q
                .fetch_all(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            rows.into_iter().map(|(t,)| t).collect()
        }
    };

    if tags.is_empty() {
        // No tags — fall back to toplist
        let podcasts = with_repo!(state, |repo| {
            PodcastRepo::toplist(&repo, count, None).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let filtered: Vec<_> = podcasts
            .into_iter()
            .filter(|p| !subscribed_ids.contains(&p.id))
            .collect();
        let results = podcasts_to_responses(&state, filtered).await;

        return Ok(Json(results));
    }

    // Find podcasts with the same tags, excluding already-subscribed ones
    let suggested: Vec<rpodder_core::types::Podcast> = match &*state.db {
        Db::Postgres(pool) => {
            let sub_ids: Vec<uuid::Uuid> = subscribed_ids.iter().copied().collect();
            sqlx::query_as::<_, PodcastRow>(
                "SELECT DISTINCT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                        p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                        p.created_at, p.updated_at
                 FROM podcasts p
                 JOIN tags t ON t.podcast_id = p.id
                 WHERE t.tag = ANY($1) AND p.id != ALL($2)
                 ORDER BY p.subscribers DESC
                 LIMIT $3",
            )
            .bind(&tags)
            .bind(&sub_ids)
            .bind(count)
            .fetch_all(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .into_iter()
            .map(Into::into)
            .collect()
        }
        Db::Sqlite(pool) => {
            let tag_placeholders: Vec<String> = tags.iter().map(|_| "?".to_string()).collect();
            let sub_placeholders: Vec<String> =
                subscribed_ids.iter().map(|_| "?".to_string()).collect();
            let sql = format!(
                "SELECT DISTINCT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                        p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                        p.created_at, p.updated_at
                 FROM podcasts p
                 JOIN tags t ON t.podcast_id = p.id
                 WHERE t.tag IN ({}) AND p.id NOT IN ({})
                 ORDER BY p.subscribers DESC
                 LIMIT ?",
                tag_placeholders.join(","),
                sub_placeholders.join(",")
            );
            let mut q = sqlx::query_as::<_, SqlitePodcastRow>(&sql);
            for tag in &tags {
                q = q.bind(tag);
            }
            for id in &subscribed_ids {
                q = q.bind(id.to_string());
            }
            q = q.bind(count);
            q.fetch_all(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .into_iter()
                .map(Into::into)
                .collect()
        }
    };

    let results = podcasts_to_responses(&state, suggested).await;

    Ok(Json(results))
}

#[derive(sqlx::FromRow)]
struct SqlitePodcastRow {
    id: String,
    title: String,
    description: String,
    link: Option<String>,
    language: Option<String>,
    logo_url: Option<String>,
    author: Option<String>,
    subscribers: i64,
    episode_count: i64,
    last_update: Option<String>,
    update_interval_hours: i32,
    created_at: String,
    updated_at: String,
}

impl From<SqlitePodcastRow> for rpodder_core::types::Podcast {
    fn from(r: SqlitePodcastRow) -> Self {
        rpodder_core::types::Podcast {
            id: r.id.parse().unwrap_or_default(),
            title: r.title,
            description: r.description,
            link: r.link,
            language: r.language,
            logo_url: r.logo_url,
            author: r.author,
            subscribers: r.subscribers,
            episode_count: r.episode_count,
            last_update: r.last_update.and_then(|s| s.parse().ok()),
            update_interval_hours: r.update_interval_hours,
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct PodcastRow {
    id: uuid::Uuid,
    title: String,
    description: String,
    link: Option<String>,
    language: Option<String>,
    logo_url: Option<String>,
    author: Option<String>,
    subscribers: i64,
    episode_count: i64,
    last_update: Option<chrono::DateTime<chrono::Utc>>,
    update_interval_hours: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<PodcastRow> for rpodder_core::types::Podcast {
    fn from(r: PodcastRow) -> Self {
        rpodder_core::types::Podcast {
            id: r.id,
            title: r.title,
            description: r.description,
            link: r.link,
            language: r.language,
            logo_url: r.logo_url,
            author: r.author,
            subscribers: r.subscribers,
            episode_count: r.episode_count,
            last_update: r.last_update,
            update_interval_hours: r.update_interval_hours,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
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

/// Convert podcasts to responses, looking up feed URLs from the DB.
/// Filters out podcasts with private/token URLs.
async fn podcasts_to_responses(
    state: &AppState,
    podcasts: Vec<rpodder_core::types::Podcast>,
) -> Vec<PodcastResponse> {
    let mut results = Vec::with_capacity(podcasts.len());
    for p in podcasts {
        let url = lookup_podcast_url(state, p.id).await.unwrap_or_default();
        // Skip private feeds from public directory
        if !url.is_empty() && is_likely_private_url(&url) {
            continue;
        }
        results.push(podcast_to_response(p, &url));
    }
    results
}

async fn lookup_podcast_url(state: &AppState, podcast_id: uuid::Uuid) -> Option<String> {
    let result: Option<(String,)> = match &*state.db {
        Db::Postgres(pool) => {
            sqlx::query_as("SELECT url FROM podcast_urls WHERE podcast_id = $1 AND \"order\" = 0")
                .bind(podcast_id)
                .fetch_optional(pool)
                .await
                .ok()?
        }
        Db::Sqlite(pool) => {
            sqlx::query_as("SELECT url FROM podcast_urls WHERE podcast_id = ? AND \"order\" = 0")
                .bind(podcast_id.to_string())
                .fetch_optional(pool)
                .await
                .ok()?
        }
    };
    result.map(|(url,)| url)
}
