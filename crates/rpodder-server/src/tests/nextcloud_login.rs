//! Nextcloud Login Flow v2, app passwords and the gpoddersync API, as
//! AntennaPod's and Podcini's "Nextcloud gPodder Sync" provider uses them.

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::{Duration, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use rpodder_core::repo::{AppPasswordRepo, SessionRepo, UserRepo};
use rpodder_core::types::{LoginFlow, Session};

use super::{TestApp, TestUser};
use crate::middleware::auth::hash_token;

const UA: &str = "AntennaPod/3.4.0";

/// Encoded at run time, so no credential literal sits in the source for secret scanners.
fn basic(user: &str, pass: &str) -> String {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
    format!("Basic {encoded}")
}

async fn start(app: &TestApp) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/index.php/login/v2")
        .header(header::HOST, "pod.example.com")
        .header("x-forwarded-proto", "https")
        .header(header::USER_AGENT, UA)
        .body(Body::empty())
        .unwrap();
    let (status, _, json) = app.send_full(req).await;
    (status, json)
}

async fn poll(app: &TestApp, token: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/index.php/login/v2/poll")
        .header(header::HOST, "pod.example.com")
        .header("x-forwarded-proto", "https")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("token={token}")))
        .unwrap();
    let (status, _, json) = app.send_full(req).await;
    (status, json)
}

/// The poll token and the login token (from the `login` URL's fragment).
async fn start_tokens(app: &TestApp) -> (String, String) {
    let (status, body) = start(app).await;
    assert_eq!(status, StatusCode::OK);
    let poll_token = body["poll"]["token"].as_str().unwrap().to_string();
    let login = body["login"].as_str().unwrap();
    let (_, login_token) = login.split_once('#').unwrap();
    (poll_token, login_token.to_string())
}

async fn answer(app: &TestApp, auth: &Auth, token: &str, approve: bool) -> StatusCode {
    auth.post(
        app,
        "/api/2/me/login-flows/answer",
        json!({ "token": token, "approve": approve }),
    )
    .await
    .0
}

/// The app password for `user`, through a full login flow.
async fn app_password(app: &TestApp, user: &TestUser) -> String {
    let (poll_token, login_token) = start_tokens(app).await;
    assert_eq!(
        answer(app, &Auth::Cookie(user.cookie.clone()), &login_token, true).await,
        StatusCode::NO_CONTENT
    );
    let (status, body) = poll(app, &poll_token).await;
    assert_eq!(status, StatusCode::OK);
    body["appPassword"].as_str().unwrap().to_string()
}

enum Auth {
    Cookie(String),
    Basic(String),
}

impl Auth {
    fn apply(&self, req: axum::http::request::Builder) -> axum::http::request::Builder {
        match self {
            Auth::Cookie(c) => req.header(header::COOKIE, c),
            Auth::Basic(b) => req.header(header::AUTHORIZATION, b),
        }
    }

    async fn get(&self, app: &TestApp, uri: &str) -> (StatusCode, Value) {
        let req = self
            .apply(Request::builder().uri(uri))
            .body(Body::empty())
            .unwrap();
        let (status, _, json) = app.send_full(req).await;
        (status, json)
    }

    async fn post(&self, app: &TestApp, uri: &str, body: Value) -> (StatusCode, Value) {
        self.post_raw(app, uri, &body.to_string()).await
    }

    async fn post_raw(&self, app: &TestApp, uri: &str, body: &str) -> (StatusCode, Value) {
        let req = self
            .apply(Request::builder().method("POST").uri(uri))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let (status, _, json) = app.send_full(req).await;
        (status, json)
    }

    async fn delete(&self, app: &TestApp, uri: &str) -> StatusCode {
        let req = self
            .apply(Request::builder().method("DELETE").uri(uri))
            .body(Body::empty())
            .unwrap();
        app.send_full(req).await.0
    }
}

