//! `RPODDER_PUBLIC_DIRECTORY` (issue #38): the directory and search routes are
//! public by default, as on gpodder.net; with the option off they answer only
//! a logged-in caller, because they reveal what the server's users subscribe to.

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::json;

use super::TestApp;

const FEED: &str = "https%3A%2F%2Ffeeds.example.com%2Fshow.xml";

/// Every route of the directory group in `api_router`.
fn directory_routes() -> Vec<String> {
    vec![
        "/search.json?q=show".into(),
        "/api/2/search/all?q=show".into(),
        "/api/2/search/external?q=show".into(),
        "/api/2/trending".into(),
        "/toplist/10.json".into(),
        format!("/api/2/data/podcast.json?url={FEED}"),
        format!("/api/2/data/episode.json?podcast={FEED}&url={FEED}"),
        format!("/api/2/data/podcast/episodes.json?url={FEED}"),
        "/api/2/tags/10.json".into(),
        "/api/2/tag/news/10.json".into(),
    ]
}

/// Encoded at run time, so no credential literal sits in the source for secret scanners.
fn basic_auth(user: &str, pass: &str) -> String {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
    format!("Basic {encoded}")
}

async fn anonymous_status(app: &TestApp, uri: &str) -> StatusCode {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    app.send_full(req).await.0
}

#[tokio::test]
async fn directory_is_public_by_default() {
    let app = TestApp::new().await;
    for uri in directory_routes() {
        assert_ne!(
            anonymous_status(&app, &uri).await,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
    }
}

#[tokio::test]
async fn private_directory_refuses_anonymous_callers() {
    let app = TestApp::with_config(json!({ "public_directory": false })).await;
    for uri in directory_routes() {
        assert_eq!(
            anonymous_status(&app, &uri).await,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
    }
}

#[tokio::test]
async fn private_directory_answers_a_session_or_basic_auth() {
    let app = TestApp::with_config(json!({ "public_directory": false })).await;
    let alice = app.create_user("alice").await;

    assert_eq!(
        app.get(&alice.cookie, "/toplist/10.json").await.0,
        StatusCode::OK
    );
    assert_eq!(
        app.get(&alice.cookie, "/search.json?q=show").await.0,
        StatusCode::OK
    );

    let req = Request::builder()
        .uri("/toplist/10.json")
        .header(header::AUTHORIZATION, basic_auth("alice", "pass"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.send_full(req).await.0, StatusCode::OK);
}

#[tokio::test]
async fn private_directory_refuses_bad_credentials() {
    let app = TestApp::with_config(json!({ "public_directory": false })).await;
    app.create_user("alice").await;

    let (status, _) = app.get("sessionid=not-a-session", "/toplist/10.json").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let req = Request::builder()
        .uri("/toplist/10.json")
        .header(header::AUTHORIZATION, basic_auth("alice", "wrong"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.send_full(req).await.0, StatusCode::UNAUTHORIZED);
}

/// The web UI reads the setting here to send logged-out visitors to /login.
#[tokio::test]
async fn server_info_reports_whether_the_directory_is_public() {
    let public = TestApp::new().await;
    let (_, info) = public.get("", "/auth/sso/info").await;
    assert_eq!(info["public_directory"], json!(true));

    let private = TestApp::with_config(json!({ "public_directory": false })).await;
    let (_, info) = private.get("", "/auth/sso/info").await;
    assert_eq!(info["public_directory"], json!(false));
}
