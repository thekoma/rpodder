use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::state::AppState;
use rpodder_db::Db;

/// GET /metrics — Prometheus-compatible metrics
pub async fn metrics(State(state): State<AppState>) -> Result<String, StatusCode> {
    let mut out = String::new();

    let (users, podcasts, episodes, subs, actions, devices) = match &*state.db {
        Db::Postgres(pool) => {
            let u: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let p: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM podcasts")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let ep: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodes")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let s: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM subscriptions")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let a: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episode_actions")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let d: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            (u.0, p.0, ep.0, s.0, a.0, d.0)
        }
        Db::Sqlite(pool) => {
            let u: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let p: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM podcasts")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let ep: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodes")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let s: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM subscriptions")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let a: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episode_actions")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let d: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices")
                .fetch_one(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            (u.0, p.0, ep.0, s.0, a.0, d.0)
        }
    };

    out.push_str("# HELP rpodder_users_total Number of registered users\n");
    out.push_str("# TYPE rpodder_users_total gauge\n");
    out.push_str(&format!("rpodder_users_total {users}\n"));
    out.push_str("# HELP rpodder_podcasts_total Number of indexed podcasts\n");
    out.push_str("# TYPE rpodder_podcasts_total gauge\n");
    out.push_str(&format!("rpodder_podcasts_total {podcasts}\n"));
    out.push_str("# HELP rpodder_episodes_total Number of indexed episodes\n");
    out.push_str("# TYPE rpodder_episodes_total gauge\n");
    out.push_str(&format!("rpodder_episodes_total {episodes}\n"));
    out.push_str("# HELP rpodder_subscriptions_total Number of active subscriptions\n");
    out.push_str("# TYPE rpodder_subscriptions_total gauge\n");
    out.push_str(&format!("rpodder_subscriptions_total {subs}\n"));
    out.push_str("# HELP rpodder_episode_actions_total Number of episode actions\n");
    out.push_str("# TYPE rpodder_episode_actions_total gauge\n");
    out.push_str(&format!("rpodder_episode_actions_total {actions}\n"));
    out.push_str("# HELP rpodder_devices_total Number of registered devices\n");
    out.push_str("# TYPE rpodder_devices_total gauge\n");
    out.push_str(&format!("rpodder_devices_total {devices}\n"));

    Ok(out)
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub database: &'static str,
}

/// GET /health — lightweight health check for load balancers
pub async fn health(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    // Quick DB check
    let db_ok = match &*state.db {
        Db::Postgres(pool) => sqlx::query("SELECT 1").execute(pool).await.is_ok(),
        Db::Sqlite(pool) => sqlx::query("SELECT 1").execute(pool).await.is_ok(),
    };

    if !db_ok {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let db_type = match &*state.db {
        Db::Postgres(_) => "postgresql",
        Db::Sqlite(_) => "sqlite",
    };

    Ok(Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        database: db_type,
    }))
}
