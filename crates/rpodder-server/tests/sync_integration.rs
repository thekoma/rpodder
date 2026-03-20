use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    middleware as axum_mw,
    routing::get,
};
use chrono::{Duration, Utc};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use rpodder_core::repo::{DeviceRepo, SessionRepo, UserRepo};
use rpodder_core::types::{Device, DeviceType, Session};
use rpodder_db::{Db, sqlite::SqliteRepo};

async fn setup_db() -> Db {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let schema = std::fs::read_to_string("../../migrations/sqlite/001_initial.up.sql").unwrap();
    sqlx::raw_sql(&schema).execute(&pool).await.unwrap();
    let m2 = std::fs::read_to_string("../../migrations/sqlite/002_add_user_roles.up.sql").unwrap();
    sqlx::raw_sql(&m2).execute(&pool).await.unwrap();
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
    use argon2::password_hash::SaltString;
    use argon2::password_hash::rand_core::OsRng;
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

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

async fn create_device(db: &Db, username: &str, device_id: &str) {
    let r = repo(db);
    let user = UserRepo::find_by_username(&r, username)
        .await
        .unwrap()
        .unwrap();
    let now = Utc::now();
    let device = Device {
        id: Uuid::now_v7(),
        user_id: user.id,
        device_id: device_id.to_string(),
        caption: device_id.to_string(),
        device_type: DeviceType::Other,
        sync_group_id: None,
        created_at: now,
        updated_at: now,
    };
    DeviceRepo::upsert(&r, &device).await.unwrap();
}

// === Router setup ===

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
        Extension,
        extract::{Request, State},
        http::{StatusCode, header},
        middleware::Next,
        response::{IntoResponse, Response},
    };

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
        let session_token = req
            .headers()
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .and_then(|c| {
                c.split(';')
                    .filter_map(|c| c.trim().strip_prefix("sessionid="))
                    .next()
                    .map(|s| s.to_string())
            });

        if let Some(token) = &session_token {
            let r = super::repo(&state.db);
            if let Ok(Some(session)) =
                rpodder_core::repo::SessionRepo::find_by_token(&r, token).await
            {
                if let Ok(Some(user)) =
                    rpodder_core::repo::UserRepo::find_by_id(&r, session.user_id).await
                {
                    req.extensions_mut().insert(AuthUser(user));
                    return next.run(req).await;
                }
            }
        }

        StatusCode::UNAUTHORIZED.into_response()
    }

    use axum::extract::{Json, Path};
    use rpodder_core::repo::SyncGroupRepo;
    use serde::{Deserialize, Serialize};

    fn strip_json(s: &str) -> &str {
        s.strip_suffix(".json").unwrap_or(s)
    }

    #[derive(Serialize)]
    pub struct SyncStatusResponse {
        pub synchronized: Vec<Vec<String>>,
        #[serde(rename = "not-synchronized")]
        pub not_synchronized: Vec<String>,
    }

    #[derive(Deserialize)]
    pub struct SyncUpdateRequest {
        pub synchronize: Vec<Vec<String>>,
        #[serde(rename = "stop-synchronize")]
        pub stop_synchronize: Option<Vec<String>>,
    }

    pub async fn get_sync_status(
        State(state): State<TestAppState>,
        Path(username_json): Path<String>,
        Extension(auth_user): Extension<AuthUser>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let username = strip_json(&username_json);
        if auth_user.0.username.to_lowercase() != username.to_lowercase() {
            return Err(StatusCode::FORBIDDEN);
        }

        let r = super::repo(&state.db);
        let groups = SyncGroupRepo::get_groups_for_user(&r, auth_user.0.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let all_devices = DeviceRepo::list_for_user(&r, auth_user.0.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut synchronized: Vec<Vec<String>> = Vec::new();
        let mut synced_device_ids = std::collections::HashSet::new();

        for (_, devices) in &groups {
            if devices.len() > 1 {
                let group_devices: Vec<String> =
                    devices.iter().map(|d| d.device_id.clone()).collect();
                for d in devices {
                    synced_device_ids.insert(d.device_id.clone());
                }
                synchronized.push(group_devices);
            }
        }

        let not_synchronized: Vec<String> = all_devices
            .iter()
            .filter(|d| !synced_device_ids.contains(&d.device_id))
            .map(|d| d.device_id.clone())
            .collect();

        Ok(Json(SyncStatusResponse {
            synchronized,
            not_synchronized,
        }))
    }

    pub async fn update_sync_status(
        State(state): State<TestAppState>,
        Path(username_json): Path<String>,
        Extension(auth_user): Extension<AuthUser>,
        Json(body): Json<SyncUpdateRequest>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let username = strip_json(&username_json);
        if auth_user.0.username.to_lowercase() != username.to_lowercase() {
            return Err(StatusCode::FORBIDDEN);
        }

        let r = super::repo(&state.db);

        // Stop synchronizing devices
        if let Some(stop) = &body.stop_synchronize {
            for device_uid in stop {
                let device = DeviceRepo::find_by_uid(&r, auth_user.0.id, device_uid)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                if let Some(device) = device {
                    SyncGroupRepo::remove_device_from_group(&r, device.id)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }
        }

        // Create sync groups
        for group_uids in &body.synchronize {
            let mut device_ids = Vec::new();
            for uid in group_uids {
                let device = DeviceRepo::find_by_uid(&r, auth_user.0.id, uid)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                if let Some(d) = device {
                    device_ids.push(d.id);
                }
            }
            if device_ids.len() > 1 {
                SyncGroupRepo::create_group(&r, auth_user.0.id, &device_ids)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
        }

        Ok(StatusCode::OK)
    }
}

fn test_router(state: TestAppState) -> Router {
    let authenticated = Router::new()
        .route(
            "/api/2/sync-devices/{username_json}",
            get(test_handlers::get_sync_status).post(test_handlers::update_sync_status),
        )
        .route_layer(axum_mw::from_fn(test_handlers::require_auth_layer(
            state.clone(),
        )));

    authenticated.with_state(state)
}

fn auth_cookie(token: &str) -> String {
    format!("sessionid={token}")
}

async fn get_body(resp: axum::http::Response<Body>) -> Value {
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

// === Tests ===

#[tokio::test]
async fn get_sync_status_no_devices() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "alice", "pass").await;

    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/alice.json")
                .header(header::COOKIE, auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = get_body(resp).await;
    assert_eq!(body["synchronized"], serde_json::json!([]));
    assert_eq!(body["not-synchronized"], serde_json::json!([]));
}

#[tokio::test]
async fn get_sync_status_all_unsynced() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "bob", "pass").await;
    create_device(&db, "bob", "phone").await;
    create_device(&db, "bob", "laptop").await;

    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/bob.json")
                .header(header::COOKIE, auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = get_body(resp).await;
    assert_eq!(body["synchronized"], serde_json::json!([]));
    let unsynced = body["not-synchronized"].as_array().unwrap();
    assert_eq!(unsynced.len(), 2);
    assert!(unsynced.contains(&serde_json::json!("phone")));
    assert!(unsynced.contains(&serde_json::json!("laptop")));
}

#[tokio::test]
async fn create_sync_group() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "carol", "pass").await;
    create_device(&db, "carol", "phone").await;
    create_device(&db, "carol", "laptop").await;
    create_device(&db, "carol", "tablet").await;

    let state = test_state(db);

    // Create sync group with phone + laptop
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/sync-devices/carol.json")
                .header(header::COOKIE, auth_cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"synchronize": [["phone", "laptop"]]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify status
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/carol.json")
                .header(header::COOKIE, auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = get_body(resp).await;
    let synced = body["synchronized"].as_array().unwrap();
    assert_eq!(synced.len(), 1);
    let group = synced[0].as_array().unwrap();
    assert_eq!(group.len(), 2);
    assert!(group.contains(&serde_json::json!("phone")));
    assert!(group.contains(&serde_json::json!("laptop")));

    let unsynced = body["not-synchronized"].as_array().unwrap();
    assert_eq!(unsynced.len(), 1);
    assert!(unsynced.contains(&serde_json::json!("tablet")));
}

#[tokio::test]
async fn stop_synchronize_device() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "dave", "pass").await;
    create_device(&db, "dave", "phone").await;
    create_device(&db, "dave", "laptop").await;

    let state = test_state(db);

    // Create sync group
    let app = test_router(state.clone());
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/2/sync-devices/dave.json")
            .header(header::COOKIE, auth_cookie(&token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"synchronize": [["phone", "laptop"]]}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();

    // Remove phone from group
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/sync-devices/dave.json")
                .header(header::COOKIE, auth_cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"synchronize": [], "stop-synchronize": ["phone"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify both are now unsynced (group with 1 device is not a group)
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/dave.json")
                .header(header::COOKIE, auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = get_body(resp).await;
    assert_eq!(body["synchronized"], serde_json::json!([]));
    let unsynced = body["not-synchronized"].as_array().unwrap();
    assert_eq!(unsynced.len(), 2);
}

#[tokio::test]
async fn sync_status_wrong_user_returns_403() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "eve", "pass").await;
    let r = repo(&db);
    UserRepo::create(&r, "frank", &hash_password("pass"), None)
        .await
        .unwrap();

    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/frank.json")
                .header(header::COOKIE, auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn sync_without_auth_returns_401() {
    let db = setup_db().await;
    let state = test_state(db);
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/nobody.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_group_with_single_device_is_noop() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "grace", "pass").await;
    create_device(&db, "grace", "phone").await;

    let state = test_state(db);

    // Try to create a group with just one device
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/sync-devices/grace.json")
                .header(header::COOKIE, auth_cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"synchronize": [["phone"]]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify no group was created
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/grace.json")
                .header(header::COOKIE, auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = get_body(resp).await;
    assert_eq!(body["synchronized"], serde_json::json!([]));
    let unsynced = body["not-synchronized"].as_array().unwrap();
    assert_eq!(unsynced.len(), 1);
}

#[tokio::test]
async fn create_multiple_sync_groups() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "hank", "pass").await;
    create_device(&db, "hank", "phone").await;
    create_device(&db, "hank", "laptop").await;
    create_device(&db, "hank", "tablet").await;
    create_device(&db, "hank", "desktop").await;

    let state = test_state(db);

    // Create two sync groups
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/sync-devices/hank.json")
                .header(header::COOKIE, auth_cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"synchronize": [["phone", "laptop"], ["tablet", "desktop"]]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify both groups exist
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/2/sync-devices/hank.json")
                .header(header::COOKIE, auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = get_body(resp).await;
    let synced = body["synchronized"].as_array().unwrap();
    assert_eq!(synced.len(), 2);
    assert_eq!(body["not-synchronized"], serde_json::json!([]));
}
