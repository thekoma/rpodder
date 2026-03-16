use axum::{
    Extension,
    body::Body,
    extract::{Json, Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{DeviceRepo, PodcastRepo, SubscriptionRepo};
use rpodder_core::types::SubscriptionAction;
use rpodder_core::url::normalize_url;

use crate::middleware::auth::AuthUser;
use crate::state::AppState;
use rpodder_db::{Db, postgres::PgRepo, sqlite::SqliteRepo};

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
// Format detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    Json,
    Opml,
    Txt,
}

/// Parse the format suffix from a path segment like "device.opml" → ("device", Opml)
fn parse_format_suffix(s: &str) -> (&str, Format) {
    if let Some(name) = s.strip_suffix(".opml") {
        (name, Format::Opml)
    } else if let Some(name) = s.strip_suffix(".txt") {
        (name, Format::Txt)
    } else if let Some(name) = s.strip_suffix(".json") {
        (name, Format::Json)
    } else {
        (s, Format::Json)
    }
}

// ---------------------------------------------------------------------------
// OPML generation / parsing
// ---------------------------------------------------------------------------

fn urls_to_opml(urls: &[String], title: &str) -> String {
    let mut opml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>"#,
    );
    opml.push_str(&xml_escape(title));
    opml.push_str("</title></head>\n  <body>\n");
    for url in urls {
        opml.push_str(&format!(
            "    <outline type=\"rss\" xmlUrl=\"{}\" />\n",
            xml_escape(url)
        ));
    }
    opml.push_str("  </body>\n</opml>\n");
    opml
}

fn opml_to_urls(opml_str: &str) -> Vec<String> {
    // Simple parser: extract xmlUrl="..." from <outline> elements
    let mut urls = Vec::new();
    for line in opml_str.lines() {
        let trimmed = line.trim();
        if (trimmed.starts_with("<outline") || trimmed.starts_with("<Outline"))
            && let Some(url) = extract_xml_attr(trimmed, "xmlUrl")
                .or_else(|| extract_xml_attr(trimmed, "xmlurl"))
                .or_else(|| extract_xml_attr(trimmed, "XMLURL"))
            && !url.is_empty()
        {
            urls.push(xml_unescape(&url));
        }
    }
    urls
}

