use axum::{
    Extension,
    body::Body,
    extract::{Json, Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

use rpodder_core::repo::{DeviceRepo, EpisodeActionRepo, PodcastRepo, SubscriptionRepo};
use rpodder_core::types::{EpisodeActionType, SubscriptionAction, SubscriptionChange};
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
    pub update_urls: Vec<(String, String)>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub timestamp: i64,
    pub update_urls: Vec<(String, String)>,
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

/// One `<outline>` per feed. OPML 2.0 requires `text`; it carries the podcast
/// title when the feed has been fetched, the URL otherwise.
fn urls_to_opml(feeds: &[(String, Option<String>)], title: &str) -> String {
    let mut opml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>"#,
    );
    opml.push_str(&xml_escape(title));
    opml.push_str("</title></head>\n  <body>\n");
    for (url, feed_title) in feeds {
        let text = xml_escape(&attribute_text(feed_title.as_deref().unwrap_or(url)));
        opml.push_str(&format!(
            "    <outline type=\"rss\" text=\"{text}\" title=\"{text}\" xmlUrl=\"{}\" />\n",
            xml_escape(url)
        ));
    }
    opml.push_str("  </body>\n</opml>\n");
    opml
}

/// One line of attribute text: whitespace runs (newlines, tabs) become one
/// space and other control characters, which XML 1.0 forbids, are dropped.
fn attribute_text(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect()
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

/// `filename` names the download (no extension); browsers save OPML exports
/// under it instead of a name derived from the URL.
fn respond_urls(
    feeds: &[(String, Option<String>)],
    format: Format,
    title: &str,
    filename: &str,
) -> Response {
    let urls: Vec<String> = feeds.iter().map(|(url, _)| url.clone()).collect();
    match format {
        Format::Json => Json(urls).into_response(),
        Format::Opml => Response::builder()
            .header(header::CONTENT_TYPE, "text/x-opml+xml; charset=utf-8")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.opml\"", download_name(filename)),
            )
            .body(Body::from(urls_to_opml(feeds, title)))
            .unwrap(),
        Format::Txt => Response::builder()
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(urls_to_txt(&urls)))
            .unwrap(),
    }
}

/// A file name safe inside a quoted `Content-Disposition` value.
fn download_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
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

