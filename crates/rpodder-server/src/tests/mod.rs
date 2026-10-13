//! Tests that drive the real `api_router` (handlers + middleware as wired in
//! `main.rs`) over in-memory SQLite.

mod auth_hardening;
mod episode_sync;
mod feed_refresh;
mod opml_export;
mod subscription_sync;
mod web_player;

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use chrono::{Duration, Utc};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use rpodder_core::repo::{SessionRepo, UserRepo};
use rpodder_core::types::{Session, User};
use rpodder_db::{Db, sqlite::SqliteRepo};

use crate::config::AppConfig;
use crate::state::AppState;

pub struct TestApp {
    pub db: Arc<Db>,
    pub router: Router,
}

pub struct TestUser {
    pub user: User,
    pub cookie: String,
}

impl TestApp {
    pub async fn new() -> Self {
        Self::with_config(serde_json::json!({})).await
    }

    /// `config` holds `AppConfig` fields; the rest keep their defaults.
    pub async fn with_config(config: Value) -> Self {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        db.migrate("../../migrations").await.unwrap();
        let db = Arc::new(db);
        let config: AppConfig = serde_json::from_value(config).unwrap();
        let router = crate::api_router(AppState {
            db: db.clone(),
            config: Arc::new(config),
        });
        Self { db, router }
    }

    pub fn repo(&self) -> SqliteRepo {
        match &*self.db {
            Db::Sqlite(pool) => SqliteRepo::new(pool.clone()),
            _ => panic!("expected sqlite"),
        }
    }

    pub async fn create_user(&self, username: &str) -> TestUser {
        let repo = self.repo();
        let hash = crate::middleware::auth::hash_password("pass").unwrap();
        let user = UserRepo::create(&repo, username, &hash, None)
            .await
            .unwrap();
        let token = format!("session-{}", Uuid::now_v7());
        SessionRepo::create(
            &repo,
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
        TestUser {
            user,
            cookie: format!("sessionid={token}"),
        }
    }

    pub async fn get(&self, cookie: &str, uri: &str) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        self.send(req).await
    }

    pub async fn post_json(&self, cookie: &str, uri: &str, body: Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .method("POST")
            .uri(uri)
            .header(header::COOKIE, cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        self.send(req).await
    }

    async fn send(&self, req: Request<Body>) -> (StatusCode, Value) {
        let (status, _, json) = self.send_full(req).await;
        (status, json)
    }

    /// GET returning the raw body text (for non-JSON formats).
    pub async fn get_text(&self, cookie: &str, uri: &str) -> (StatusCode, HeaderMap, String) {
        let req = Request::builder()
            .uri(uri)
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            headers,
            String::from_utf8_lossy(&bytes).into_owned(),
        )
    }

    /// Like `send`, but also returns the response headers.
    pub async fn send_full(&self, req: Request<Body>) -> (StatusCode, HeaderMap, Value) {
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }
}
