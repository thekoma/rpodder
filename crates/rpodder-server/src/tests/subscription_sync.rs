//! Subscription delta endpoints: the returned cursor lags the clock, so a
//! device may see its own recent changes again; the delta must stay coherent.

use serde_json::json;

use super::TestApp;

const X: &str = "http://example.com/x.xml";

#[tokio::test]
async fn resubscribe_within_cursor_window_is_only_an_add() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let url = "/api/2/subscriptions/alice/phone.json";

    let mut since = 0;
    for (add, remove) in [(vec![X], vec![]), (vec![], vec![X]), (vec![X], vec![])] {
        let (status, body) = app
            .post_json(&alice.cookie, url, json!({"add": add, "remove": remove}))
            .await;
        assert_eq!(status, 200, "{body}");
        since = body["timestamp"].as_i64().unwrap();
    }

    let (_, body) = app
        .get(&alice.cookie, &format!("{url}?since={since}"))
        .await;
    assert_eq!(body["add"], json!([X]), "{body}");
    assert_eq!(body["remove"], json!([]), "{body}");

    let (_, body) = app
        .get(&alice.cookie, "/api/2/updates/alice/phone.json?since=0")
        .await;
    assert_eq!(body["add"], json!([X]), "{body}");
    assert_eq!(body["remove"], json!([]), "{body}");
}

#[tokio::test]
async fn repeated_remove_still_reports_the_url() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let url = "/api/2/subscriptions/alice/phone.json";

    let mut since = 0;
    for (add, remove) in [(vec![X], vec![]), (vec![], vec![X]), (vec![], vec![X])] {
        let (status, body) = app
            .post_json(&alice.cookie, url, json!({"add": add, "remove": remove}))
            .await;
        assert_eq!(status, 200, "{body}");
        since = body["timestamp"].as_i64().unwrap();
    }

    let (_, body) = app
        .get(&alice.cookie, &format!("{url}?since={since}"))
        .await;
    assert_eq!(body["add"], json!([]), "{body}");
    assert_eq!(body["remove"], json!([X]), "{body}");
}
