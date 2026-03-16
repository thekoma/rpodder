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
    use argon2::password_hash::SaltString;
    use argon2::password_hash::rand_core::OsRng;
    use argon2::{Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .unwrap()
        .to_string()
}

async fn create_test_user(db: &Db, username: &str) -> String {
    let r = repo(db);
    let user = UserRepo::create(&r, username, &hash_password("pass"), None)
        .await
        .unwrap();

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

    let now = Utc::now();
    DeviceRepo::upsert(
        &r,
        &Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "phone".to_string(),
            caption: "Phone".to_string(),
            device_type: DeviceType::Mobile,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    token
}

// === Test router ===

#[derive(Clone)]
struct TestAppState {
    db: Arc<Db>,
}

mod test_handlers {
    use super::*;
    use axum::{
        Extension,
        extract::{Json, Path, Query, Request, State},
        http::{StatusCode, header},
        middleware::Next,
        response::{IntoResponse, Response},
    };
    use chrono::TimeZone;
    use rpodder_core::repo::{EpisodeActionRepo, EpisodeRepo, PodcastRepo};
    use rpodder_core::types::{EpisodeAction, EpisodeActionType};
    use serde::{Deserialize, Serialize};

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

    fn parse_action_type(s: &str) -> Option<EpisodeActionType> {
        match s {
            "download" => Some(EpisodeActionType::Download),
            "play" => Some(EpisodeActionType::Play),
            "delete" => Some(EpisodeActionType::Delete),
            "new" => Some(EpisodeActionType::New),
            _ => None,
        }
    }

    fn action_type_str(a: EpisodeActionType) -> &'static str {
        match a {
            EpisodeActionType::Download => "download",
            EpisodeActionType::Play => "play",
            EpisodeActionType::Delete => "delete",
            EpisodeActionType::New => "new",
        }
    }

    #[derive(Deserialize)]
    pub struct ActionInput {
        pub podcast: String,
        pub episode: String,
        pub device: Option<String>,
        pub action: String,
        pub timestamp: Option<String>,
        pub started: Option<i32>,
        pub position: Option<i32>,
        pub total: Option<i32>,
    }

    #[derive(Deserialize)]
    pub struct UploadBody {
        pub actions: Vec<ActionInput>,
    }

    #[derive(Serialize)]
    pub struct UploadResponse {
        pub timestamp: i64,
        pub update_urls: Vec<Vec<String>>,
    }

    #[derive(Serialize)]
    pub struct ActionResponse {
        pub podcast: String,
        pub episode: String,
        pub action: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub timestamp: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub started: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub position: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub total: Option<i32>,
    }

    #[derive(Serialize)]
    pub struct DownloadResponse {
        pub actions: Vec<ActionResponse>,
        pub timestamp: i64,
    }

    #[derive(Deserialize)]
    pub struct EpisodeQuery {
        pub since: Option<i64>,
    }

    pub async fn upload_actions(
        State(state): State<TestAppState>,
        Path(username_json): Path<String>,
        Extension(auth_user): Extension<AuthUser>,
        Json(body): Json<UploadBody>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let username = strip_json(&username_json);
        if auth_user.0.username.to_lowercase() != username.to_lowercase() {
            return Err(StatusCode::FORBIDDEN);
        }

        let r = super::repo(&state.db);

        for input in &body.actions {
            let action_type = parse_action_type(&input.action).ok_or(StatusCode::BAD_REQUEST)?;

            if action_type == EpisodeActionType::Play && input.position.is_none() {
                return Err(StatusCode::BAD_REQUEST);
            }

            let (podcast, _) = PodcastRepo::get_or_create_for_url(&r, &input.podcast)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let (episode, _) = EpisodeRepo::get_or_create_for_url(&r, podcast.id, &input.episode)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let device_uuid = if let Some(dev_str) = &input.device {
                DeviceRepo::find_by_uid(&r, auth_user.0.id, dev_str)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .map(|d| d.id)
            } else {
                None
            };

            let timestamp = input
                .timestamp
                .as_deref()
                .and_then(|ts| {
                    ts.parse().ok().or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                            .ok()
                            .map(|ndt| ndt.and_utc())
                    })
                })
                .unwrap_or_else(Utc::now);

            let ep_action = EpisodeAction {
                id: Uuid::now_v7(),
                user_id: auth_user.0.id,
                device_id: device_uuid,
                episode_id: episode.id,
                action: action_type,
                podcast_ref_url: Some(input.podcast.clone()),
                episode_ref_url: Some(input.episode.clone()),
                started: input.started,
                position: input.position,
                total: input.total,
                timestamp,
                created_at: Utc::now(),
            };
            EpisodeActionRepo::create(&r, &ep_action)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        Ok(Json(UploadResponse {
            timestamp: Utc::now().timestamp(),
            update_urls: Vec::new(),
        }))
    }

    pub async fn download_actions(
        State(state): State<TestAppState>,
        Path(username_json): Path<String>,
        Extension(auth_user): Extension<AuthUser>,
        Query(params): Query<EpisodeQuery>,
    ) -> Result<impl IntoResponse, StatusCode> {
        let username = strip_json(&username_json);
        if auth_user.0.username.to_lowercase() != username.to_lowercase() {
            return Err(StatusCode::FORBIDDEN);
        }

        let r = super::repo(&state.db);
        let since = params
            .since
            .and_then(|ts| Utc.timestamp_opt(ts, 0).single());

        let actions = EpisodeActionRepo::list(&r, auth_user.0.id, None, None, since, 1000)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let response_actions: Vec<ActionResponse> = actions
            .into_iter()
            .map(|a| ActionResponse {
                podcast: a.podcast_ref_url.unwrap_or_default(),
                episode: a.episode_ref_url.unwrap_or_default(),
                action: action_type_str(a.action).to_string(),
                timestamp: Some(a.timestamp.format("%Y-%m-%dT%H:%M:%S").to_string()),
                started: a.started,
                position: a.position,
                total: a.total,
            })
            .collect();

        Ok(Json(DownloadResponse {
            actions: response_actions,
            timestamp: Utc::now().timestamp(),
        }))
    }
}

