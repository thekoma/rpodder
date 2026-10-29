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
                 <link>http://news.example/ed-{n}</link>\
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

#[tokio::test]
async fn admin_refresh_rereads_the_feed_and_reports_new_episodes() {
    let app = TestApp::new().await;
    let admin = app.create_user("admin").await;
    rpodder_core::repo::UserRepo::set_admin(&app.repo(), admin.user.id, true)
        .await
        .unwrap();
    let feed = Feed::default();
    feed.episodes.store(1, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;
    episode_titles(&app, &admin, &url, 0).await;

    let refresh = format!(
        "/api/admin/feeds/update/single?url={}",
        urlencoding::encode(&url)
    );
    // Unchanged feed: still downloaded in full (no 304), nothing new.
    let (status, body) = app
        .post_json(&admin.cookie, &refresh, serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "updated");
    assert_eq!(body["new_episodes"], 0);
    assert_eq!(feed.not_modified.load(Ordering::SeqCst), 0);

    feed.episodes.store(3, Ordering::SeqCst);
    let (status, body) = app
        .post_json(&admin.cookie, &refresh, serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["new_episodes"], 2);
    let (titles, _) = episode_titles(&app, &admin, &url, 0).await;
    assert_eq!(titles.len(), 3);
}

#[tokio::test]
async fn copies_stored_under_the_web_page_are_removed() {
    use rpodder_core::repo::EpisodeRepo;

    let app = TestApp::new().await;
    let admin = app.create_user("admin").await;
    rpodder_core::repo::UserRepo::set_admin(&app.repo(), admin.user.id, true)
        .await
        .unwrap();
    let feed = Feed::default();
    feed.episodes.store(2, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;
    episode_titles(&app, &admin, &url, 0).await;

    // What 2026.10.1-2026.10.2 stored when it missed the enclosure: the same
    // item (guid ed-1) under its web page, with no media type.
    let repo = app.repo();
    let podcast = PodcastRepo::find_by_url(&repo, &url)
        .await
        .unwrap()
        .unwrap();
    let (mut copy, _) =
        EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://news.example/ed-1")
            .await
            .unwrap();
    copy.title = "Edition 1".to_string();
    copy.guid = Some("ed-1".to_string());
    copy.mimetype = None;
    EpisodeRepo::update(&repo, &copy).await.unwrap();
    let (titles, _) = episode_titles(&app, &admin, &url, 0).await;
    assert_eq!(titles.len(), 3, "the copy is listed before the cleanup");

    let refresh = format!(
        "/api/admin/feeds/update/single?url={}",
        urlencoding::encode(&url)
    );
    let (status, body) = app
        .post_json(&admin.cookie, &refresh, serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (titles, _) = episode_titles(&app, &admin, &url, 0).await;
    assert_eq!(titles, ["Edition 2", "Edition 1"]);
}

#[tokio::test]
async fn a_moved_enclosure_keeps_its_old_episode() {
    use rpodder_core::repo::EpisodeRepo;

    let app = TestApp::new().await;
    let admin = app.create_user("admin").await;
    rpodder_core::repo::UserRepo::set_admin(&app.repo(), admin.user.id, true)
        .await
        .unwrap();
    let feed = Feed::default();
    feed.episodes.store(1, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;
    episode_titles(&app, &admin, &url, 0).await;

    // Same guid, no media type, but stored under another audio address (say
    // the feed moved CDN): not a web-page copy, so it must stay with its history.
    let repo = app.repo();
    let podcast = PodcastRepo::find_by_url(&repo, &url)
        .await
        .unwrap()
        .unwrap();
    let (mut old, _) =
        EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://old-cdn.example/ed-1.mp3")
            .await
            .unwrap();
    old.title = "Edition 1 (old address)".to_string();
    old.guid = Some("ed-1".to_string());
    old.mimetype = None;
    EpisodeRepo::update(&repo, &old).await.unwrap();

    let refresh = format!(
        "/api/admin/feeds/update/single?url={}",
        urlencoding::encode(&url)
    );
    let (status, body) = app
        .post_json(&admin.cookie, &refresh, serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (titles, _) = episode_titles(&app, &admin, &url, 0).await;
    assert_eq!(titles.len(), 2, "{titles:?}");
}

async fn serve_xml(xml: String) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/feed.xml",
        get(move || {
            let xml = xml.clone();
            async move { xml }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}/feed.xml")
}

#[tokio::test]
async fn a_text_item_sharing_a_guid_and_link_is_kept() {
    let app = TestApp::new().await;
    let admin = app.create_user("admin").await;
    rpodder_core::repo::UserRepo::set_admin(&app.repo(), admin.user.id, true)
        .await
        .unwrap();
    // Both items carry the same guid and <link>; only the first has audio.
    let url = serve_xml(
        "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Shared</title>\
         <description>Same guid twice</description>\
         <item><title>Audio</title><guid>same</guid><link>http://show.example/</link>\
         <pubDate>Tue, 13 Oct 2026 08:00:00 +0000</pubDate>\
         <enclosure url=\"http://show.example/a.mp3\" type=\"audio/mpeg\" length=\"1\"/></item>\
         <item><title>Notes</title><guid>same</guid><link>http://show.example/</link>\
         <pubDate>Tue, 13 Oct 2026 07:00:00 +0000</pubDate></item>\
         </channel></rss>"
            .to_string(),
    )
    .await;
    episode_titles(&app, &admin, &url, 0).await;

    let refresh = format!(
        "/api/admin/feeds/update/single?url={}",
        urlencoding::encode(&url)
    );
    for _ in 0..2 {
        let (status, body) = app
            .post_json(&admin.cookie, &refresh, serde_json::json!({}))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["new_episodes"], 0, "no churn: {body}");
    }
    let (titles, _) = episode_titles(&app, &admin, &url, 0).await;
    assert_eq!(titles, ["Audio", "Notes"]);
}

#[tokio::test]
async fn an_admin_reread_shows_as_running_on_a_fresh_podcast() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let feed = Feed::default();
    feed.episodes.store(1, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;
    episode_titles(&app, &alice, &url, 0).await;
    let podcast = PodcastRepo::find_by_url(&app.repo(), &url)
        .await
        .unwrap()
        .unwrap();

    // Checked just now, but an admin re-read is running.
    let done = crate::feed_updater::track_refresh(podcast.id);
    let (_, refreshing) = episode_titles(&app, &alice, &url, 0).await;
    assert!(refreshing);
    assert_eq!(feed.served.load(Ordering::SeqCst), 1, "no second fetch");

    done.store(true, Ordering::SeqCst);
    let (_, refreshing) = episode_titles(&app, &alice, &url, 0).await;
    assert!(!refreshing);
}

/// A site that serves its own feed the way Radio Capital does: no image, and
/// (when `echo`) the URL it was requested with as the channel <link>. Its
/// home page is `home` (counted in `home_hits`); `/favicon.ico` is an icon
/// when `favicon`.
struct ArtlessSite {
    home: &'static str,
    echo: bool,
    favicon: bool,
    home_hits: Arc<AtomicUsize>,
}

async fn serve_artless_site(site: ArtlessSite) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let link = if site.echo {
        format!("<link>{base}/feed.xml</link>")
    } else {
        String::new()
    };
    let feed_base = base.clone();
    let feed = move || {
        let (base, link) = (feed_base.clone(), link.clone());
        async move {
            (
                [("content-type", "application/rss+xml")],
                format!(
                    "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel>\
                     <title>Site News</title><description>Bulletins</description>{link}\
                     <item><title>Ed 1</title><guid>e1</guid>\
                     <pubDate>Tue, 27 Oct 2026 09:00:00 +0000</pubDate>\
                     <enclosure url=\"{base}/e1.mp3\" type=\"audio/mpeg\" length=\"1\"/></item>\
                     </channel></rss>"
                ),
            )
        }
    };
    let hits = site.home_hits.clone();
    let home = site.home;
    let favicon = site.favicon;
    let router = Router::new()
        .route("/feed.xml", get(feed))
        .route(
            "/",
            get(move || {
                hits.fetch_add(1, Ordering::SeqCst);
                async move { ([("content-type", "text/html; charset=utf-8")], home) }
            }),
        )
        .route(
            "/favicon.ico",
            get(move || async move {
                if favicon {
                    ([("content-type", "image/x-icon")], "ico").into_response()
                } else {
                    StatusCode::NOT_FOUND.into_response()
                }
            }),
        );
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    base
}

async fn podcast_json(app: &TestApp, user: &super::TestUser, url: &str) -> serde_json::Value {
    let uri = format!(
        "/api/2/data/podcast/episodes.json?url={}",
        urlencoding::encode(url)
    );
    let (status, body) = app.get(&user.cookie, &uri).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["podcast"].clone()
}

#[tokio::test]
async fn a_feed_without_art_gets_its_site_and_icon() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let home_hits = Arc::new(AtomicUsize::new(0));
    let base = serve_artless_site(ArtlessSite {
        home: "<html><head><link rel=\"icon\" href=\"/favicon.ico\">\
               <link rel=\"apple-touch-icon\" href=\"/touch.png\"></head></html>",
        echo: true,
        favicon: true,
        home_hits: home_hits.clone(),
    })
    .await;
    let url = format!("{base}/feed.xml");

    let podcast = podcast_json(&app, &alice, &url).await;
    assert_eq!(podcast["website"], format!("{base}/"), "{podcast}");
    assert_eq!(
        podcast["logo_url"],
        format!("{base}/touch.png"),
        "{podcast}"
    );
    assert_eq!(home_hits.load(Ordering::SeqCst), 1);

    // Later reads keep the icon without asking the site again (daily at most).
    mark_checked(&app, &url, Duration::hours(2)).await;
    let podcast = podcast_json(&app, &alice, &url).await;
    assert_eq!(podcast["logo_url"], format!("{base}/touch.png"));
    assert_eq!(home_hits.load(Ordering::SeqCst), 1);

    // An admin's Refresh feed looks at the site again.
    rpodder_core::repo::UserRepo::set_admin(&app.repo(), alice.user.id, true)
        .await
        .unwrap();
    let refresh = format!(
        "/api/admin/feeds/update/single?url={}",
        urlencoding::encode(&url)
    );
    let (status, body) = app
        .post_json(&alice.cookie, &refresh, serde_json::json!({}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(home_hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_site_without_declared_icons_falls_back_to_its_favicon() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let base = serve_artless_site(ArtlessSite {
        home: "<html><head><title>No icons</title></head></html>",
        echo: true,
        favicon: true,
        home_hits: Arc::new(AtomicUsize::new(0)),
    })
    .await;
    let podcast = podcast_json(&app, &alice, &format!("{base}/feed.xml")).await;
    assert_eq!(
        podcast["logo_url"],
        format!("{base}/favicon.ico"),
        "{podcast}"
    );
}

#[tokio::test]
async fn no_link_means_no_website_and_no_lookup() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let home_hits = Arc::new(AtomicUsize::new(0));
    let base = serve_artless_site(ArtlessSite {
        home: "<link rel=\"apple-touch-icon\" href=\"/touch.png\">",
        echo: false,
        favicon: true,
        home_hits: home_hits.clone(),
    })
    .await;
    let podcast = podcast_json(&app, &alice, &format!("{base}/feed.xml")).await;
    assert!(podcast["website"].is_null(), "{podcast}");
    assert!(podcast["logo_url"].is_null(), "{podcast}");
    assert_eq!(home_hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_feed_that_loses_its_art_keeps_its_cover() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    // A site with only a favicon: not enough to replace a real cover.
    let base = serve_artless_site(ArtlessSite {
        home: "<html></html>",
        echo: true,
        favicon: true,
        home_hits: Arc::new(AtomicUsize::new(0)),
    })
    .await;
    let url = format!("{base}/feed.xml");
    podcast_json(&app, &alice, &url).await;
    // The feed used to carry art.
    let repo = app.repo();
    let mut podcast = PodcastRepo::find_by_url(&repo, &url)
        .await
        .unwrap()
        .unwrap();
    podcast.logo_url = Some("https://cdn.example/old-cover.jpg".to_string());
    podcast.last_update = Some(Utc::now() - Duration::hours(2));
    PodcastRepo::update(&repo, &podcast).await.unwrap();

    let podcast = podcast_json(&app, &alice, &url).await;
    assert_eq!(
        podcast["logo_url"], "https://cdn.example/old-cover.jpg",
        "{podcast}"
    );
}

#[tokio::test]
async fn a_link_to_another_local_service_is_not_fetched() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    // An internal service the feed's <link> points at (another loopback port).
    let internal_hits = Arc::new(AtomicUsize::new(0));
    let internal = serve_artless_site(ArtlessSite {
        home: "<link rel=\"apple-touch-icon\" href=\"/touch.png\">",
        echo: false,
        favicon: true,
        home_hits: internal_hits.clone(),
    })
    .await;
    let url = serve_xml(format!(
        "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Probe</title>\
         <description>Points elsewhere</description><link>{internal}/</link>\
         <item><title>E</title><guid>p1</guid>\
         <enclosure url=\"http://media.example/p1.mp3\" type=\"audio/mpeg\" length=\"1\"/></item>\
         </channel></rss>"
    ))
    .await;

    let podcast = podcast_json(&app, &alice, &url).await;
    assert_eq!(podcast["website"], format!("{internal}/"), "{podcast}");
    assert!(podcast["logo_url"].is_null(), "{podcast}");
    assert_eq!(internal_hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn anonymous_data_requests_never_fetch_a_url() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let feed = Feed::default();
    feed.episodes.store(1, Ordering::SeqCst);
    let url = serve_feed(feed.clone()).await;
    let enc = urlencoding::encode(&url);

    // An unknown URL is neither fetched nor stored for an anonymous caller.
    for uri in [
        format!("/api/2/data/podcast.json?url={enc}"),
        format!("/api/2/data/podcast/episodes.json?url={enc}"),
    ] {
        let (status, _) = app.get("", &uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    assert_eq!(feed.served.load(Ordering::SeqCst), 0);
    assert!(
        PodcastRepo::find_by_url(&app.repo(), &url)
            .await
            .unwrap()
            .is_none()
    );

    // A logged-in user still adds it, and anyone can then read it.
    episode_titles(&app, &alice, &url, 0).await;
    assert_eq!(feed.served.load(Ordering::SeqCst), 1);
    let (status, body) = app
        .get("", &format!("/api/2/data/podcast.json?url={enc}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["title"], "News");
    assert_eq!(feed.served.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_url_whose_first_read_fails_is_not_kept() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    // Nothing listens there.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/feed.xml", listener.local_addr().unwrap());
    drop(listener);

    let uri = format!("/api/2/data/podcast.json?url={}", urlencoding::encode(&url));
    let (status, _) = app.get(&alice.cookie, &uri).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        PodcastRepo::find_by_url(&app.repo(), &url)
            .await
            .unwrap()
            .is_none()
    );
}
