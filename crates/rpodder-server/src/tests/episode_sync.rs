//! Episode-action sync between devices: `since` cursor, device attribution,
//! filters, aggregation, and no silent truncation.

use chrono::{Duration, TimeZone, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use rpodder_core::repo::{EpisodeActionRepo, EpisodeRepo, PodcastRepo, UserRepo};
use rpodder_core::types::{EpisodeAction, EpisodeActionType};

use super::TestApp;

const FEED: &str = "http://example.com/feed.xml";

fn play(episode: &str, device: &str, timestamp: chrono::DateTime<Utc>, position: i32) -> Value {
    json!({
        "podcast": FEED,
        "episode": episode,
        "device": device,
        "action": "play",
        "timestamp": timestamp.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "started": 0,
        "position": position,
        "total": 3600,
    })
}

fn actions(body: &Value) -> &Vec<Value> {
    body["actions"].as_array().expect("actions array")
}

#[tokio::test]
async fn late_upload_is_seen_by_device_that_already_synced() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;

    // Device B syncs first and stores the returned cursor.
    let (status, body) = app.get(&alice.cookie, "/api/2/episodes/alice.json").await;
    assert_eq!(status, 200);
    let since = body["timestamp"].as_i64().unwrap();

    // Device A uploads an action it recorded offline an hour ago.
    let listened_at = Utc::now() - Duration::hours(1);
    let (status, _) = app
        .post_json(
            &alice.cookie,
            "/api/2/episodes/alice.json",
            json!([play("http://example.com/ep1.mp3", "a", listened_at, 120)]),
        )
        .await;
    assert_eq!(status, 200);

    // Device B's next incremental sync must include it.
    let (_, body) = app
        .get(
            &alice.cookie,
            &format!("/api/2/episodes/alice.json?since={since}"),
        )
        .await;
    assert_eq!(
        actions(&body).len(),
        1,
        "late-uploaded action missing: {body}"
    );
}

#[tokio::test]
async fn reuploaded_action_with_new_position_is_seen_again() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    // Whole seconds: the upload's string timestamp has no sub-second part, so
    // the re-upload below must hit the same row.
    let ts = Utc
        .timestamp_opt((Utc::now() - Duration::hours(2)).timestamp(), 0)
        .unwrap();
    let ep = "http://example.com/ep1.mp3";

    // First upload, received an hour ago (well before the cursor's slack).
    let repo = app.repo();
    let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, FEED)
        .await
        .unwrap();
    let (episode, _) = EpisodeRepo::get_or_create_for_url(&repo, podcast.id, ep)
        .await
        .unwrap();
    EpisodeActionRepo::create(
        &repo,
        &EpisodeAction {
            id: Uuid::now_v7(),
            user_id: alice.user.id,
            device_id: None,
            episode_id: episode.id,
            action: EpisodeActionType::Play,
            podcast_ref_url: Some(FEED.to_string()),
            episode_ref_url: Some(ep.to_string()),
            started: Some(0),
            position: Some(10),
            total: Some(3600),
            timestamp: ts,
            created_at: Utc::now() - Duration::hours(1),
        },
    )
    .await
    .unwrap();
    let (_, body) = app.get(&alice.cookie, "/api/2/episodes/alice.json").await;
    let since = body["timestamp"].as_i64().unwrap();

    // Unchanged re-upload (same episode, no device, action, timestamp) must
    // not resurface; a changed position must.
    let mut unchanged = play(ep, "a", ts, 10);
    unchanged.as_object_mut().unwrap().remove("device");
    app.post_json(
        &alice.cookie,
        "/api/2/episodes/alice.json",
        json!([unchanged]),
    )
    .await;
    let (_, body) = app
        .get(
            &alice.cookie,
            &format!("/api/2/episodes/alice.json?since={since}"),
        )
        .await;
    assert!(
        actions(&body).is_empty(),
        "unchanged re-upload resurfaced: {body}"
    );

    let mut changed = play(ep, "a", ts, 99);
    changed.as_object_mut().unwrap().remove("device");
    app.post_json(
        &alice.cookie,
        "/api/2/episodes/alice.json",
        json!([changed]),
    )
    .await;

    let (_, body) = app
        .get(
            &alice.cookie,
            &format!("/api/2/episodes/alice.json?since={since}"),
        )
        .await;
    assert!(
        actions(&body).iter().any(|a| a["position"] == 99),
        "updated position not returned: {body}"
    );
}

#[tokio::test]
async fn unknown_device_is_created_and_reported_on_actions() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;

    app.post_json(
        &alice.cookie,
        "/api/2/episodes/alice.json",
        json!([play("http://example.com/ep1.mp3", "newdev", Utc::now(), 5)]),
    )
    .await;

    let (_, devices) = app.get(&alice.cookie, "/api/2/devices/alice.json").await;
    assert!(
        devices
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["id"] == "newdev"),
        "device not auto-created: {devices}"
    );

    let (_, body) = app.get(&alice.cookie, "/api/2/episodes/alice.json").await;
    assert_eq!(actions(&body)[0]["device"], "newdev", "{body}");
}