fn test_router(state: TestAppState) -> Router {
    Router::new()
        .route(
            "/api/2/episodes/{username_json}",
            get(test_handlers::download_actions).post(test_handlers::upload_actions),
        )
        .route_layer(axum_mw::from_fn(test_handlers::require_auth_layer(
            state.clone(),
        )))
        .with_state(state)
}

fn cookie(token: &str) -> String {
    format!("sessionid={token}")
}

// === Tests ===

#[tokio::test]
async fn upload_and_download_episode_actions() {
    let db = setup_db().await;
    let token = create_test_user(&db, "alice").await;
    let state = TestAppState { db: Arc::new(db) };

    // Upload actions
    let app = test_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/episodes/alice.json")
                .header(header::COOKIE, cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"actions": [
                        {"podcast": "http://feed.com/rss", "episode": "http://feed.com/ep1.mp3",
                         "action": "play", "started": 0, "position": 120, "total": 3600,
                         "device": "phone", "timestamp": "2024-06-15T10:30:00"},
                        {"podcast": "http://feed.com/rss", "episode": "http://feed.com/ep2.mp3",
                         "action": "download", "device": "phone"}
                    ]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert!(result["timestamp"].as_i64().unwrap() > 0);

    // Download actions
    let app = test_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/episodes/alice.json?since=0")
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
    let result: Value = serde_json::from_slice(&body).unwrap();
    let actions = result["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 2);

    // First action should be the play (timestamp 2024, comes first when sorted)
    let play = &actions[0];
    assert_eq!(play["action"], "play");
    assert_eq!(play["position"], 120);
    assert_eq!(play["total"], 3600);
    assert_eq!(play["podcast"], "http://feed.com/rss");
    assert_eq!(play["episode"], "http://feed.com/ep1.mp3");
}

#[tokio::test]
async fn upload_invalid_action_type_returns_400() {
    let db = setup_db().await;
    let token = create_test_user(&db, "bob").await;

    let state = TestAppState { db: Arc::new(db) };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/episodes/bob.json")
                .header(header::COOKIE, cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"actions": [
                        {"podcast": "http://feed.com/rss", "episode": "http://feed.com/ep1.mp3",
                         "action": "invalid_action"}
                    ]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_play_without_position_returns_400() {
    let db = setup_db().await;
    let token = create_test_user(&db, "carol").await;

    let state = TestAppState { db: Arc::new(db) };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/2/episodes/carol.json")
                .header(header::COOKIE, cookie(&token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"actions": [
                        {"podcast": "http://feed.com/rss", "episode": "http://feed.com/ep1.mp3",
                         "action": "play"}
                    ]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn download_empty_actions() {
    let db = setup_db().await;
    let token = create_test_user(&db, "dave").await;

    let state = TestAppState { db: Arc::new(db) };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/episodes/dave.json")
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
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert!(result["actions"].as_array().unwrap().is_empty());
    assert!(result["timestamp"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn unauthenticated_returns_401() {
    let db = setup_db().await;
    let state = TestAppState { db: Arc::new(db) };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/episodes/anyone.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_user_returns_403() {
    let db = setup_db().await;
    let token = create_test_user(&db, "eve").await;

    let state = TestAppState { db: Arc::new(db) };
    let app = test_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/2/episodes/noteve.json")
                .header(header::COOKIE, cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