fn extract_xml_attr(element: &str, attr_name: &str) -> Option<String> {
    let needle = format!("{attr_name}=\"");
    let start = element.find(&needle)? + needle.len();
    let rest = &element[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

// ---------------------------------------------------------------------------
// TXT generation / parsing
// ---------------------------------------------------------------------------

fn urls_to_txt(urls: &[String]) -> String {
    let mut txt = String::new();
    for url in urls {
        txt.push_str(url);
        txt.push('\n');
    }
    txt
}

fn txt_to_urls(txt: &str) -> Vec<String> {
    txt.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

fn respond_urls(urls: &[String], format: Format, title: &str) -> Response {
    match format {
        Format::Json => Json(urls.to_vec()).into_response(),
        Format::Opml => Response::builder()
            .header(header::CONTENT_TYPE, "text/x-opml+xml; charset=utf-8")
            .body(Body::from(urls_to_opml(urls, title)))
            .unwrap(),
        Format::Txt => Response::builder()
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(urls_to_txt(urls)))
            .unwrap(),
    }
}

// ---------------------------------------------------------------------------
// Macro
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Simple API: GET /subscriptions/{username}/{deviceid}.{format}
// ---------------------------------------------------------------------------

pub async fn get_device_subscriptions(
    State(state): State<AppState>,
    Path((username_raw, deviceid_raw)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Response, StatusCode> {
    let (username, _) = parse_format_suffix(&username_raw);
    let (deviceid, format) = parse_format_suffix(&deviceid_raw);

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
    let title = format!("{username} — {deviceid} subscriptions");
    Ok(respond_urls(&urls, format, &title))
}

// ---------------------------------------------------------------------------
// Simple API: PUT /subscriptions/{username}/{deviceid}.{format}
// ---------------------------------------------------------------------------

pub async fn put_device_subscriptions(
    State(state): State<AppState>,
    Path((username_raw, deviceid_raw)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    body: String,
) -> Result<impl IntoResponse, StatusCode> {
    let (username, _) = parse_format_suffix(&username_raw);
    let (deviceid, format) = parse_format_suffix(&deviceid_raw);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    // Parse URLs from body based on format
    let urls = match format {
        Format::Json => {
            serde_json::from_str::<Vec<String>>(&body).map_err(|_| StatusCode::BAD_REQUEST)?
        }
        Format::Opml => opml_to_urls(&body),
        Format::Txt => txt_to_urls(&body),
    };

    // Normalize URLs
    let urls: Vec<String> = urls.into_iter().map(|u| normalize_url(&u)).collect();

    let device = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, auth_user.0.id, deviceid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let current_subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_device(&repo, auth_user.0.id, device.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let current_urls: std::collections::HashSet<String> =
        current_subs.iter().map(|s| s.ref_url.clone()).collect();
    let new_urls: std::collections::HashSet<String> = urls.into_iter().collect();

    for sub in &current_subs {
        if !new_urls.contains(&sub.ref_url) {
            with_repo!(state, |repo| {
                SubscriptionRepo::unsubscribe(&repo, auth_user.0.id, device.id, sub.podcast_id)
                    .await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    for url in &new_urls {
        if !current_urls.contains(url) {
            let (podcast, _) = with_repo!(state, |repo| {
                PodcastRepo::get_or_create_for_url(&repo, url).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            with_repo!(state, |repo| {
                SubscriptionRepo::subscribe(&repo, auth_user.0.id, device.id, podcast.id, url).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Simple API: GET /subscriptions/{username}.{format}
// ---------------------------------------------------------------------------

pub async fn get_user_subscriptions(
    State(state): State<AppState>,
    Path(username_raw): Path<String>,
    Extension(auth_user): Extension<AuthUser>,
) -> Result<Response, StatusCode> {
    let (username, format) = parse_format_suffix(&username_raw);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_user(&repo, auth_user.0.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let urls: Vec<String> = subs.into_iter().map(|s| s.ref_url).collect();
    let title = format!("{username} subscriptions");
    Ok(respond_urls(&urls, format, &title))
}

// ---------------------------------------------------------------------------
// Advanced API: POST /api/2/subscriptions/{username}/{deviceid}.json
// ---------------------------------------------------------------------------

pub async fn upload_subscription_changes(
    State(state): State<AppState>,
    Path((username, deviceid_raw)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Json(body): Json<DeltaUploadRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let (deviceid, _) = parse_format_suffix(&deviceid_raw);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let device = with_repo!(state, |repo| {
        DeviceRepo::find_by_uid(&repo, auth_user.0.id, deviceid).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Normalize URLs
    let add_urls: Vec<String> = body.add.iter().map(|u| normalize_url(u)).collect();
    let remove_urls: Vec<String> = body.remove.iter().map(|u| normalize_url(u)).collect();

    for url in &remove_urls {
        let podcast = with_repo!(state, |repo| PodcastRepo::find_by_url(&repo, url).await)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some(podcast) = podcast {
            with_repo!(state, |repo| {
                SubscriptionRepo::unsubscribe(&repo, auth_user.0.id, device.id, podcast.id).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    for url in &add_urls {
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
        add: add_urls,
        remove: remove_urls,
        timestamp,
    }))
}

// ---------------------------------------------------------------------------
// Advanced API: GET /api/2/subscriptions/{username}/{deviceid}.json?since=T
// ---------------------------------------------------------------------------

pub async fn download_subscription_changes(
    State(state): State<AppState>,
    Path((username, deviceid_raw)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<SinceQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let (deviceid, _) = parse_format_suffix(&deviceid_raw);

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