async fn insert_session(app: &TestApp, user: &TestUser, token: &str) {
    SessionRepo::create(
        &app.repo(),
        &Session {
            id: Uuid::now_v7(),
            user_id: user.user.id,
            token: token.into(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();
}

/// `sessionid=…` from a `login.json` response.
async fn login_cookie(app: &TestApp, username: &str, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/2/auth/{username}/login.json"))
        .header(header::AUTHORIZATION, basic(username, password))
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = app.send_full(req).await;
    assert_eq!(status, StatusCode::OK);
    let set_cookie = headers.get(header::SET_COOKIE).unwrap().to_str().unwrap();
    set_cookie.split(';').next().unwrap().to_string()
}

async fn app_password_id(app: &TestApp, user: &TestUser) -> Uuid {
    AppPasswordRepo::list_for_user(&app.repo(), user.user.id)
        .await
        .unwrap()[0]
        .id
}

// ---------------------------------------------------------------------------
// Login flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_login_flow_issues_one_app_password() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let me = Auth::Cookie(alice.cookie.clone());

    let (status, body) = start(&app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["poll"]["endpoint"],
        "https://pod.example.com/index.php/login/v2/poll"
    );
    let login = body["login"].as_str().unwrap();
    assert!(login.starts_with("https://pod.example.com/login/flow#"));
    assert!(!login.contains('?'), "the token stays out of the query");
    let poll_token = body["poll"]["token"].as_str().unwrap();
    let login_token = login.split_once('#').unwrap().1;

    assert_eq!(poll(&app, poll_token).await.0, StatusCode::NOT_FOUND);

    let (status, info) = me
        .post(
            &app,
            "/api/2/me/login-flows/lookup",
            json!({ "token": login_token }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(info["client_name"], UA);
    assert_eq!(info["approved"], false);
    assert_eq!(info["username"], "alice");

    assert_eq!(
        answer(&app, &me, login_token, true).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        answer(&app, &me, login_token, true).await,
        StatusCode::NO_CONTENT,
        "approving twice is fine"
    );

    let (status, creds) = poll(&app, poll_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(creds["server"], "https://pod.example.com");
    assert_eq!(creds["loginName"], "alice");
    let password = creds["appPassword"].as_str().unwrap();
    assert_eq!(password.len(), 64);

    assert_eq!(poll(&app, poll_token).await.0, StatusCode::NOT_FOUND);

    let (status, info) = me
        .post(
            &app,
            "/api/2/me/login-flows/lookup",
            json!({ "token": login_token }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        info["approved"], true,
        "a reload after redeeming still shows the grant"
    );

    let listed = me.get(&app, "/api/2/me/app-passwords").await.1;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["name"], UA);
}

#[tokio::test]
async fn base_url_setting_wins_over_request_headers() {
    let app = TestApp::with_config(json!({ "base_url": "https://pods.example.org/" })).await;
    let (_, body) = start(&app).await;
    assert_eq!(
        body["poll"]["endpoint"],
        "https://pods.example.org/index.php/login/v2/poll"
    );

    let app = TestApp::new().await;
    let req = Request::builder()
        .method("POST")
        .uri("/index.php/login/v2")
        .header(header::HOST, "localhost:3005")
        .body(Body::empty())
        .unwrap();
    let (_, _, body) = app.send_full(req).await;
    assert_eq!(
        body["poll"]["endpoint"],
        "http://localhost:3005/index.php/login/v2/poll"
    );
}

#[tokio::test]
async fn poll_without_token_is_not_found() {
    let app = TestApp::new().await;
    assert_eq!(poll(&app, "").await.0, StatusCode::NOT_FOUND);
    let req = Request::builder()
        .method("POST")
        .uri("/index.php/login/v2/poll")
        .header(header::HOST, "pod.example.com")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.send_full(req).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn denied_and_foreign_flows() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let bob = app.create_user("bob").await;
    let a = Auth::Cookie(alice.cookie.clone());
    let b = Auth::Cookie(bob.cookie.clone());

    let (poll_token, login_token) = start_tokens(&app).await;
    assert_eq!(
        answer(&app, &a, &login_token, false).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(poll(&app, &poll_token).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        answer(&app, &a, &login_token, true).await,
        StatusCode::NOT_FOUND
    );

    let (poll_token, login_token) = start_tokens(&app).await;
    assert_eq!(
        answer(&app, &a, &login_token, true).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        answer(&app, &b, &login_token, true).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        answer(&app, &b, &login_token, false).await,
        StatusCode::NOT_FOUND,
        "bob can't cancel alice's grant"
    );
    let (status, _) = b
        .post(
            &app,
            "/api/2/me/login-flows/lookup",
            json!({ "token": login_token }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, creds) = poll(&app, &poll_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(creds["loginName"], "alice");
}

#[tokio::test]
async fn expired_flow_is_gone() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let now = Utc::now();
    AppPasswordRepo::create_flow(
        &app.repo(),
        &LoginFlow {
            id: Uuid::now_v7(),
            poll_token_hash: hash_token("poll-token"),
            login_token_hash: hash_token("login-token"),
            client_name: UA.into(),
            user_id: None,
            redeemed_at: None,
            expires_at: now - Duration::minutes(1),
            created_at: now - Duration::minutes(11),
        },
    )
    .await
    .unwrap();
    let a = Auth::Cookie(alice.cookie.clone());
    let (status, _) = a
        .post(
            &app,
            "/api/2/me/login-flows/lookup",
            json!({ "token": "login-token" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer(&app, &a, "login-token", true).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(poll(&app, "poll-token").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn flow_answer_needs_json() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let (_, login_token) = start_tokens(&app).await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/2/me/login-flows/answer")
        .header(header::COOKIE, &alice.cookie)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(
            json!({ "token": login_token, "approve": true }).to_string(),
        ))
        .unwrap();
    assert_eq!(
        app.send_full(req).await.0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
}

// ---------------------------------------------------------------------------
// App passwords as credentials
// ---------------------------------------------------------------------------

#[tokio::test]
async fn app_password_authenticates_until_revoked() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let password = app_password(&app, &alice).await;
    let ap = Auth::Basic(basic("alice", &password));

    assert_eq!(
        ap.get(&app, "/index.php/apps/gpoddersync/subscriptions")
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        ap.get(&app, "/api/2/devices/alice.json").await.0,
        StatusCode::OK
    );
    assert_eq!(
        Auth::Basic(basic("alice", &"x".repeat(64)))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let bob = app.create_user("bob").await;
    assert_eq!(
        Auth::Basic(basic("bob", &password))
            .get(&app, "/api/2/devices/bob.json")
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "an app password only works for its own user"
    );
    assert_eq!(
        Auth::Basic(basic("alice", "pass"))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::OK,
        "the account password keeps working"
    );

    let id = app_password_id(&app, &alice).await;
    let me = Auth::Cookie(alice.cookie.clone());
    assert_eq!(
        Auth::Cookie(bob.cookie.clone())
            .delete(&app, &format!("/api/2/me/app-passwords/{id}"))
            .await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        me.delete(&app, &format!("/api/2/me/app-passwords/{id}"))
            .await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        ap.get(&app, "/api/2/devices/alice.json").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        me.delete(&app, "/api/2/me/app-passwords/not-a-uuid").await,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn deactivated_user_app_password_is_refused() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let password = app_password(&app, &alice).await;
    UserRepo::set_active(&app.repo(), alice.user.id, false)
        .await
        .unwrap();
    assert_eq!(
        Auth::Basic(basic("alice", &password))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn sessions_from_an_app_password_stay_tied_to_it() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let password = app_password(&app, &alice).await;
    let id = app_password_id(&app, &alice).await;

    let cookie = login_cookie(&app, "alice", &password).await;
    assert!(cookie.starts_with(&format!("sessionid=app-{id}-")));
    let session = Auth::Cookie(cookie.clone());
    assert_eq!(
        session.get(&app, "/api/2/devices/alice.json").await.0,
        StatusCode::OK
    );

    // login.json with the app session keeps it an app session.
    let req = Request::builder()
        .method("POST")
        .uri("/api/2/auth/alice/login.json")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = app.send_full(req).await;
    assert_eq!(status, StatusCode::OK);
    let again = headers.get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(again.starts_with(&format!("sessionid=app-{id}-")));

    assert_eq!(
        Auth::Cookie(alice.cookie.clone())
            .delete(&app, &format!("/api/2/me/app-passwords/{id}"))
            .await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        session.get(&app, "/api/2/devices/alice.json").await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn app_session_without_its_app_password_is_refused() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    // A login that raced the revoke inserted its session after the delete.
    let token = format!("app-{}-{}", Uuid::now_v7(), "x".repeat(64));
    insert_session(&app, &alice, &token).await;
    assert_eq!(
        Auth::Cookie(format!("sessionid={token}"))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn app_passwords_cannot_manage_credentials() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    UserRepo::set_admin(&app.repo(), alice.user.id, true)
        .await
        .unwrap();
    let password = app_password(&app, &alice).await;
    let app_cookie = login_cookie(&app, "alice", &password).await;
    let (_, login_token) = start_tokens(&app).await;

    for auth in [
        Auth::Basic(basic("alice", &password)),
        Auth::Cookie(app_cookie),
    ] {
        assert_eq!(
            auth.get(&app, "/api/2/me/app-passwords").await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            auth.post(
                &app,
                "/api/2/me/app-passwords",
                json!({ "name": "x", "current_password": "pass" })
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            auth.delete(&app, &format!("/api/2/me/app-passwords/{}", Uuid::now_v7()))
                .await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            auth.post(
                &app,
                "/api/2/me/login-flows/lookup",
                json!({ "token": login_token })
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            answer(&app, &auth, &login_token, true).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            auth.post(
                &app,
                "/api/2/me/password",
                json!({ "old_password": "pass", "new_password": "other" })
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            auth.get(&app, "/api/admin/users").await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(auth.get(&app, "/api/2/me").await.0, StatusCode::OK);
    }
    assert_eq!(
        Auth::Cookie(alice.cookie.clone())
            .get(&app, "/api/admin/users")
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn creating_by_hand_needs_the_current_password_unless_sso() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let me = Auth::Cookie(alice.cookie.clone());

    let (status, body) = me
        .post(&app, "/api/2/me/app-passwords", json!({ "name": "Kasts" }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "current password required");
    let (status, body) = me
        .post(
            &app,
            "/api/2/me/app-passwords",
            json!({ "name": "Kasts", "current_password": "wrong" }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "wrong password");
    let (status, _) = me
        .post(
            &app,
            "/api/2/me/app-passwords",
            json!({ "name": "  ", "current_password": "pass" }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, created) = me
        .post(
            &app,
            "/api/2/me/app-passwords",
            json!({ "name": " Kasts ", "current_password": "pass" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["name"], "Kasts");
    let password = created["password"].as_str().unwrap();
    assert_eq!(
        Auth::Basic(basic("alice", password))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::OK
    );

    let sso = "sso-token-for-alice";
    insert_session(&app, &alice, sso).await;
    let (status, _) = Auth::Cookie(format!("sessionid={sso}"))
        .post(
            &app,
            "/api/2/me/app-passwords",
            json!({ "name": "Podcini" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

// ---------------------------------------------------------------------------
// Password changes and recovery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn own_password_change_keeps_app_passwords() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let password = app_password(&app, &alice).await;
    let app_cookie = login_cookie(&app, "alice", &password).await;

    let (status, _) = Auth::Cookie(alice.cookie.clone())
        .post(
            &app,
            "/api/2/me/password",
            json!({ "old_password": "pass", "new_password": "new-pass" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        Auth::Cookie(app_cookie)
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::UNAUTHORIZED,
        "other sessions are logged out"
    );
    assert_eq!(
        Auth::Basic(basic("alice", &password))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn password_reset_revokes_app_passwords_and_granted_flows() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let password = app_password(&app, &alice).await;
    let (poll_token, login_token) = start_tokens(&app).await;
    assert_eq!(
        answer(
            &app,
            &Auth::Cookie(alice.cookie.clone()),
            &login_token,
            true
        )
        .await,
        StatusCode::NO_CONTENT
    );
    insert_session(&app, &alice, "reset-the-reset-token").await;

    let req = Request::builder()
        .method("POST")
        .uri("/api/2/password-reset/confirm")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "token": "the-reset-token", "new_password": "fresh-pass" }).to_string(),
        ))
        .unwrap();
    assert_eq!(app.send_full(req).await.0, StatusCode::OK);

    assert_eq!(
        Auth::Basic(basic("alice", &password))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        poll(&app, &poll_token).await.0,
        StatusCode::NOT_FOUND,
        "a flow granted before the reset can't be redeemed after it"
    );
}

#[tokio::test]
async fn admin_setting_a_password_revokes_app_passwords() {
    let app = TestApp::new().await;
    let admin = app.create_user("admin").await;
    UserRepo::set_admin(&app.repo(), admin.user.id, true)
        .await
        .unwrap();
    let alice = app.create_user("alice").await;
    let alices = app_password(&app, &alice).await;
    let admins = app_password(&app, &admin).await;
    let a = Auth::Cookie(admin.cookie.clone());

    let (status, _) = a
        .post(
            &app,
            "/api/admin/users/alice/password",
            json!({ "password": "set-by-admin" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        Auth::Basic(basic("alice", &alices))
            .get(&app, "/api/2/devices/alice.json")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    let (status, _) = a
        .post(
            &app,
            "/api/admin/users/admin/password",
            json!({ "password": "admin-own" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        Auth::Basic(basic("admin", &admins))
            .get(&app, "/api/2/devices/admin.json")
            .await
            .0,
        StatusCode::OK,
        "an admin's own password change keeps their app passwords"
    );
}

// ---------------------------------------------------------------------------
// gpoddersync
// ---------------------------------------------------------------------------

#[tokio::test]
async fn gpoddersync_round_trip() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    let password = app_password(&app, &alice).await;
    let ap = Auth::Basic(basic("alice", &password));
    let feed = "https://feeds.example.com/show.xml";

    let (status, _) = ap
        .post(
            &app,
            "/index.php/apps/gpoddersync/subscription_change/create",
            json!({ "add": [feed], "remove": [] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, subs) = ap
        .get(&app, "/index.php/apps/gpoddersync/subscriptions?since=0")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(subs["add"], json!([feed]));
    assert!(subs["timestamp"].is_i64());

    // AntennaPod's next `since` is the phone's clock, which may run ahead.
    let ahead = Utc::now().timestamp() + 120;
    let (_, subs) = ap
        .get(
            &app,
            &format!("/index.php/apps/gpoddersync/subscriptions?since={ahead}"),
        )
        .await;
    assert_eq!(subs["add"], json!([feed]));

    let actions = json!([{
        "podcast": feed,
        "episode": "https://feeds.example.com/ep1.mp3",
        "guid": "ep1",
        "action": "PLAY",
        "timestamp": "2026-11-23T10:00:00",
        "started": 0,
        "position": 120,
        "total": 3600
    }]);
    let (status, _) = ap
        .post(
            &app,
            "/index.php/apps/gpoddersync/episode_action/create",
            actions,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, got) = ap
        .get(&app, "/index.php/apps/gpoddersync/episode_action?since=0")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(got["actions"][0]["position"], 120);

    // A gpodder.net client of the same user sees the action, on the gpoddersync device.
    let (_, gpodder) = Auth::Cookie(alice.cookie.clone())
        .get(&app, "/api/2/episodes/alice.json?since=0")
        .await;
    assert_eq!(gpodder["actions"][0]["device"], "gpoddersync");

    let (_, devices) = Auth::Cookie(alice.cookie.clone())
        .get(&app, "/api/2/devices/alice.json")
        .await;
    let device = devices
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == "gpoddersync")
        .unwrap();
    assert_eq!(device["caption"], "Nextcloud sync");
}

#[tokio::test]
async fn gpoddersync_needs_credentials() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .uri("/index.php/apps/gpoddersync/subscriptions")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.send_full(req).await.0, StatusCode::UNAUTHORIZED);
}
