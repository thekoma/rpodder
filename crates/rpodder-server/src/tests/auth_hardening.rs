//! Session hardening: one-time tokens are not login sessions, credential and
//! account changes revoke sessions, and the SSO callback is bound to the
//! browser that started the login (`state`).

use axum::{
    Json, Router,
    body::Body,
    http::{Request, StatusCode, header},
    routing::{get, post},
};
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use rpodder_core::repo::{SessionRepo, UserRepo};
use rpodder_core::types::Session;

use super::{TestApp, TestUser};

async fn add_token(app: &TestApp, user: &TestUser, token: &str) {
    SessionRepo::create(
        &app.repo(),
        &Session {
            id: Uuid::now_v7(),
            user_id: user.user.id,
            token: token.to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();
}

async fn admin(app: &TestApp) -> TestUser {
    let admin = app.create_user("admin").await;
    UserRepo::set_admin(&app.repo(), admin.user.id, true)
        .await
        .unwrap();
    admin
}

fn devices_uri(user: &TestUser) -> String {
    format!("/api/2/devices/{}.json", user.user.username)
}

#[tokio::test]
async fn reset_and_activation_tokens_are_not_sessions() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    add_token(&app, &alice, "reset-abc").await;
    add_token(&app, &alice, "activate-def").await;

    let (status, _) = app.get(&alice.cookie, &devices_uri(&alice)).await;
    assert_eq!(status, StatusCode::OK, "a login session still works");
    for token in ["reset-abc", "activate-def"] {
        let (status, _) = app
            .get(&format!("sessionid={token}"), &devices_uri(&alice))
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{token} used as a cookie");
    }
}

#[tokio::test]
async fn password_reset_confirm_revokes_sessions() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    add_token(&app, &alice, "reset-abc").await;

    let (status, _) = app
        .post_json(
            "",
            "/api/2/password-reset/confirm",
            json!({"token": "abc", "new_password": "newpass"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = app.get(&alice.cookie, &devices_uri(&alice)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "old session survived");
}

#[tokio::test]
async fn admin_set_password_revokes_sessions() {
    let app = TestApp::new().await;
    let admin = admin(&app).await;
    let alice = app.create_user("alice").await;

    let (status, _) = app
        .post_json(
            &admin.cookie,
            "/api/admin/users/alice/password",
            json!({"password": "newpass"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = app.get(&alice.cookie, &devices_uri(&alice)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "old session survived");
    let (status, _) = app.get(&admin.cookie, &devices_uri(&admin)).await;
    assert_eq!(status, StatusCode::OK, "the admin's own session is kept");
}

#[tokio::test]
async fn admin_setting_own_password_stays_logged_in() {
    let app = TestApp::new().await;
    let admin = admin(&app).await;
    add_token(&app, &admin, "admin-phone").await;

    let (status, _) = app
        .post_json(
            &admin.cookie,
            "/api/admin/users/admin/password",
            json!({"password": "newpass"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = app.get(&admin.cookie, &devices_uri(&admin)).await;
    assert_eq!(status, StatusCode::OK, "the admin's own session is kept");
    let (status, _) = app.get("sessionid=admin-phone", &devices_uri(&admin)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "other session survived");
}

#[tokio::test]
async fn deactivate_revokes_sessions() {
    let app = TestApp::new().await;
    let admin = admin(&app).await;
    let alice = app.create_user("alice").await;

    for action in ["deactivate", "activate"] {
        let (status, _) = app
            .post_json(
                &admin.cookie,
                &format!("/api/admin/users/alice/{action}"),
                json!({}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{action}");
    }

    let (status, _) = app.get(&alice.cookie, &devices_uri(&alice)).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "session came back after re-activation"
    );
}

#[tokio::test]
async fn own_password_change_revokes_other_sessions() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    add_token(&app, &alice, "alice-phone").await;

    let (status, _) = app
        .post_json(
            &alice.cookie,
            "/api/2/me/password",
            json!({"old_password": "pass", "new_password": "newpass"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = app.get(&alice.cookie, &devices_uri(&alice)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the session making the change stays"
    );
    let (status, _) = app.get("sessionid=alice-phone", &devices_uri(&alice)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "other session survived");
}

#[tokio::test]
async fn password_change_without_current_password_needs_sso_session() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    add_token(&app, &alice, "alice-phone").await;

    let (status, body) = app
        .post_json(
            &alice.cookie,
            "/api/2/me/password",
            json!({"new_password": "takeover"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _) = app.get("sessionid=alice-phone", &devices_uri(&alice)).await;
    assert_eq!(status, StatusCode::OK, "a refused change revokes nothing");

    add_token(&app, &alice, "sso-alice").await;
    let (status, _) = app
        .post_json(
            "sessionid=sso-alice",
            "/api/2/me/password",
            json!({"new_password": "newpass"}),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "SSO accounts have no known password"
    );
}

#[tokio::test]
async fn password_reset_keeps_pending_activation_link() {
    let app = TestApp::new().await;
    let alice = app.create_user("alice").await;
    UserRepo::set_active(&app.repo(), alice.user.id, false)
        .await
        .unwrap();
    add_token(&app, &alice, "activate-xyz").await;
    add_token(&app, &alice, "reset-abc").await;

    let (status, _) = app
        .post_json(
            "",
            "/api/2/password-reset/confirm",
            json!({"token": "abc", "new_password": "newpass"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = app.get("", "/api/2/activate?token=xyz").await;
    assert!(status.is_redirection(), "activation link lost: {status}");
    let user = UserRepo::find_by_id(&app.repo(), alice.user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(user.is_active);
}

// --- SSO state ---

const SSO_USER: &str = "sso-alice";

/// Minimal OIDC provider: discovery, a token endpoint that accepts any code,
/// and a userinfo endpoint naming `SSO_USER`.
async fn fake_idp() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let discovery = json!({
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "userinfo_endpoint": format!("{base}/userinfo"),
    });
    let idp = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(move || async move { Json(discovery) }),
        )
        .route(
            "/token",
            post(|| async { Json(json!({"access_token": "idp-token"})) }),
        )
        .route(
            "/userinfo",
            get(|| async { Json(json!({"preferred_username": SSO_USER})) }),
        );
    tokio::spawn(async move { axum::serve(listener, idp).await.unwrap() });
    base
}

async fn sso_app() -> TestApp {
    let issuer = fake_idp().await;
    TestApp::with_config(json!({
        "oauth_issuer_url": issuer,
        "oauth_client_id": "rpodder",
        "oauth_client_secret": "secret",
        "base_url": "http://rpodder.test",
    }))
    .await
}

async fn get_full(app: &TestApp, uri: &str, cookie: Option<&str>) -> (StatusCode, Vec<String>) {
    get_with_host(app, uri, cookie, None).await
}

/// Set-Cookie values plus a `Location: …` entry for redirects.
async fn get_with_host(
    app: &TestApp,
    uri: &str,
    cookie: Option<&str>,
    host: Option<&str>,
) -> (StatusCode, Vec<String>) {
    let mut req = Request::builder().uri(uri);
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    if let Some(host) = host {
        req = req.header(header::HOST, host);
    }
    let (status, headers, _) = app.send_full(req.body(Body::empty()).unwrap()).await;
    let cookies = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    let location = headers
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_string());
    let mut out: Vec<String> = cookies;
    out.extend(location.map(|l| format!("Location: {l}")));
    (status, out)
}

async fn sso_user_exists(app: &TestApp) -> bool {
    UserRepo::find_by_username(&app.repo(), SSO_USER)
        .await
        .unwrap()
        .is_some()
}

#[tokio::test]
async fn sso_callback_without_state_cookie_is_rejected() {
    let app = sso_app().await;

    let (status, headers) = get_full(&app, "/auth/sso/callback?code=c&state=s", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{headers:?}");
    assert!(
        !headers.iter().any(|h| h.starts_with("sessionid=")),
        "{headers:?}"
    );
    assert!(!sso_user_exists(&app).await, "callback logged a user in");

    let (status, _) = get_full(&app, "/auth/sso/callback?code=c", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no state at all");
}

#[tokio::test]
async fn sso_callback_with_mismatched_state_is_rejected() {
    let app = sso_app().await;

    let (status, headers) = get_full(
        &app,
        "/auth/sso/callback?code=c&state=attacker",
        Some("rpodder_sso_state=victim"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{headers:?}");
    assert!(!sso_user_exists(&app).await, "callback logged a user in");
}

#[tokio::test]
async fn sso_login_round_trip_binds_state_to_browser() {
    let app = sso_app().await;

    let (status, headers) = get_full(&app, "/auth/sso/login", None).await;
    assert!(status.is_redirection(), "{status} {headers:?}");
    let location = headers
        .iter()
        .find_map(|h| h.strip_prefix("Location: "))
        .expect("redirect to the IdP");
    let state = location
        .split(['?', '&'])
        .find_map(|p| p.strip_prefix("state="))
        .expect("state in the authorize URL")
        .to_string();
    let state_cookie = headers
        .iter()
        .find_map(|h| h.strip_prefix("rpodder_sso_state="))
        .expect("state cookie set");
    assert!(
        state_cookie.starts_with(&format!("{state};")),
        "{state_cookie}"
    );
    assert!(state_cookie.contains("HttpOnly"), "{state_cookie}");

    let (status, headers) = get_full(
        &app,
        &format!("/auth/sso/callback?code=c&state={state}"),
        Some(&format!("rpodder_sso_state={state}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{headers:?}");
    assert!(
        headers.iter().any(|h| h.starts_with("sessionid=sso-")),
        "{headers:?}"
    );
    assert!(
        headers
            .iter()
            .any(|h| h.starts_with("rpodder_sso_state=;") && h.contains("Max-Age=0")),
        "state cookie cleared: {headers:?}"
    );
    assert!(sso_user_exists(&app).await);
}

#[tokio::test]
async fn sso_login_moves_to_the_base_url_host_first() {
    let app = sso_app().await;

    let (status, headers) =
        get_with_host(&app, "/auth/sso/login", None, Some("192.168.1.10:3005")).await;
    assert!(status.is_redirection(), "{status} {headers:?}");
    assert!(
        headers.contains(&"Location: http://rpodder.test/auth/sso/login?canonical=1".to_string()),
        "{headers:?}"
    );
    assert!(
        !headers.iter().any(|h| h.starts_with("rpodder_sso_state=")),
        "no state cookie on the wrong host: {headers:?}"
    );

    for (uri, host) in [
        ("/auth/sso/login", "rpodder.test"),
        ("/auth/sso/login", "RPODDER.TEST"),
        // A proxy that rewrites Host must not loop.
        ("/auth/sso/login?canonical=1", "127.0.0.1:3005"),
    ] {
        let (status, headers) = get_with_host(&app, uri, None, Some(host)).await;
        assert!(status.is_redirection(), "{host}: {status} {headers:?}");
        assert!(
            headers.iter().any(|h| h.starts_with("rpodder_sso_state=")),
            "{host}: {headers:?}"
        );
        assert!(
            headers.iter().any(|h| h.contains("/authorize?")),
            "{host}: goes to the IdP: {headers:?}"
        );
    }
}

#[tokio::test]
async fn sso_callback_provider_error_clears_state() {
    let app = sso_app().await;

    let (status, headers) = get_full(
        &app,
        "/auth/sso/callback?error=access_denied&state=s1",
        Some("rpodder_sso_state=s1"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{headers:?}");
    assert!(
        headers
            .iter()
            .any(|h| h.starts_with("rpodder_sso_state=;") && h.contains("Max-Age=0")),
        "state cookie cleared: {headers:?}"
    );
    assert!(!sso_user_exists(&app).await);
}
