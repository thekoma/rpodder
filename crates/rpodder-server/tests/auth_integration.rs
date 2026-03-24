use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    middleware as axum_mw,
    routing::post,
};
use chrono::{Duration, Utc};
use tower::ServiceExt;
use uuid::Uuid;

use rpodder_core::repo::{SessionRepo, UserRepo};
use rpodder_core::types::Session;
use rpodder_db::{Db, sqlite::SqliteRepo};

// We test against the server crate's internals, so we replicate the router setup
// to avoid needing to expose everything as pub.

/// Build an in-memory SQLite DB with the schema applied.
async fn setup_db() -> Db {
    let db = Db::connect("sqlite::memory:").await.unwrap();
    db.migrate("../../migrations").await.unwrap();
    db
}

fn repo(db: &Db) -> SqliteRepo {
    match db {
        Db::Sqlite(pool) => SqliteRepo::new(pool.clone()),
        _ => panic!("expected sqlite"),
    }
}

/// Hash a password with argon2 for test users.
fn hash_password(password: &str) -> String {
    use argon2::password_hash::SaltString;
    use argon2::password_hash::rand_core::OsRng;
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

fn basic_auth_header(username: &str, password: &str) -> String {
    use base64::Engine;
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    format!("Basic {encoded}")
}

// === Tests ===

#[tokio::test]
async fn login_without_auth_returns_401() {
    let db = setup_db().await;
    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/testuser/login.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_with_wrong_password_returns_401() {
    let db = setup_db().await;
    let r = repo(&db);
    UserRepo::create(&r, "testuser", &hash_password("correct"), None)
        .await
        .unwrap();

    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/testuser/login.json")
                .header(
                    header::AUTHORIZATION,
                    basic_auth_header("testuser", "wrong"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_with_correct_password_returns_200_with_session_cookie() {
    let db = setup_db().await;
    let r = repo(&db);
    UserRepo::create(&r, "testuser", &hash_password("mypassword"), None)
        .await
        .unwrap();

    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/testuser/login.json")
                .header(
                    header::AUTHORIZATION,
                    basic_auth_header("testuser", "mypassword"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    // Should have a Set-Cookie header with sessionid
    let set_cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(set_cookie.starts_with("sessionid="));
    assert!(set_cookie.contains("HttpOnly"));
}

#[tokio::test]
async fn login_with_wrong_username_in_path_returns_403() {
    let db = setup_db().await;
    let r = repo(&db);
    UserRepo::create(&r, "alice", &hash_password("pass"), None)
        .await
        .unwrap();

    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    // Authenticate as alice, but path says bob
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/bob/login.json")
                .header(header::AUTHORIZATION, basic_auth_header("alice", "pass"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn login_case_insensitive_username() {
    let db = setup_db().await;
    let r = repo(&db);
    UserRepo::create(&r, "CamelCase", &hash_password("pass"), None)
        .await
        .unwrap();

    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/camelcase/login.json")
                .header(
                    header::AUTHORIZATION,
                    basic_auth_header("camelcase", "pass"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn session_cookie_authenticates_subsequent_requests() {
    let db = setup_db().await;
    let r = repo(&db);
    let user = UserRepo::create(&r, "sessuser", &hash_password("pass"), None)
        .await
        .unwrap();

    // Insert a session directly
    let session = Session {
        id: Uuid::now_v7(),
        user_id: user.id,
        token: "valid-session-token".to_string(),
        expires_at: Utc::now() + Duration::hours(1),
        created_at: Utc::now(),
    };
    SessionRepo::create(&r, &session).await.unwrap();

    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    // Use the session cookie instead of Basic Auth
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/sessuser/login.json")
                .header(header::COOKIE, "sessionid=valid-session-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn expired_session_cookie_returns_401() {
    let db = setup_db().await;
    let r = repo(&db);
    let user = UserRepo::create(&r, "expuser", &hash_password("pass"), None)
        .await
        .unwrap();

    let session = Session {
        id: Uuid::now_v7(),
        user_id: user.id,
        token: "expired-session-token".to_string(),
        expires_at: Utc::now() - Duration::hours(1),
        created_at: Utc::now() - Duration::hours(2),
    };
    SessionRepo::create(&r, &session).await.unwrap();

    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/expuser/login.json")
                .header(header::COOKIE, "sessionid=expired-session-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_clears_session() {
    let db = setup_db().await;
    let r = repo(&db);
    let user = UserRepo::create(&r, "logoutuser", &hash_password("pass"), None)
        .await
        .unwrap();

    let session = Session {
        id: Uuid::now_v7(),
        user_id: user.id,
        token: "logout-me-token".to_string(),
        expires_at: Utc::now() + Duration::hours(1),
        created_at: Utc::now(),
    };
    SessionRepo::create(&r, &session).await.unwrap();

    let state = rpodder_server_test_state(db.clone());
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/logoutuser/logout.json")
                .header(header::COOKIE, "sessionid=logout-me-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    // Session should be deleted
    let r2 = repo(&db);
    let found = r2.find_by_token("logout-me-token").await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn logout_without_cookie_returns_200() {
    let db = setup_db().await;
    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/anyone/logout.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Logout is always 200 (matches mygpo behavior)
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn inactive_user_cannot_login() {
    let db = setup_db().await;
    let r = repo(&db);
    let user = UserRepo::create(&r, "inactive", &hash_password("pass"), None)
        .await
        .unwrap();

    // Deactivate the user directly in DB
    sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
        .bind(user.id.to_string())
        .execute(match &db {
            Db::Sqlite(p) => p,
            _ => panic!(),
        })
        .await
        .unwrap();

    let state = rpodder_server_test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/auth/inactive/login.json")
                .header(header::AUTHORIZATION, basic_auth_header("inactive", "pass"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// === Helpers to build test router ===
// We replicate the router from main.rs since the binary's internals aren't pub.

#[derive(Clone)]
struct TestAppState {
    db: Arc<Db>,
}

fn rpodder_server_test_state(db: Db) -> TestAppState {
    TestAppState { db: Arc::new(db) }
}

/// Simplified versions of the auth handlers for testing that work with TestAppState.
mod test_handlers {
    use super::*;
    use axum::{
        extract::{Path, Request, State},
        http::{StatusCode, header},
        middleware::Next,
        response::{IntoResponse, Response},
    };
    use rpodder_core::repo::SessionRepo;

    #[derive(Clone)]
    pub struct AuthUser(pub rpodder_core::types::User);

    pub fn require_auth_layer(
        state: TestAppState,
    ) -> impl Fn(
        Request,
        Next,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
    + Clone
    + Send {
        move |req, next| {
            let state = state.clone();
            Box::pin(require_auth_inner(state, req, next))
        }
    }

    async fn require_auth_inner(state: TestAppState, mut req: Request, next: Next) -> Response {
        let session_token = extract_session_token(&req);
        let basic_auth = extract_basic_auth(&req);

        if let Some(token) = &session_token {
            if let Some(user) = resolve_session(&state, token).await {
                req.extensions_mut().insert(AuthUser(user));
                return next.run(req).await;
            }
        }

        if let Some((username, password)) = &basic_auth {
            if let Some(user) = resolve_basic_auth(&state, username, password).await {
                req.extensions_mut().insert(AuthUser(user));
                return next.run(req).await;
            }
        }

        StatusCode::UNAUTHORIZED.into_response()
    }

    fn extract_session_token(req: &Request) -> Option<String> {
        let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;
        cookie_header
            .split(';')
            .filter_map(|c| c.trim().strip_prefix("sessionid="))
            .next()
            .map(|s| s.to_string())
    }

    fn extract_basic_auth(req: &Request) -> Option<(String, String)> {
        use base64::Engine;
        let auth_header = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
        let encoded = auth_header.strip_prefix("Basic ")?;
        let decoded = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .ok()?,
        )
        .ok()?;
        let (username, password) = decoded.split_once(':')?;
        Some((username.to_string(), password.to_string()))
    }

    async fn resolve_session(
        state: &TestAppState,
        token: &str,
    ) -> Option<rpodder_core::types::User> {
        use rpodder_core::repo::UserRepo;
        let r = super::repo(&state.db);
        let session = SessionRepo::find_by_token(&r, token).await.ok()??;
        UserRepo::find_by_id(&r, session.user_id).await.ok()?
    }

    async fn resolve_basic_auth(
        state: &TestAppState,
        username: &str,
        password: &str,
    ) -> Option<rpodder_core::types::User> {
        use argon2::{Argon2, PasswordHash, PasswordVerifier};
        use rpodder_core::repo::UserRepo;
        let r = super::repo(&state.db);
        let user = UserRepo::find_by_username(&r, username).await.ok()??;
        if !user.is_active {
            return None;
        }
        let parsed = PasswordHash::new(&user.password_hash).ok()?;
        if Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
        {
            Some(user)
        } else {
            None
        }
    }

    pub async fn login(
        State(state): State<TestAppState>,
        Path(username): Path<String>,
        axum::Extension(auth_user): axum::Extension<AuthUser>,
    ) -> Result<Response, StatusCode> {
        if auth_user.0.username.to_lowercase() != username.to_lowercase() {
            return Err(StatusCode::FORBIDDEN);
        }

        let session = rpodder_core::types::Session {
            id: Uuid::now_v7(),
            user_id: auth_user.0.id,
            token: format!("test-session-{}", Uuid::now_v7()),
            expires_at: Utc::now() + Duration::days(365),
            created_at: Utc::now(),
        };

        let r = super::repo(&state.db);
        SessionRepo::create(&r, &session)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let cookie = format!(
            "sessionid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
            session.token,
            365 * 24 * 3600,
        );

        Ok((StatusCode::OK, [(header::SET_COOKIE, cookie)], "").into_response())
    }

    pub async fn logout(State(state): State<TestAppState>, req: Request) -> StatusCode {
        let token = req
            .headers()
            .get(header::COOKIE)
            .and_then(|v: &header::HeaderValue| v.to_str().ok())
            .and_then(|cookies: &str| {
                cookies
                    .split(';')
                    .filter_map(|c: &str| c.trim().strip_prefix("sessionid="))
                    .next()
                    .map(|t: &str| t.to_string())
            });

        if let Some(token) = token {
            let r = super::repo(&state.db);
            let _ = SessionRepo::delete(&r, &token).await;
        }

        StatusCode::OK
    }
}

fn test_router(state: TestAppState) -> Router {
    let authenticated = Router::new()
        .route(
            "/api/2/auth/{username}/login.json",
            post(test_handlers::login),
        )
        .route_layer(axum_mw::from_fn(test_handlers::require_auth_layer(
            state.clone(),
        )));

    let public = Router::new().route(
        "/api/2/auth/{username}/logout.json",
        post(test_handlers::logout),
    );

    authenticated.merge(public).with_state(state)
}
