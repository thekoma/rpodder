use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware as axum_mw,
    routing::get,
    Router,
};
use chrono::{Duration, Utc};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use rpodder_core::repo::{DeviceRepo, PodcastRepo, SessionRepo, SubscriptionRepo, UserRepo};
use rpodder_core::types::{Device, DeviceType, Session};
use rpodder_db::{sqlite::SqliteRepo, Db};

// === Shared test infrastructure ===

async fn setup_db() -> Db {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let schema = std::fs::read_to_string("../../migrations/sqlite/001_initial.up.sql").unwrap();
    sqlx::raw_sql(&schema).execute(&pool).await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .unwrap();
    Db::Sqlite(pool)
}

fn repo(db: &Db) -> SqliteRepo {
    match db {
        Db::Sqlite(pool) => SqliteRepo::new(pool.clone()),
        _ => panic!("expected sqlite"),
    }
}

fn hash_password(password: &str) -> String {
    use argon2::password_hash::rand_core::OsRng;
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

/// Create a user with a session and a device, returns (session_token, device_uid).
async fn create_test_user(db: &Db, username: &str) -> (String, String) {
    let r = repo(db);
    let user = UserRepo::create(&r, username, &hash_password("pass"), None)
        .await
        .unwrap();

    let token = format!("session-{}", Uuid::now_v7());
    let session = Session {
        id: Uuid::now_v7(),
        user_id: user.id,
        token: token.clone(),
        expires_at: Utc::now() + Duration::hours(1),
        created_at: Utc::now(),
    };
    SessionRepo::create(&r, &session).await.unwrap();

    let now = Utc::now();
    let device = Device {
        id: Uuid::now_v7(),
        user_id: user.id,
        device_id: "phone".to_string(),
        caption: "Phone".to_string(),
        device_type: DeviceType::Mobile,
        sync_group_id: None,
        created_at: now,
        updated_at: now,
    };
    DeviceRepo::upsert(&r, &device).await.unwrap();

    (token, "phone".to_string())
}

// === Test router that mirrors the server's subscription routes ===
// We replicate the handlers to test the full HTTP flow.

#[derive(Clone)]
struct TestAppState {
    db: Arc<Db>,
}

mod test_handlers {
    use super::*;
    use axum::{
        extract::{Json, Path, Query, Request, State},
        http::{header, StatusCode},
        middleware::Next,
        response::{IntoResponse, Response},
        Extension,
    };
    use chrono::TimeZone;
    use rpodder_core::types::SubscriptionAction;
    use serde::{Deserialize, Serialize};

    #[derive(Clone)]
    pub struct AuthUser(pub rpodder_core::types::User);

    pub fn require_auth_layer(
        state: TestAppState,
    ) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
           + Clone
           + Send {
        move |req, next| {
            let state = state.clone();
            Box::pin(require_auth_inner(state, req, next))
        }
    }

    async fn require_auth_inner(state: TestAppState, mut req: Request, next: Next) -> Response {
        let session_token = req
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies
                    .split(';')
                    .filter_map(|c| c.trim().strip_prefix("sessionid="))
                    .next()
                    .map(|s| s.to_string())
            });

        if let Some(token) = &session_token {
            let r = super::repo(&state.db);
            if let Ok(Some(session)) = SessionRepo::find_by_token(&r, token).await {
                if let Ok(Some(user)) = UserRepo::find_by_id(&r, session.user_id).await {
                    req.extensions_mut().insert(AuthUser(user));
                    return next.run(req).await;
                }
            }
        }

        StatusCode::UNAUTHORIZED.into_response()
    }

    fn strip_json(s: &str) -> &str {
        s.strip_suffix(".json").unwrap_or(s)
    }

    // GET /subscriptions/{username}/{deviceid_json}
    pub async fn get_device_subscriptions(
        State(state): State<TestAppState>,
        Path((_username, deviceid_json)): Path<(String, String)>,
        Extension(auth_user): Extension<AuthUser>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let deviceid = strip_json(&deviceid_json);
        let r = super::repo(&state.db);

        let device = DeviceRepo::find_by_uid(&r, auth_user.0.id, deviceid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        let subs = SubscriptionRepo::list_for_device(&r, auth_user.0.id, device.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let urls: Vec<String> = subs.into_iter().map(|s| s.ref_url).collect();
        Ok(Json(urls))
    }

    // PUT /subscriptions/{username}/{deviceid_json}
    pub async fn put_device_subscriptions(
        State(state): State<TestAppState>,
        Path((_username, deviceid_json)): Path<(String, String)>,
        Extension(auth_user): Extension<AuthUser>,
        Json(urls): Json<Vec<String>>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let deviceid = strip_json(&deviceid_json);
        let r = super::repo(&state.db);

        let device = DeviceRepo::find_by_uid(&r, auth_user.0.id, deviceid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        let current = SubscriptionRepo::list_for_device(&r, auth_user.0.id, device.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let current_urls: std::collections::HashSet<String> =
            current.iter().map(|s| s.ref_url.clone()).collect();
        let new_urls: std::collections::HashSet<String> = urls.into_iter().collect();

        for sub in &current {
            if !new_urls.contains(&sub.ref_url) {
                SubscriptionRepo::unsubscribe(&r, auth_user.0.id, device.id, sub.podcast_id)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }

        for url in &new_urls {
            if !current_urls.contains(url) {
                let (podcast, _) = PodcastRepo::get_or_create_for_url(&r, url)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                SubscriptionRepo::subscribe(&r, auth_user.0.id, device.id, podcast.id, url)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }

        Ok(StatusCode::OK)
    }

    // GET /subscriptions/{username_json}
    pub async fn get_user_subscriptions(
        State(state): State<TestAppState>,
        Path(username_json): Path<String>,
        Extension(auth_user): Extension<AuthUser>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let username = strip_json(&username_json);
        if auth_user.0.username.to_lowercase() != username.to_lowercase() {
            return Err(StatusCode::FORBIDDEN);
        }

        let r = super::repo(&state.db);
        let subs = SubscriptionRepo::list_for_user(&r, auth_user.0.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let urls: Vec<String> = subs.into_iter().map(|s| s.ref_url).collect();
        Ok(Json(urls))
    }

    #[derive(Deserialize)]
    pub struct DeltaUpload {
        pub add: Vec<String>,
        pub remove: Vec<String>,
    }

    #[derive(Serialize)]
    pub struct DeltaResponse {
        pub add: Vec<String>,
        pub remove: Vec<String>,
        pub timestamp: i64,
    }

    #[derive(Deserialize)]
    pub struct SinceQuery {
        pub since: Option<i64>,
    }

    // POST /api/2/subscriptions/{username}/{deviceid_json}
    pub async fn upload_changes(
        State(state): State<TestAppState>,
        Path((_username, deviceid_json)): Path<(String, String)>,
        Extension(auth_user): Extension<AuthUser>,
        Json(body): Json<DeltaUpload>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let deviceid = strip_json(&deviceid_json);
        let r = super::repo(&state.db);

        let device = DeviceRepo::find_by_uid(&r, auth_user.0.id, deviceid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        for url in &body.remove {
            if let Ok(Some(podcast)) = PodcastRepo::find_by_url(&r, url).await {
                let _ =
                    SubscriptionRepo::unsubscribe(&r, auth_user.0.id, device.id, podcast.id).await;
            }
        }

        for url in &body.add {
            let (podcast, _) = PodcastRepo::get_or_create_for_url(&r, url)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            SubscriptionRepo::subscribe(&r, auth_user.0.id, device.id, podcast.id, url)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        Ok(Json(DeltaResponse {
            add: body.add,
            remove: body.remove,
            timestamp: Utc::now().timestamp(),
        }))
    }

    // GET /api/2/subscriptions/{username}/{deviceid_json}?since=T
    pub async fn download_changes(
        State(state): State<TestAppState>,
        Path((_username, deviceid_json)): Path<(String, String)>,
        Extension(auth_user): Extension<AuthUser>,
        Query(params): Query<SinceQuery>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let deviceid = strip_json(&deviceid_json);
        let r = super::repo(&state.db);

        let device = DeviceRepo::find_by_uid(&r, auth_user.0.id, deviceid)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        let since = params
            .since
            .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
            .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap());

        let changes = SubscriptionRepo::changes_since(&r, auth_user.0.id, device.id, since)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut add = Vec::new();
        let mut remove = Vec::new();
        for change in changes {
            match change.action {
                SubscriptionAction::Subscribe => add.push(change.ref_url),
                SubscriptionAction::Unsubscribe => remove.push(change.ref_url),
            }
        }

        Ok(Json(DeltaResponse {
            add,
            remove,
            timestamp: Utc::now().timestamp(),
        }))
    }
}

fn test_router(state: TestAppState) -> Router {
    let authenticated = Router::new()
        .route(
            "/subscriptions/{username}/{deviceid_json}",
            get(test_handlers::get_device_subscriptions)
                .put(test_handlers::put_device_subscriptions),
        )
        .route(
            "/subscriptions/{username_json}",
            get(test_handlers::get_user_subscriptions),
        )
        .route(
            "/api/2/subscriptions/{username}/{deviceid_json}",
            get(test_handlers::download_changes).post(test_handlers::upload_changes),
        )
        .route_layer(axum_mw::from_fn(
            test_handlers::require_auth_layer(state.clone()),
        ));

    authenticated.with_state(state)
}

fn cookie(token: &str) -> String {
    format!("sessionid={token}")
}

// === Tests ===

#[tokio::test]
async fn get_device_subscriptions_empty() {
    let db = setup_db().await;
    let (token, _dev) = create_test_user(&db, "alice").await;

    let state = TestAppState {
        db: Arc::new(db),
    };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/subscriptions/alice/phone.json")
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let urls: Vec<String> = serde_json::from_slice(&body).unwrap();
    assert!(urls.is_empty());
}

#[tokio::test]
async fn put_and_get_device_subscriptions() {
    let db = setup_db().await;
    let (token, _dev) = create_test_user(&db, "bob").await;

    let state = TestAppState {
        db: Arc::new(db),
    };

    // PUT subscriptions
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/subscriptions/bob/phone.json")
                .header(header::COOKIE, cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"["http://feed1.com/rss", "http://feed2.com/rss"]"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // GET subscriptions
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/subscriptions/bob/phone.json")
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let mut urls: Vec<String> = serde_json::from_slice(&body).unwrap();
    urls.sort();
    assert_eq!(urls, vec!["http://feed1.com/rss", "http://feed2.com/rss"]);
}

#[tokio::test]
async fn put_replaces_subscriptions() {
    let db = setup_db().await;
    let (token, _dev) = create_test_user(&db, "carol").await;

    let state = TestAppState {
        db: Arc::new(db),
    };

    // Initial subscriptions
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri("/subscriptions/carol/phone.json")
            .header(header::COOKIE, cookie(&token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"["http://feed1.com/rss", "http://feed2.com/rss"]"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    // Replace with a different set
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri("/subscriptions/carol/phone.json")
            .header(header::COOKIE, cookie(&token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"["http://feed2.com/rss", "http://feed3.com/rss"]"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    // Check: feed1 gone, feed2 remains, feed3 added
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/subscriptions/carol/phone.json")
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let mut urls: Vec<String> = serde_json::from_slice(&body).unwrap();
    urls.sort();
    assert_eq!(urls, vec!["http://feed2.com/rss", "http://feed3.com/rss"]);
}

#[tokio::test]
async fn get_user_subscriptions_across_devices() {
    let db = setup_db().await;
    let (token, _dev) = create_test_user(&db, "dave").await;

    // Create a second device
    let r = repo(&db);
    let user = UserRepo::find_by_username(&r, "dave")
        .await
        .unwrap()
        .unwrap();
    let now = Utc::now();
    DeviceRepo::upsert(
        &r,
        &Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "laptop".to_string(),
            caption: "Laptop".to_string(),
            device_type: DeviceType::Laptop,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    let state = TestAppState {
        db: Arc::new(db),
    };

    // Subscribe phone to feed1
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri("/subscriptions/dave/phone.json")
            .header(header::COOKIE, cookie(&token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"["http://feed1.com/rss"]"#))
            .unwrap(),
    )
    .await
    .unwrap();

    // Subscribe laptop to feed2
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("PUT")
            .uri("/subscriptions/dave/laptop.json")
            .header(header::COOKIE, cookie(&token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"["http://feed2.com/rss"]"#))
            .unwrap(),
    )
    .await
    .unwrap();

    // GET all user subscriptions
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/subscriptions/dave.json")
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let mut urls: Vec<String> = serde_json::from_slice(&body).unwrap();
    urls.sort();
    assert_eq!(urls, vec!["http://feed1.com/rss", "http://feed2.com/rss"]);
}

#[tokio::test]
async fn delta_upload_and_download() {
    let db = setup_db().await;
    let (token, _dev) = create_test_user(&db, "eve").await;

    let state = TestAppState {
        db: Arc::new(db),
    };

    let since_ts = Utc::now().timestamp() - 1;

    // Upload delta: add two feeds
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/subscriptions/eve/phone.json")
                .header(header::COOKIE, cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"add": ["http://a.com/rss", "http://b.com/rss"], "remove": []}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let delta: Value = serde_json::from_slice(&body).unwrap();
    assert!(delta["timestamp"].as_i64().unwrap() > 0);

    // Download changes since before the upload
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!(
                    "/api/2/subscriptions/eve/phone.json?since={since_ts}"
                ))
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let delta: Value = serde_json::from_slice(&body).unwrap();
    let mut add: Vec<String> = delta["add"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    add.sort();
    assert_eq!(add, vec!["http://a.com/rss", "http://b.com/rss"]);
    assert!(delta["remove"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn delta_upload_remove() {
    let db = setup_db().await;
    let (token, _dev) = create_test_user(&db, "frank").await;

    let state = TestAppState {
        db: Arc::new(db),
    };

    // First add a feed
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/2/subscriptions/frank/phone.json")
            .header(header::COOKIE, cookie(&token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"add": ["http://removeme.com/rss"], "remove": []}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    // Remove it
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/2/subscriptions/frank/phone.json")
            .header(header::COOKIE, cookie(&token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"add": [], "remove": ["http://removeme.com/rss"]}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    // Check it's gone
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/subscriptions/frank/phone.json")
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let urls: Vec<String> = serde_json::from_slice(&body).unwrap();
    assert!(urls.is_empty());
}

#[tokio::test]
async fn nonexistent_device_returns_404() {
    let db = setup_db().await;
    let (token, _dev) = create_test_user(&db, "ghost").await;

    let state = TestAppState {
        db: Arc::new(db),
    };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/subscriptions/ghost/nonexistent.json")
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unauthenticated_returns_401() {
    let db = setup_db().await;
    let state = TestAppState {
        db: Arc::new(db),
    };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/subscriptions/anyone/phone.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