/// Pairs each subscription with its podcast title; titles are looked up only
/// for OPML, the other formats carry URLs alone.
async fn feeds_with_titles(
    state: &AppState,
    subs: Vec<rpodder_core::types::Subscription>,
    format: Format,
) -> Vec<(String, Option<String>)> {
    let titles = if matches!(format, Format::Opml) {
        let ids: Vec<uuid::Uuid> = subs.iter().map(|s| s.podcast_id).collect();
        with_repo!(state, |repo| PodcastRepo::titles_by_ids(&repo, &ids).await).unwrap_or_else(
            |e| {
                tracing::warn!(error = %e, "OPML export: podcast title lookup failed, exporting URLs only");
                Default::default()
            },
        )
    } else {
        Default::default()
    };
    subs.into_iter()
        .map(|sub| {
            let title = titles
                .get(&sub.podcast_id)
                .map(|t| attribute_text(t))
                .filter(|t| !t.is_empty() && *t != sub.ref_url);
            (sub.ref_url, title)
        })
        .collect()
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

    let device = super::helpers::find_or_create_device(&state, auth_user.0.id, deviceid).await?;

    let subs = with_repo!(state, |repo| {
        SubscriptionRepo::list_for_device(&repo, auth_user.0.id, device.id).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let feeds = feeds_with_titles(&state, subs, format).await;
    let title = format!("{username} — {deviceid} subscriptions");
    Ok(respond_urls(
        &feeds,
        format,
        &title,
        &format!("rpodder-{username}-{deviceid}"),
    ))
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

    let device = super::helpers::find_or_create_device(&state, auth_user.0.id, deviceid).await?;

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

    let feeds = feeds_with_titles(&state, subs, format).await;
    let title = format!("{username} subscriptions");
    Ok(respond_urls(
        &feeds,
        format,
        &title,
        &format!("rpodder-{username}"),
    ))
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

    let device = super::helpers::find_or_create_device(&state, auth_user.0.id, deviceid).await?;

    // Taken before writing, with slack (see helpers::SYNC_CURSOR_SLACK_SECS).
    let timestamp = super::helpers::sync_cursor();

    // Normalize URLs and track mappings for update_urls response
    let add_urls: Vec<String> = body.add.iter().map(|u| normalize_url(u)).collect();
    let remove_urls: Vec<String> = body.remove.iter().map(|u| normalize_url(u)).collect();

    let mut update_urls: Vec<(String, String)> = Vec::new();
    for (orig, normalized) in body.add.iter().zip(&add_urls) {
        if orig != normalized {
            update_urls.push((orig.clone(), normalized.clone()));
        }
    }
    for (orig, normalized) in body.remove.iter().zip(&remove_urls) {
        if orig != normalized {
            update_urls.push((orig.clone(), normalized.clone()));
        }
    }

    for url in &remove_urls {
        let podcast = with_repo!(state, |repo| PodcastRepo::find_by_url(&repo, url).await)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if let Some(podcast) = podcast {
            with_repo!(state, |repo| {
                SubscriptionRepo::unsubscribe(&repo, auth_user.0.id, device.id, podcast.id).await
            })
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // Propagate to synced devices
            super::helpers::propagate_to_sync_group(
                &state,
                auth_user.0.id,
                &device,
                podcast.id,
                url,
                false,
            )
            .await;
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

        // Propagate to synced devices
        super::helpers::propagate_to_sync_group(
            &state,
            auth_user.0.id,
            &device,
            podcast.id,
            url,
            true,
        )
        .await;
    }

    Ok(Json(UploadResponse {
        timestamp,
        update_urls,
    }))
}

/// Collapse a change log (ordered oldest first) to the latest action per
/// podcast, like mygpo's subscription diff: a URL never appears in both lists,
/// even when the log holds a remove and a re-add within the requested window.
/// A repeated unsubscribe is logged with an empty `ref_url`; it keeps the
/// podcast's last known URL rather than replacing it.
fn split_changes(changes: Vec<SubscriptionChange>) -> (Vec<String>, Vec<String>) {
    let mut order: Vec<uuid::Uuid> = Vec::new();
    let mut latest: std::collections::HashMap<uuid::Uuid, (SubscriptionAction, String)> =
        std::collections::HashMap::new();
    for change in changes {
        match latest.get_mut(&change.podcast_id) {
            Some(entry) => {
                entry.0 = change.action;
                if !change.ref_url.is_empty() {
                    entry.1 = change.ref_url;
                }
            }
            None => {
                order.push(change.podcast_id);
                latest.insert(change.podcast_id, (change.action, change.ref_url));
            }
        }
    }

    let mut add = Vec::new();
    let mut remove = Vec::new();
    for (action, url) in order.into_iter().filter_map(|id| latest.remove(&id)) {
        if url.is_empty() {
            continue;
        }
        match action {
            SubscriptionAction::Subscribe => add.push(url),
            SubscriptionAction::Unsubscribe => remove.push(url),
        }
    }
    (add, remove)
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

    let device = super::helpers::find_or_create_device(&state, auth_user.0.id, deviceid).await?;

    // Taken before querying, with slack (see helpers::SYNC_CURSOR_SLACK_SECS).
    let timestamp = super::helpers::sync_cursor();

    let since = params
        .since
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap());

    let changes = with_repo!(state, |repo| {
        SubscriptionRepo::changes_since(&repo, auth_user.0.id, device.id, since).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (add, remove) = split_changes(changes);

    Ok(Json(DeltaResponse {
        add,
        remove,
        update_urls: vec![],
        timestamp,
    }))
}

// ---------------------------------------------------------------------------
// Combined updates: GET /api/2/updates/{username}/{deviceid}.json?since=T
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CombinedUpdatesResponse {
    pub add: Vec<String>,
    pub remove: Vec<String>,
    pub updates: Vec<EpisodeActionUpdate>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct EpisodeActionUpdate {
    pub podcast: String,
    pub episode: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
}

pub async fn combined_updates(
    State(state): State<AppState>,
    Path((username, deviceid_raw)): Path<(String, String)>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<SinceQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let (deviceid, _) = parse_format_suffix(&deviceid_raw);

    if auth_user.0.username.to_lowercase() != username.to_lowercase() {
        return Err(StatusCode::FORBIDDEN);
    }

    let device = super::helpers::find_or_create_device(&state, auth_user.0.id, deviceid).await?;

    // Taken before querying, with slack (see helpers::SYNC_CURSOR_SLACK_SECS).
    let timestamp = super::helpers::sync_cursor();

    let since = params
        .since
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap());

    // Subscription changes
    let changes = with_repo!(state, |repo| {
        SubscriptionRepo::changes_since(&repo, auth_user.0.id, device.id, since).await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (add, remove) = split_changes(changes);

    // Episode actions
    let actions = with_repo!(state, |repo| {
        EpisodeActionRepo::list(
            &repo,
            auth_user.0.id,
            Some(device.id),
            None,
            Some(since),
            None,
        )
        .await
    })
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let updates: Vec<EpisodeActionUpdate> = actions
        .into_iter()
        .map(|a| EpisodeActionUpdate {
            podcast: a.podcast_ref_url.unwrap_or_default(),
            episode: a.episode_ref_url.unwrap_or_default(),
            action: match a.action {
                EpisodeActionType::Download => "download",
                EpisodeActionType::Play => "play",
                EpisodeActionType::Delete => "delete",
                EpisodeActionType::New => "new",
            }
            .to_string(),
            timestamp: Some(a.timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()),
            position: a.position,
            total: a.total,
        })
        .collect();

    Ok(Json(CombinedUpdatesResponse {
        add,
        remove,
        updates,
        timestamp,
    }))
}

// ---------------------------------------------------------------------------
// rpodder: GET /api/2/me/holdings[?podcast=URL]
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct HoldingsQuery {
    pub podcast: Option<String>,
}

/// One podcast and the user's devices subscribed to it. `urls` are the URLs
/// those devices subscribed with (plus the queried one): aliases of a single
/// feed, which a client comparing URLs alone would take for different feeds.
#[derive(Debug, Serialize)]
pub struct Holding {
    pub urls: Vec<String>,
    pub devices: Vec<String>,
}

pub async fn holdings(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthUser>,
    Query(params): Query<HoldingsQuery>,
) -> Result<Json<Vec<Holding>>, StatusCode> {
    let user_id = auth_user.0.id;
    let devices = with_repo!(state, |repo| DeviceRepo::list_for_user(&repo, user_id)
        .await)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut held = Vec::new();
    for device in &devices {
        let subs = with_repo!(state, |repo| {
            SubscriptionRepo::list_for_device(&repo, user_id, device.id).await
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        held.extend(subs.into_iter().map(|sub| (&device.device_id, sub)));
    }

    // Resolved like a subscription upload: removing this URL from the devices
    // listed unsubscribes every one of them.
    let wanted = match params.podcast.as_deref().map(normalize_url) {
        None => None,
        Some(url) => {
            let podcast = with_repo!(state, |repo| PodcastRepo::find_by_url(&repo, &url).await)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let Some(podcast) = podcast else {
                return Ok(Json(Vec::new()));
            };
            Some((podcast.id, url))
        }
    };

    let mut holdings: Vec<Holding> = Vec::new();
    let mut index: std::collections::HashMap<uuid::Uuid, usize> = Default::default();
    for (device_id, sub) in held {
        if wanted.as_ref().is_some_and(|(id, _)| *id != sub.podcast_id) {
            continue;
        }
        let i = *index.entry(sub.podcast_id).or_insert_with(|| {
            holdings.push(Holding {
                urls: Vec::new(),
                devices: Vec::new(),
            });
            holdings.len() - 1
        });
        let holding = &mut holdings[i];
        if !holding.urls.contains(&sub.ref_url) {
            holding.urls.push(sub.ref_url);
        }
        if !holding.devices.contains(device_id) {
            holding.devices.push(device_id.clone());
        }
    }

    if let Some((_, url)) = wanted {
        for holding in &mut holdings {
            if !holding.urls.contains(&url) {
                holding.urls.push(url.clone());
            }
        }
    }
    Ok(Json(holdings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opml_outlines_carry_titles_and_escape_them() {
        let feeds = vec![
            (
                "http://a.example/feed?x=1&y=2".to_string(),
                Some("Tom & \"Jerry\" <live>".to_string()),
            ),
            ("http://b.example/rss".to_string(), None),
        ];
        let opml = urls_to_opml(&feeds, "alice subscriptions");
        assert!(
            opml.contains(r#"<outline type="rss" text="Tom &amp; &quot;Jerry&quot; &lt;live&gt;" title="Tom &amp; &quot;Jerry&quot; &lt;live&gt;" xmlUrl="http://a.example/feed?x=1&amp;y=2" />"#),
            "{opml}"
        );
        // No title yet: the URL stands in for the required text attribute.
        assert!(
            opml.contains(r#"text="http://b.example/rss" title="http://b.example/rss" xmlUrl="http://b.example/rss""#),
            "{opml}"
        );
        // The parser used for uploads reads the export back.
        assert_eq!(
            opml_to_urls(&opml),
            vec!["http://a.example/feed?x=1&y=2", "http://b.example/rss"]
        );
    }

    #[test]
    fn opml_titles_stay_on_one_line_without_control_characters() {
        let feeds = vec![(
            "http://c.example/rss".to_string(),
            Some("Line one\n\tLine\u{0007} two\r\n".to_string()),
        )];
        let opml = urls_to_opml(&feeds, "t");
        assert!(
            opml.contains(r#"text="Line one Line two" title="Line one Line two""#),
            "{opml}"
        );
        assert_eq!(opml_to_urls(&opml), vec!["http://c.example/rss"]);
    }

    #[test]
    fn download_name_strips_unsafe_characters() {
        assert_eq!(
            download_name("rpodder-alice-pixel.7"),
            "rpodder-alice-pixel.7"
        );
        assert_eq!(download_name("rpodder-a\"b c/d"), "rpodder-a_b_c_d");
    }
}
