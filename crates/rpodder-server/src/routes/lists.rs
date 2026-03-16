use axum::{
    Extension,
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rpodder_core::repo::{PodcastListRepo, PodcastRepo};
use rpodder_core::types::PodcastList;

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

fn strip_format(s: &str) -> &str {
    s.strip_suffix(".json")
        .or_else(|| s.strip_suffix(".opml"))
        .or_else(|| s.strip_suffix(".txt"))
        .unwrap_or(s)
}

#[derive(Deserialize)]
pub struct CreateListRequest {
    pub title: String,
}

#[derive(Serialize)]
pub struct ListSummary {
    pub title: String,
    pub slug: String,
    pub web: String,
}

#[derive(Serialize)]
pub struct ListDetail {
    pub title: String,
    pub slug: String,
    pub podcasts: Vec<ListPodcast>,
}

#[derive(Serialize)]
pub struct ListPodcast {
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// POST /api/2/lists/{username}/create.json
pub async fn create_list(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<CreateListRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let now = Utc::now();
    let slug = slugify(&body.title);

    let list = PodcastList {
        id: Uuid::now_v7(),
        user_id: auth_user.0.id,
        title: body.title,
        slug,
        created_at: now,
        updated_at: now,
    };

    with_repo!(state, |repo| {
        PodcastListRepo::create(&repo, &list).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(ListSummary {
            title: list.title,
            slug: list.slug.clone(),
            web: format!("/api/2/lists/{}/{}", username, list.slug),
        }),
    ))
}

/// GET /api/2/lists/{username}.json
pub async fn get_lists(
    State(state): State<AppState>,
    Path(username_json): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let username = strip_format(&username_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let lists = with_repo!(state, |repo| {
        PodcastListRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<ListSummary> = lists
        .into_iter()
        .map(|l| ListSummary {
            web: format!("/api/2/lists/{}/{}", username, l.slug),
            title: l.title,
            slug: l.slug,
        })
        .collect();

    Ok(Json(results))
}

/// GET /api/2/lists/{username}/list/{slug}.json
pub async fn get_list(
    State(state): State<AppState>,
    Path((username, slug_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let slug = strip_format(&slug_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let list = with_repo!(state, |repo| {
        PodcastListRepo::find_by_slug(&repo, auth_user.0.id, slug).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let entries = with_repo!(state, |repo| {
        PodcastListRepo::get_entries(&repo, list.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let podcasts: Vec<ListPodcast> = entries
        .into_iter()
        .map(|p| ListPodcast {
            url: String::new(),
            title: p.title,
            author: p.author,
        })
        .collect();

    Ok(Json(ListDetail {
        title: list.title,
        slug: list.slug,
        podcasts,
    }))
}

/// PUT /api/2/lists/{username}/list/{slug}.json
#[derive(Deserialize)]
pub struct UpdateListRequest {
    pub title: Option<String>,
    pub podcasts: Option<Vec<String>>,
}

pub async fn update_list(
    State(state): State<AppState>,
    Path((username, slug_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<UpdateListRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let slug = strip_format(&slug_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let list = with_repo!(state, |repo| {
        PodcastListRepo::find_by_slug(&repo, auth_user.0.id, slug).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(podcast_urls) = &body.podcasts {
        let mut podcast_ids = Vec::new();
        for url in podcast_urls {
            let (podcast, _) = with_repo!(state, |repo| {
                PodcastRepo::get_or_create_for_url(&repo, url).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            podcast_ids.push(podcast.id);
        }

        with_repo!(state, |repo| {
            PodcastListRepo::set_entries(&repo, list.id, &podcast_ids).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(StatusCode::OK)
}

/// DELETE /api/2/lists/{username}/list/{slug}.json
pub async fn delete_list(
    State(state): State<AppState>,
    Path((username, slug_json)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<impl IntoResponse, StatusCode> {
    let slug = strip_format(&slug_json);
    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let list = with_repo!(state, |repo| {
        PodcastListRepo::find_by_slug(&repo, auth_user.0.id, slug).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    with_repo!(state, |repo| {
        PodcastListRepo::delete(&repo, list.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}
