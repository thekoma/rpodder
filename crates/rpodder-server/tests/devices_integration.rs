use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware as axum_mw,
    routing::{get, post},
    Router,
};
use chrono::{Duration, Utc};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use rpodder_core::repo::{SessionRepo, UserRepo};
use rpodder_core::types::Session;
use rpodder_db::{sqlite::SqliteRepo, Db};

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

fn basic_auth_header(username: &str, password: &str) -> String {
    use base64::Engine;
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    format!("Basic {encoded}")
}

/// Create a test user and return a session token for auth.
async fn create_user_with_session(db: &Db, username: &str, password: &str) -> String {
    let r = repo(db);
    let user = UserRepo::create(&r, username, &hash_password(password), None)
        .await
        .unwrap();

    let token = format!("test-session-{}", Uuid::now_v7());
    let session = Session {
        id: Uuid::now_v7(),
        user_id: user.id,
        token: token.clone(),
        expires_at: Utc::now() + Duration::hours(1),
        created_at: Utc::now(),
    };
    SessionRepo::create(&r, &session).await.unwrap();
    token
}

// === Router setup (mirrors server internals) ===

#[derive(Clone)]
struct TestAppState {
    db: Arc<Db>,
}

fn test_state(db: Db) -> TestAppState {
    TestAppState { db: Arc::new(db) }
}

mod test_handlers {
    use super::*;
    use axum::{
        extract::{Json, Path, Request, State},
        http::{header, StatusCode},
        middleware::Next,
        response::{IntoResponse, Response},
        Extension,
    };
    use rpodder_core::repo::DeviceRepo;
    use rpodder_core::types::DeviceType;
    use serde::Deserialize;

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
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)?
            .to_str()
            .ok()?;
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

    fn parse_device_type(s: &str) -> DeviceType {
        match s {
            "desktop" => DeviceType::Desktop,
            "laptop" => DeviceType::Laptop,
            "mobile" => DeviceType::Mobile,
            "server" => DeviceType::Server,
            "tablet" => DeviceType::Tablet,
            _ => DeviceType::Other,
        }
    }

    fn device_type_str(dt: DeviceType) -> &'static str {
        match dt {
            DeviceType::Desktop => "desktop",
            DeviceType::Laptop => "laptop",
            DeviceType::Mobile => "mobile",
            DeviceType::Server => "server",
            DeviceType::Tablet => "tablet",
            DeviceType::Other => "other",
        }
    }

    #[derive(Deserialize)]
    pub struct DeviceUpdateRequest {
        pub caption: Option<String>,
        #[serde(rename = "type")]
        pub device_type: Option<String>,
    }

    pub async fn update_device(
        State(state): State<TestAppState>,
        Path((_username, deviceid_json)): Path<(String, String)>,
        Extension(auth_user): Extension<AuthUser>,
        Json(body): Json<DeviceUpdateRequest>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let now = chrono::Utc::now();
        let deviceid = deviceid_json
            .strip_suffix(".json")
            .unwrap_or(&deviceid_json)
            .to_string();
        let dt = body
            .device_type
            .as_deref()
            .map(parse_device_type)
            .unwrap_or(DeviceType::Other);
        let caption = body.caption.unwrap_or_default();

        let device = rpodder_core::types::Device {
            id: Uuid::now_v7(),
            user_id: auth_user.0.id,
            device_id: deviceid,
            caption,
            device_type: dt,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        };

        let r = super::repo(&state.db);
        DeviceRepo::upsert(&r, &device)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(StatusCode::OK)
    }

    pub async fn list_devices(
        State(state): State<TestAppState>,
        Path(username_json): Path<String>,
        Extension(auth_user): Extension<AuthUser>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let username = username_json
            .strip_suffix(".json")
            .unwrap_or(&username_json);
        if auth_user.0.username.to_lowercase() != username.to_lowercase() {
            return Err(StatusCode::FORBIDDEN);
        }

        let r = super::repo(&state.db);
        let devices = DeviceRepo::list_for_user(&r, auth_user.0.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let response: Vec<serde_json::Value> = devices
            .into_iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.device_id,
                    "caption": d.caption,
                    "type": device_type_str(d.device_type),
                    "subscriptions": 0
                })
            })
            .collect();

        Ok(Json(response))
    }
}

fn test_router(state: TestAppState) -> Router {
    let authenticated = Router::new()
        .route(
            "/api/2/devices/{username}/{deviceid_json}",
            post(test_handlers::update_device),
        )
        .route(
            "/api/2/devices/{username_json}",
            get(test_handlers::list_devices),
        )
        .route_layer(axum_mw::from_fn(
            test_handlers::require_auth_layer(state.clone()),
        ));

    authenticated.with_state(state)
}

// === Tests ===

#[tokio::test]
async fn create_device_returns_200() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "alice", "pass").await;

    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/devices/alice/my-phone.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"caption": "My Phone", "type": "mobile"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_device_without_auth_returns_401() {
    let db = setup_db().await;
    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/devices/alice/my-phone.json")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"caption": "Phone"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_devices_returns_created_devices() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "bob", "pass").await;

    let state = test_state(db);

    // Create two devices
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/devices/bob/phone.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"caption": "My Phone", "type": "mobile"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/devices/bob/laptop.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"caption": "My Laptop", "type": "laptop"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // List devices
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/devices/bob.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let devices: Vec<Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0]["id"], "phone");
    assert_eq!(devices[0]["caption"], "My Phone");
    assert_eq!(devices[0]["type"], "mobile");
    assert_eq!(devices[1]["id"], "laptop");
    assert_eq!(devices[1]["caption"], "My Laptop");
    assert_eq!(devices[1]["type"], "laptop");
}

#[tokio::test]
async fn update_existing_device() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "carol", "pass").await;

    let state = test_state(db);

    // Create device
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/2/devices/carol/dev1.json")
            .header(header::COOKIE, format!("sessionid={token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"caption": "Old Name", "type": "mobile"}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    // Update device
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/devices/carol/dev1.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"caption": "New Name", "type": "laptop"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify update
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/devices/carol.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let devices: Vec<Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["caption"], "New Name");
    assert_eq!(devices[0]["type"], "laptop");
}

#[tokio::test]
async fn list_devices_wrong_user_returns_403() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "dave", "pass").await;
    // Create another user
    let r = repo(&db);
    UserRepo::create(&r, "eve", &hash_password("pass"), None)
        .await
        .unwrap();

    let state = test_state(db);
    let app = test_router(state);

    // dave tries to list eve's devices
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/devices/eve.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_devices_empty() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "frank", "pass").await;

    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/devices/frank.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let devices: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert!(devices.is_empty());
}

#[tokio::test]
async fn create_device_with_basic_auth() {
    let db = setup_db().await;
    let r = repo(&db);
    UserRepo::create(&r, "grace", &hash_password("mypass"), None)
        .await
        .unwrap();

    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/devices/grace/tablet1.json")
                .header(
                    header::AUTHORIZATION,
                    basic_auth_header("grace", "mypass"),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"caption": "My Tablet", "type": "tablet"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_device_default_type_is_other() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "hank", "pass").await;

    let state = test_state(db);

    // Create device without type field
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/2/devices/hank/noname.json")
            .header(header::COOKIE, format!("sessionid={token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"caption": "Unknown Device"}"#))
            .unwrap(),
    )
    .await
    .unwrap();

    // Verify type defaults to "other"
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/devices/hank.json")
                .header(header::COOKIE, format!("sessionid={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let devices: Vec<Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["type"], "other");
}
