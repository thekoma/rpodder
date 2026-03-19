use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    middleware as axum_mw,
    routing::{get, post},
};
use chrono::{Duration, Utc};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use rpodder_core::repo::{
    DeviceRepo, EpisodeActionRepo, EpisodeRepo, PodcastRepo, SessionRepo, UserRepo,
};
use rpodder_core::types::{Device, DeviceType, EpisodeAction, EpisodeActionType, Session};
use rpodder_db::{Db, sqlite::SqliteRepo};

async fn setup_db() -> Db {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let schema = std::fs::read_to_string("../../migrations/sqlite/001_initial.up.sql").unwrap();
    sqlx::raw_sql(&schema).execute(&pool).await.unwrap();
    let migration2 =
        std::fs::read_to_string("../../migrations/sqlite/002_add_user_roles.up.sql").unwrap();
    sqlx::raw_sql(&migration2).execute(&pool).await.unwrap();
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

async fn create_user_with_session(db: &Db, username: &str) -> String {
    create_user_with_session_opts(db, username, false).await
}

async fn create_admin_with_session(db: &Db, username: &str) -> String {
    create_user_with_session_opts(db, username, true).await
}

async fn create_user_with_session_opts(db: &Db, username: &str, admin: bool) -> String {
    let r = repo(db);
    let user = UserRepo::create(&r, username, &hash_password("pass"), None)
        .await
        .unwrap();
    if admin {
        UserRepo::set_admin(&r, user.id, true).await.unwrap();
    }
    let token = format!("session-{}", Uuid::now_v7());
    SessionRepo::create(
        &r,
        &Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: token.clone(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        },
    )
    .await
    .unwrap();
    token
}

// Minimal test handlers replicating the auth middleware
#[derive(Clone)]
struct TestAppState {
    db: Arc<Db>,
}

mod test_handlers {
    use super::*;
    use axum::{
        Extension,
        extract::Request,
        http::header,
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
            Box::pin(async move {
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
                            let mut req = req;
                            req.extensions_mut().insert(AuthUser(user));
                            return next.run(req).await;
                        }
                    }
                }

                StatusCode::UNAUTHORIZED.into_response()
            })
        }
    }
}

fn cookie(token: &str) -> String {
    format!("sessionid={token}")
}

// === Admin API Tests ===

#[tokio::test]
async fn list_users_returns_all_users() {
    let db = setup_db().await;
    let token = create_user_with_session(&db, "admin").await;
    let r = repo(&db);
    UserRepo::create(&r, "user2", &hash_password("pass"), Some("u2@test.com"))
        .await
        .unwrap();

    let state = TestAppState { db: Arc::new(db) };
    let app = Router::new()
        .route("/api/admin/users", get(rpodder_server_test_admin_list))
        .route_layer(axum_mw::from_fn(test_handlers::require_auth_layer(
            state.clone(),
        )))
        .with_state(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/admin/users")
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
    let users: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(users.len(), 2);
    assert_eq!(users[0]["username"], "admin");
    assert_eq!(users[1]["username"], "user2");
    assert_eq!(users[1]["email"], "u2@test.com");
}

// We can't call the actual admin handler directly because it depends on the full
// rpodder-server internals. Instead, test the admin logic via the DB directly.

#[tokio::test]
async fn episode_history_query_works() {
    let db = setup_db().await;
    let r = repo(&db);
    let user = UserRepo::create(&r, "hist_user", &hash_password("pass"), None)
        .await
        .unwrap();
    let now = Utc::now();

    // Create a podcast + episode + action
    let (podcast, _) = PodcastRepo::get_or_create_for_url(&r, "http://test.com/feed")
        .await
        .unwrap();
    let (episode, _) =
        EpisodeRepo::get_or_create_for_url(&r, podcast.id, "http://test.com/ep1.mp3")
            .await
            .unwrap();

    let device = DeviceRepo::upsert(
        &r,
        &Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "phone".into(),
            caption: "Phone".into(),
            device_type: DeviceType::Mobile,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    EpisodeActionRepo::create(
        &r,
        &EpisodeAction {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: Some(device.id),
            episode_id: episode.id,
            action: EpisodeActionType::Play,
            podcast_ref_url: Some("http://test.com/feed".into()),
            episode_ref_url: Some("http://test.com/ep1.mp3".into()),
            started: Some(0),
            position: Some(120),
            total: Some(3600),
            timestamp: now,
            created_at: now,
        },
    )
    .await
    .unwrap();

    // Verify the enriched history query works
    let pool = match &db {
        Db::Sqlite(pool) => pool,
        _ => panic!("expected sqlite"),
    };

    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        Option<i32>,
        Option<i32>,
    )> = sqlx::query_as(
        "SELECT
                COALESCE(p.title, ea.podcast_ref_url, '') as podcast_title,
                COALESCE(ea.podcast_ref_url, '') as podcast_url,
                COALESCE(e.title, ea.episode_ref_url, '') as episode_title,
                ea.action,
                ea.timestamp,
                ea.position,
                ea.total
             FROM episode_actions ea
             LEFT JOIN episodes e ON e.id = ea.episode_id
             LEFT JOIN podcasts p ON p.id = e.podcast_id
             WHERE ea.user_id = ?
             ORDER BY ea.timestamp DESC
             LIMIT 50",
    )
    .bind(user.id.to_string())
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].3, "play"); // action
    assert_eq!(rows[0].5, Some(120)); // position
    assert_eq!(rows[0].6, Some(3600)); // total
}