#[tokio::test]
async fn unknown_device_or_podcast_filter_returns_nothing() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    app.post_json(
        &alice.cookie,
        "/api/2/episodes/alice.json",
        json!([play("http://example.com/ep1.mp3", "a", Utc::now(), 5)]),
    )
    .await;

    let (_, body) = app
        .get(&alice.cookie, "/api/2/episodes/alice.json?device=nope")
        .await;
    assert!(actions(&body).is_empty(), "device filter ignored: {body}");

    let (_, body) = app
        .get(
            &alice.cookie,
            "/api/2/episodes/alice.json?podcast=http://unknown.example/feed",
        )
        .await;
    assert!(actions(&body).is_empty(), "podcast filter ignored: {body}");
}

#[tokio::test]
async fn no_actions_are_dropped_beyond_1000() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let repo = app.repo();
    let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, FEED)
        .await
        .unwrap();
    let (episode, _) =
        EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://example.com/ep1.mp3")
            .await
            .unwrap();
    let base = Utc::now() - Duration::days(30);
    for i in 0..1001 {
        EpisodeActionRepo::create(
            &repo,
            &EpisodeAction {
                id: Uuid::now_v7(),
                user_id: alice.user.id,
                device_id: None,
                episode_id: episode.id,
                action: EpisodeActionType::Play,
                podcast_ref_url: Some(FEED.to_string()),
                episode_ref_url: Some("http://example.com/ep1.mp3".to_string()),
                started: Some(0),
                position: Some(i),
                total: Some(3600),
                timestamp: base + Duration::seconds(i64::from(i)),
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();
    }

    let (_, body) = app.get(&alice.cookie, "/api/2/episodes/alice.json").await;
    assert_eq!(actions(&body).len(), 1001);
}

#[tokio::test]
async fn aggregated_returns_latest_action_per_episode() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let t = Utc::now() - Duration::hours(3);
    let ep1 = "http://example.com/ep1.mp3";
    let ep2 = "http://example.com/ep2.mp3";
    app.post_json(
        &alice.cookie,
        "/api/2/episodes/alice.json",
        json!([
            play(ep1, "a", t, 10),
            play(ep1, "b", t + Duration::minutes(20), 30),
            play(ep1, "a", t + Duration::minutes(10), 20),
            play(ep2, "a", t, 7),
        ]),
    )
    .await;

    let (_, body) = app
        .get(&alice.cookie, "/api/2/episodes/alice.json?aggregated=true")
        .await;
    let list = actions(&body);
    assert_eq!(list.len(), 2, "{body}");
    let latest_ep1 = list.iter().find(|a| a["episode"] == ep1).unwrap();
    assert_eq!(latest_ep1["position"], 30);
    assert_eq!(latest_ep1["device"], "b");
}

#[tokio::test]
async fn deactivated_user_session_is_rejected() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let (status, _) = app.get(&alice.cookie, "/api/2/devices/alice.json").await;
    assert_eq!(status, 200);

    UserRepo::set_active(&app.repo(), alice.user.id, false)
        .await
        .unwrap();

    let (status, _) = app.get(&alice.cookie, "/api/2/devices/alice.json").await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn invalid_device_id_is_ignored_not_created() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let long_id = "d".repeat(65);
    for device in [long_id.as_str(), "has space", "évice"] {
        let (status, _) = app
            .post_json(
                &alice.cookie,
                "/api/2/episodes/alice.json",
                json!([play("http://example.com/ep1.mp3", device, Utc::now(), 5)]),
            )
            .await;
        assert_eq!(status, 200, "upload with device {device:?} failed");
    }
    let (_, devices) = app.get(&alice.cookie, "/api/2/devices/alice.json").await;
    assert!(devices.as_array().unwrap().is_empty(), "{devices}");
}

#[tokio::test]
async fn registered_device_with_nonstandard_id_keeps_its_actions() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let (status, _) = app
        .post_json(
            &alice.cookie,
            "/api/2/devices/alice/my%20phone.json",
            json!({"caption": "My phone", "type": "mobile"}),
        )
        .await;
    assert_eq!(status, 200);

    app.post_json(
        &alice.cookie,
        "/api/2/episodes/alice.json",
        json!([play(
            "http://example.com/ep1.mp3",
            "my phone",
            Utc::now(),
            5
        )]),
    )
    .await;

    let (_, body) = app.get(&alice.cookie, "/api/2/episodes/alice.json").await;
    assert_eq!(actions(&body)[0]["device"], "my phone", "{body}");
}

#[tokio::test]
async fn returned_cursor_lags_the_clock() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let now = Utc::now().timestamp();
    let (_, body) = app.get(&alice.cookie, "/api/2/episodes/alice.json").await;
    let ts = body["timestamp"].as_i64().unwrap();
    assert!(
        ts <= now - crate::routes::helpers::SYNC_CURSOR_SLACK_SECS + 1,
        "{ts} vs {now}"
    );
    let (_, body) = app
        .post_json(
            &alice.cookie,
            "/api/2/episodes/alice.json",
            json!([play("http://example.com/ep1.mp3", "a", Utc::now(), 5)]),
        )
        .await;
    let ts = body["timestamp"].as_i64().unwrap();
    assert!(
        ts <= now - crate::routes::helpers::SYNC_CURSOR_SLACK_SECS + 1,
        "{ts} vs {now}"
    );
}
