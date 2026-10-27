//! `/api/2/me/holdings`: which of the user's devices hold each podcast,
//! grouped by podcast rather than by the URL each device subscribed with.

use std::collections::BTreeSet;

use axum::{Router, routing::get};
use serde_json::{Value, json};

use rpodder_core::repo::PodcastRepo;
use rpodder_core::url::normalize_url;

use super::{TestApp, TestUser};

const FEED: &str = "http://example.com/feed.rss";
const ALIAS: &str = "http://example.com/feed";
const UNUSED_ALIAS: &str = "http://example.com/feed.xml";
const OTHER: &str = "http://example.com/other.xml";

fn set(v: &Value) -> BTreeSet<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn strs(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

async fn change(app: &TestApp, user: &TestUser, device: &str, add: &[&str], remove: &[&str]) {
    let (status, body) = app
        .post_json(
            &user.cookie,
            &format!("/api/2/subscriptions/{}/{device}.json", user.user.username),
            json!({"add": add, "remove": remove}),
        )
        .await;
    assert_eq!(status, 200, "{body}");
}

/// Devices holding any podcast `url` names.
async fn devices_holding(app: &TestApp, user: &TestUser, url: &str) -> BTreeSet<String> {
    let (status, body) = app
        .get(
            &user.cookie,
            &format!("/api/2/me/holdings?podcast={}", urlencoding::encode(url)),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let mut devices = BTreeSet::new();
    for holding in body.as_array().unwrap() {
        assert!(
            set(&holding["urls"]).contains(&normalize_url(url)),
            "{url}: {body}"
        );
        devices.extend(set(&holding["devices"]));
    }
    devices
}

#[tokio::test]
async fn aliases_of_one_podcast_are_one_holding() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let repo = app.repo();
    let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, FEED)
        .await
        .unwrap();
    PodcastRepo::add_url(&repo, podcast.id, ALIAS)
        .await
        .unwrap();
    PodcastRepo::add_url(&repo, podcast.id, UNUSED_ALIAS)
        .await
        .unwrap();

    change(&app, &alice, "phone", &[FEED], &[]).await;
    change(&app, &alice, "laptop", &[ALIAS, OTHER], &[]).await;

    let (status, body) = app.get(&alice.cookie, "/api/2/me/holdings").await;
    assert_eq!(status, 200, "{body}");
    let all = body.as_array().unwrap();
    assert_eq!(all.len(), 2, "{body}");
    let feed = all
        .iter()
        .find(|h| set(&h["urls"]).contains(FEED))
        .unwrap_or_else(|| panic!("no holding for {FEED}: {body}"));
    assert_eq!(set(&feed["urls"]), strs(&[FEED, ALIAS]), "{body}");
    assert_eq!(set(&feed["devices"]), strs(&["phone", "laptop"]), "{body}");

    // A page opened on an alias no device subscribed with still finds them.
    assert_eq!(
        devices_holding(&app, &alice, UNUSED_ALIAS).await,
        strs(&["phone", "laptop"])
    );

    // Removing the page's URL unsubscribes a device that used another alias.
    change(&app, &alice, "laptop", &[], &[UNUSED_ALIAS]).await;
    assert_eq!(devices_holding(&app, &alice, FEED).await, strs(&["phone"]));
}

#[tokio::test]
async fn holdings_are_per_user_and_need_a_login() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let bob = app.create_user("bob").await;
    change(&app, &alice, "phone", &[FEED], &[]).await;

    let (status, body) = app.get(&bob.cookie, "/api/2/me/holdings").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, json!([]));
    let (_, body) = app
        .get(&bob.cookie, &format!("/api/2/me/holdings?podcast={FEED}"))
        .await;
    assert_eq!(body, json!([]));

    let (_, body) = app
        .get(
            &alice.cookie,
            "/api/2/me/holdings?podcast=http://unknown.example/feed",
        )
        .await;
    assert_eq!(body, json!([]));

    let (status, _) = app.get("", "/api/2/me/holdings").await;
    assert_eq!(status, 401);
}

const SHOW: &str = "<?xml version=\"1.0\"?><rss version=\"2.0\"><channel><title>Show</title>\
    <description>One feed, two addresses</description>\
    <item><title>One</title><guid>one</guid>\
    <pubDate>Tue, 13 Oct 2026 08:00:00 +0000</pubDate>\
    <enclosure url=\"http://show.example/one.mp3\" type=\"audio/mpeg\" length=\"1\"/></item>\
    </channel></rss>";

/// The same feed served at `/a.xml` and `/a`.
async fn serve_twice() -> (String, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/a.xml", get(|| async { SHOW }))
        .route("/a", get(|| async { SHOW }));
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("http://{addr}/a.xml"), format!("http://{addr}/a"))
}

/// Reading a second address of a known feed moves its subscriptions to the
/// first podcast: one holding, both devices.
#[tokio::test]
async fn a_merged_subscription_joins_its_podcast() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let (first, second) = serve_twice().await;
    let fetcher = rpodder_feed::FeedFetcher::new();

    change(&app, &alice, "phone", &[&first], &[]).await;
    crate::feed_updater::update_podcast_feed_forced(&app.db, &fetcher, &first)
        .await
        .unwrap();
    change(&app, &alice, "laptop", &[&second], &[]).await;
    let outcome = crate::feed_updater::update_podcast_feed_forced(&app.db, &fetcher, &second)
        .await
        .unwrap();
    assert_eq!(outcome, crate::feed_updater::FeedOutcome::Merged);

    let (_, body) = app.get(&alice.cookie, "/api/2/me/holdings").await;
    let all = body.as_array().unwrap();
    assert_eq!(all.len(), 1, "{body}");
    assert_eq!(set(&all[0]["urls"]), strs(&[&first, &second]), "{body}");
    assert_eq!(
        devices_holding(&app, &alice, &first).await,
        strs(&["phone", "laptop"])
    );
}