#[tokio::test]
async fn sync_group_propagation_db_test() {
    use rpodder_core::repo::{SubscriptionRepo, SyncGroupRepo};

    let db = setup_db().await;
    let r = repo(&db);
    let user = UserRepo::create(&r, "sync_user", &hash_password("pass"), None)
        .await
        .unwrap();
    let now = Utc::now();

    // Create two devices
    let dev1 = DeviceRepo::upsert(
        &r,
        &Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "phone".into(),
            caption: "Phone".into(),
            device_type: DeviceType::Mobile,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    let dev2 = DeviceRepo::upsert(
        &r,
        &Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "laptop".into(),
            caption: "Laptop".into(),
            device_type: DeviceType::Laptop,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    // Create sync group
    let _group = SyncGroupRepo::create_group(&r, user.id, &[dev1.id, dev2.id])
        .await
        .unwrap();

    // Subscribe on dev1
    let (podcast, _) = PodcastRepo::get_or_create_for_url(&r, "http://test.com/feed")
        .await
        .unwrap();

    SubscriptionRepo::subscribe(&r, user.id, dev1.id, podcast.id, "http://test.com/feed")
        .await
        .unwrap();

    // Manually check dev1 has the subscription
    let subs1 = SubscriptionRepo::list_for_device(&r, user.id, dev1.id)
        .await
        .unwrap();
    assert_eq!(subs1.len(), 1);

    // Dev2 doesn't have it yet (propagation is in the HTTP handler, not the repo)
    let subs2 = SubscriptionRepo::list_for_device(&r, user.id, dev2.id)
        .await
        .unwrap();
    assert_eq!(subs2.len(), 0);

    // Manually propagate (simulating what the handler does)
    SubscriptionRepo::subscribe(&r, user.id, dev2.id, podcast.id, "http://test.com/feed")
        .await
        .unwrap();

    let subs2 = SubscriptionRepo::list_for_device(&r, user.id, dev2.id)
        .await
        .unwrap();
    assert_eq!(subs2.len(), 1);
    assert_eq!(subs2[0].ref_url, "http://test.com/feed");
}

// Dummy handler for testing — the actual admin handler needs the full server crate
async fn rpodder_server_test_admin_list(
    axum::extract::State(state): axum::extract::State<TestAppState>,
) -> Result<axum::response::Response<Body>, StatusCode> {
    let users: Vec<(String, String, Option<String>, bool)> = match &*state.db {
        Db::Sqlite(pool) => {
            sqlx::query_as("SELECT id, username, email, is_active FROM users ORDER BY created_at")
                .fetch_all(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
        _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let response: Vec<serde_json::Value> = users
        .into_iter()
        .map(|(_id, username, email, active)| {
            serde_json::json!({
                "username": username,
                "email": email,
                "active": active,
                "devices": 0,
                "subscriptions": 0,
            })
        })
        .collect();

    Ok(axum::Json(response).into_response())
}

use axum::response::IntoResponse;
