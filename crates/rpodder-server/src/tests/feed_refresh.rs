//! Opening a podcast page re-reads a feed that hasn't been checked for a
//! while, so new episodes show up without waiting for the background updater.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{Duration, Utc};

use rpodder_core::repo::{DeviceRepo, PodcastRepo, SubscriptionRepo};

use super::TestApp;

#[derive(Clone, Default)]
struct Feed {
    /// Episodes the feed lists.
    episodes: Arc<AtomicUsize>,
    /// Requests served.
    served: Arc<AtomicUsize>,
    /// Milliseconds to wait before answering.
    delay_ms: Arc<AtomicUsize>,
    /// Requests answered with 304 Not Modified.
    not_modified: Arc<AtomicUsize>,
}

async fn rss(State(feed): State<Feed>, headers: HeaderMap) -> Response {
    feed.served.fetch_add(1, Ordering::SeqCst);
    let delay = feed.delay_ms.load(Ordering::SeqCst) as u64;
    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    // The ETag is the episode count, so an unchanged feed answers 304.
    let etag = format!("\"v{}\"", feed.episodes.load(Ordering::SeqCst));
    if headers.get("if-none-match").and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        feed.not_modified.fetch_add(1, Ordering::SeqCst);
        return (StatusCode::NOT_MODIFIED, [("etag", etag)]).into_response();
    }
    let items: String = (1..=feed.episodes.load(Ordering::SeqCst))
        .map(|n| {
            format!(
                "<item><title>Edition {n}</title><guid>ed-{n}</guid>\
                 <pubDate>Tue, 13 Oct 2026 0{n}:00:00 +0000</pubDate>\
                 <enclosure url=\"http://media.example/ed-{n}.mp3\" type=\"audio/mpeg\" length=\"1\"/></item>"
            )
        })
        .collect();
    (
        [
            ("content-type", "application/rss+xml".to_string()),
            ("etag", etag),
        ],
        format!(
            "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>News</title>\
             <description>Three editions a day</description><link>http://news.example</link>\
             {items}</channel></rss>"
        ),
    )
        .into_response()
}

async fn serve_feed(feed: Feed) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = Router::new().route("/feed.xml", get(rss)).with_state(feed);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}/feed.xml")
}

async fn episode_titles(
    app: &TestApp,
    user: &super::TestUser,
    url: &str,
    page: u32,
) -> (Vec<String>, bool) {
    let uri = format!(
        "/api/2/data/podcast/episodes.json?url={}&page={page}",
        urlencoding::encode(url)
    );
    let (status, body) = app.get(&user.cookie, &uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let titles = body["episodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["title"].as_str().unwrap().to_string())
        .collect();
    (titles, body["refreshing"].as_bool().unwrap_or(false))
}

async fn phone(app: &TestApp, user: &super::TestUser) -> uuid::Uuid {
    let uri = format!("/api/2/devices/{}/phone.json", user.user.username);
    let (status, body) = app
        .post_json(
            &user.cookie,
            &uri,
            serde_json::json!({"caption": "Phone", "type": "mobile"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    DeviceRepo::find_by_uid(&app.repo(), user.user.id, "phone")
        .await
        .unwrap()
        .unwrap()
        .id
}

async fn mark_checked(app: &TestApp, url: &str, ago: Duration) {
    let repo = app.repo();
    let mut podcast = PodcastRepo::find_by_url(&repo, url).await.unwrap().unwrap();
    podcast.last_update = Some(Utc::now() - ago);
    PodcastRepo::update(&repo, &podcast).await.unwrap();
}

#[tokio::test]
async fn opening_a_stale_podcast_reads_new_episodes() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let feed = Feed::default();
    feed.episodes.store(1, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;

    // First visit reads the feed (no metadata yet).
    let (titles, _) = episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(titles, ["Edition 1"]);
    assert_eq!(feed.served.load(Ordering::SeqCst), 1);

    // A new edition is published; the last check was two hours ago.
    feed.episodes.store(2, Ordering::SeqCst);
    mark_checked(&app, &url, Duration::hours(2)).await;

    // Later pages don't trigger a refresh.
    episode_titles(&app, &alice, &url, 1).await;
    assert_eq!(feed.served.load(Ordering::SeqCst), 1);

    // Opening the page does, and returns the new edition.
    let (titles, refreshing) = episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(titles, ["Edition 2", "Edition 1"]);
    assert!(!refreshing);
    assert_eq!(feed.served.load(Ordering::SeqCst), 2);

    // Just checked: no new request.
    episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(feed.served.load(Ordering::SeqCst), 2);

    // Even if the stored check looks stale again (a failed fetch leaves it
    // unchanged), page opens don't retry within minutes.
    mark_checked(&app, &url, Duration::hours(2)).await;
    episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(feed.served.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_slow_feed_finishes_in_the_background() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let feed = Feed::default();
    feed.episodes.store(1, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;
    episode_titles(&app, &alice, &url, 0).await;

    feed.episodes.store(2, Ordering::SeqCst);
    feed.delay_ms.store(5_500, Ordering::SeqCst);
    mark_checked(&app, &url, Duration::hours(2)).await;

    // The page answers after the 4 s wait with what it has.
    let (titles, refreshing) = episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(titles, ["Edition 1"]);
    assert!(refreshing);
    // A second viewer meanwhile is told to check back too.
    let (_, refreshing) = episode_titles(&app, &alice, &url, 0).await;
    assert!(refreshing);

    // Once the update has finished, the new edition is there.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let (titles, refreshing) = episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(titles, ["Edition 2", "Edition 1"]);
    assert!(!refreshing);
    assert_eq!(feed.served.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn an_unchanged_feed_still_gets_a_new_check_interval() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let feed = Feed::default();
    feed.episodes.store(2, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;

    // Read while nobody subscribes: checked weekly.
    episode_titles(&app, &alice, &url, 0).await;
    let repo = app.repo();
    let podcast = PodcastRepo::find_by_url(&repo, &url)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(podcast.update_interval_hours, 168);

    // Alice subscribes; the next check gets a 304.
    let device = phone(&app, &alice).await;
    SubscriptionRepo::subscribe(&repo, alice.user.id, device, podcast.id, &url)
        .await
        .unwrap();
    mark_checked(&app, &url, Duration::hours(2)).await;
    episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(feed.served.load(Ordering::SeqCst), 2);
    assert_eq!(feed.not_modified.load(Ordering::SeqCst), 1);

    let podcast = PodcastRepo::find_by_url(&repo, &url)
        .await
        .unwrap()
        .unwrap();
    assert!(
        podcast.update_interval_hours <= 24,
        "still {}",
        podcast.update_interval_hours
    );
    assert!(podcast.last_update.unwrap() > Utc::now() - Duration::minutes(1));
}
