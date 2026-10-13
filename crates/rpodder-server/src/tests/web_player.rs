//! Web player endpoints: media URLs in the episode list, "continue listening"
//! (`/api/2/me/in-progress`) and per-podcast positions (`/api/2/me/positions`).

use chrono::{Duration, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use rpodder_core::repo::{EpisodeRepo, PodcastRepo};
use rpodder_core::types::{Episode, Podcast};
use rpodder_db::Db;

use super::{TestApp, TestUser};

const FEED: &str = "http://example.com/feed.xml";
const OTHER_FEED: &str = "http://example.com/other.xml";

async fn podcast(app: &TestApp, url: &str) -> Podcast {
    let repo = app.repo();
    let (mut p, _) = PodcastRepo::get_or_create_for_url(&repo, url)
        .await
        .unwrap();
    // A title, a description and a recent check keep episodes.json from
    // fetching the feed.
    p.title = format!("Podcast {url}");
    p.description = "A podcast".to_string();
    p.last_update = Some(chrono::Utc::now());
    PodcastRepo::update(&repo, &p).await.unwrap();
    p
}

async fn episode(
    app: &TestApp,
    podcast: &Podcast,
    url: &str,
    title: &str,
    mimetype: Option<&str>,
    link: Option<&str>,
    duration: Option<i64>,
) -> Episode {
    let repo = app.repo();
    let (mut e, _) = EpisodeRepo::get_or_create_for_url(&repo, podcast.id, url)
        .await
        .unwrap();
    e.title = title.to_string();
    e.mimetype = mimetype.map(str::to_string);
    e.link = link.map(str::to_string);
    e.duration = duration;
    e.released = Some(Utc::now());
    EpisodeRepo::update(&repo, &e).await.unwrap();
    e
}

fn ts(ago: Duration) -> String {
    (Utc::now() - ago).format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn action(
    feed: &str,
    episode: &str,
    device: &str,
    kind: &str,
    position: Option<i32>,
    total: Option<i32>,
    ago: Duration,
) -> Value {
    let mut a = json!({
        "podcast": feed,
        "episode": episode,
        "device": device,
        "action": kind,
        "timestamp": ts(ago),
    });
    if let Some(p) = position {
        a["started"] = json!(0);
        a["position"] = json!(p);
    }
    if let Some(t) = total {
        a["total"] = json!(t);
    }
    a
}

async fn upload(app: &TestApp, user: &TestUser, actions: Vec<Value>) {
    let uri = format!("/api/2/episodes/{}.json", user.user.username);
    let (status, body) = app.post_json(&user.cookie, &uri, json!(actions)).await;
    assert_eq!(status, 200, "{body}");
}

fn episode_urls(body: &Value) -> Vec<&str> {
    body.as_array()
        .unwrap()
        .iter()
        .map(|i| i["episode_url"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn episodes_list_includes_primary_media_url() {
    let app = TestApp::new().await;
    let p = podcast(&app, FEED).await;
    let a = episode(
        &app,
        &p,
        "http://example.com/a.mp3",
        "A",
        Some("audio/mpeg"),
        Some("http://example.com/a"),
        None,
    )
    .await;
    episode(
        &app,
        &p,
        "http://example.com/b.html",
        "B",
        None,
        Some("http://example.com/b.html"),
        None,
    )
    .await;
    let Db::Sqlite(pool) = &*app.db else {
        panic!("expected sqlite")
    };
    sqlx::query(r#"INSERT INTO episode_urls (id, episode_id, url, "order") VALUES (?, ?, ?, 1)"#)
        .bind(Uuid::now_v7().to_string())
        .bind(a.id.to_string())
        .bind("http://example.com/a-alt.mp3")
        .execute(pool)
        .await
        .unwrap();

    let user = app.create_user("alice").await;
    let (status, body) = app
        .get(
            &user.cookie,
            &format!("/api/2/data/podcast/episodes.json?url={FEED}"),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let episodes = body["episodes"].as_array().unwrap();
    let by_title = |t: &str| episodes.iter().find(|e| e["title"] == t).unwrap().clone();
    assert_eq!(by_title("A")["url"], "http://example.com/a.mp3");
    assert_eq!(by_title("A")["media"], true);
    assert_eq!(by_title("B")["url"], "http://example.com/b.html");
    assert_eq!(by_title("B")["media"], false);
}

#[tokio::test]
async fn in_progress_returns_latest_unfinished_play_per_episode() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let p = podcast(&app, FEED).await;
    let ep = |n: u32| format!("http://example.com/ep{n}.mp3");
    for n in 1..=7 {
        let duration = match n {
            5 => Some(120),
            _ => None,
        };
        episode(
            &app,
            &p,
            &ep(n),
            &format!("Episode {n}"),
            Some("audio/mpeg"),
            None,
            duration,
        )
        .await;
    }
    let h = Duration::hours;
    upload(
        &app,
        &alice,
        vec![
            action(FEED, &ep(1), "phone", "play", Some(100), Some(3600), h(2)),
            action(FEED, &ep(1), "tablet", "play", Some(200), Some(3600), h(1)),
            action(
                FEED,
                &ep(1),
                "phone",
                "download",
                None,
                None,
                Duration::minutes(30),
            ),
            action(FEED, &ep(2), "phone", "play", Some(3590), Some(3600), h(1)),
            action(FEED, &ep(3), "phone", "play", Some(0), Some(3600), h(1)),
            action(FEED, &ep(4), "phone", "download", None, None, h(1)),
            action(FEED, &ep(5), "phone", "play", Some(100), None, h(1)),
            action(FEED, &ep(6), "phone", "play", Some(50), None, h(4)),
            action(FEED, &ep(7), "phone", "play", Some(500), Some(3600), h(3)),
            action(FEED, &ep(7), "phone", "new", None, None, h(2)),
        ],
    )
    .await;

    let (status, body) = app.get(&alice.cookie, "/api/2/me/in-progress").await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(episode_urls(&body), vec![ep(1), ep(6)], "{body}");
    let first = &body[0];
    assert_eq!(first["position"], 200);
    assert_eq!(first["total"], 3600);
    assert_eq!(first["device"], "tablet");
    assert_eq!(first["episode_title"], "Episode 1");
    assert_eq!(first["podcast_url"], FEED);
    assert_eq!(first["mimetype"], "audio/mpeg");
    assert_eq!(body[1]["total"], Value::Null);
}

#[tokio::test]
async fn in_progress_titles_null_when_equal_to_url() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    // Podcast and episode only known from the action upload: titles are URLs.
    upload(
        &app,
        &alice,
        vec![action(
            OTHER_FEED,
            "http://example.com/unknown.mp3",
            "phone",
            "play",
            Some(10),
            Some(600),
            Duration::hours(1),
        )],
    )
    .await;
    let (_, body) = app.get(&alice.cookie, "/api/2/me/in-progress").await;
    assert_eq!(
        body[0]["episode_url"], "http://example.com/unknown.mp3",
        "{body}"
    );
    assert_eq!(body[0]["episode_title"], Value::Null, "{body}");
    assert_eq!(body[0]["mimetype"], Value::Null, "{body}");
}

#[tokio::test]
async fn in_progress_is_scoped_and_authenticated() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let bob = app.create_user("bob").await;
    upload(
        &app,
        &bob,
        vec![action(
            FEED,
            "http://example.com/bob.mp3",
            "phone",
            "play",
            Some(10),
            Some(600),
            Duration::hours(1),
        )],
    )
    .await;
    let (_, body) = app.get(&alice.cookie, "/api/2/me/in-progress").await;
    assert_eq!(body, json!([]));
    let (status, _) = app.get("", "/api/2/me/in-progress").await;
    assert_eq!(status, 401);
    let (status, _) = app
        .get("", &format!("/api/2/me/positions?podcast={FEED}"))
        .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn in_progress_limit_is_clamped() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let actions = (0..60)
        .map(|n| {
            action(
                FEED,
                &format!("http://example.com/many{n}.mp3"),
                "phone",
                "play",
                Some(10),
                Some(600),
                Duration::minutes(n),
            )
        })
        .collect();
    upload(&app, &alice, actions).await;
    for (query, expected) in [
        ("", 20),
        ("?limit=0", 1),
        ("?limit=-5", 1),
        ("?limit=500", 50),
    ] {
        let (status, body) = app
            .get(&alice.cookie, &format!("/api/2/me/in-progress{query}"))
            .await;
        assert_eq!(status, 200);
        assert_eq!(body.as_array().unwrap().len(), expected, "query {query:?}");
    }
}

#[tokio::test]
async fn positions_returns_latest_play_per_episode_of_podcast() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let h = Duration::hours;
    upload(
        &app,
        &alice,
        vec![
            action(
                FEED,
                "http://example.com/p1.mp3",
                "phone",
                "play",
                Some(10),
                Some(600),
                h(3),
            ),
            action(
                FEED,
                "http://example.com/p1.mp3",
                "tablet",
                "play",
                Some(40),
                Some(600),
                h(2),
            ),
            action(
                FEED,
                "http://example.com/p1.mp3",
                "phone",
                "download",
                None,
                None,
                h(1),
            ),
            action(
                FEED,
                "http://example.com/done.mp3",
                "phone",
                "play",
                Some(600),
                Some(600),
                h(1),
            ),
            action(
                OTHER_FEED,
                "http://example.com/x.mp3",
                "phone",
                "play",
                Some(5),
                Some(600),
                h(1),
            ),
        ],
    )
    .await;

    // Trailing slash: the podcast URL is normalized before lookup.
    let (status, body) = app
        .get(
            &alice.cookie,
            "/api/2/me/positions?podcast=http://example.com/feed.xml/",
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let mut urls = episode_urls(&body);
    urls.sort();
    assert_eq!(
        urls,
        vec!["http://example.com/done.mp3", "http://example.com/p1.mp3"]
    );
    let p1 = body
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["episode_url"] == "http://example.com/p1.mp3")
        .unwrap();
    assert_eq!(p1["position"], 40);
    assert_eq!(p1["device"], "tablet");

    let (status, body) = app
        .get(
            &alice.cookie,
            "/api/2/me/positions?podcast=http://unknown.example/feed",
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(body, json!([]));

    let (status, _) = app.get(&alice.cookie, "/api/2/me/positions").await;
    assert_eq!(status, 400);
}
