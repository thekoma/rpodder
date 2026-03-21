use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::state::AppState;
use rpodder_db::Db;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

/// GET /health — lightweight health check for load balancers (no sensitive info)
pub async fn health(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    let db_ok = match &*state.db {
        Db::Postgres(pool) => sqlx::query("SELECT 1").execute(pool).await.is_ok(),
        Db::Sqlite(pool) => sqlx::query("SELECT 1").execute(pool).await.is_ok(),
    };

    if !db_ok {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    Ok(Json(HealthResponse { status: "ok" }))
}

#[derive(Serialize)]
pub struct BuildInfo {
    pub status: &'static str,
    pub version: &'static str,
    pub database: &'static str,
    pub build_tag: String,
    pub build_sha: String,
}

/// GET /api/2/me/build — build info for authenticated users only
pub async fn build_info(State(state): State<AppState>) -> impl IntoResponse {
    let db_type = match &*state.db {
        Db::Postgres(_) => "postgresql",
        Db::Sqlite(_) => "sqlite",
    };

    Json(BuildInfo {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        database: db_type,
        build_tag: std::env::var("RPODDER_BUILD_TAG").unwrap_or_else(|_| "dev".into()),
        build_sha: std::env::var("RPODDER_BUILD_SHA").unwrap_or_else(|_| "local".into()),
    })
}

/// GET /metrics — Prometheus-compatible metrics (served on dedicated metrics port)
pub async fn metrics(State(state): State<AppState>) -> Result<String, StatusCode> {
    let mut out = String::new();

    // --- Business metrics ---
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

    // --- Process metrics ---
    out.push_str("# HELP rpodder_build_info Build metadata\n");
    out.push_str("# TYPE rpodder_build_info gauge\n");
    let version = env!("CARGO_PKG_VERSION");
    let tag = std::env::var("RPODDER_BUILD_TAG").unwrap_or_else(|_| "dev".into());
    let sha = std::env::var("RPODDER_BUILD_SHA").unwrap_or_else(|_| "local".into());
    let db_type = match &*state.db {
        Db::Postgres(_) => "postgresql",
        Db::Sqlite(_) => "sqlite",
    };
    out.push_str(&format!(
        "rpodder_build_info{{version=\"{version}\",tag=\"{tag}\",sha=\"{sha}\",database=\"{db_type}\"}} 1\n"
    ));

    // Process uptime (seconds since start)
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    let uptime = START.get_or_init(Instant::now).elapsed().as_secs();
    out.push_str("# HELP rpodder_uptime_seconds Seconds since process start\n");
    out.push_str("# TYPE rpodder_uptime_seconds counter\n");
    out.push_str(&format!("rpodder_uptime_seconds {uptime}\n"));

    // Resident memory (Linux only, read from /proc/self/statm)
    #[cfg(target_os = "linux")]
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if parts.len() >= 2 {
            let page_size: u64 = 4096;
            if let Ok(rss_pages) = parts[1].parse::<u64>() {
                let rss_bytes = rss_pages * page_size;
                out.push_str(
                    "# HELP process_resident_memory_bytes Resident memory size in bytes\n",
                );
                out.push_str("# TYPE process_resident_memory_bytes gauge\n");
                out.push_str(&format!("process_resident_memory_bytes {rss_bytes}\n"));
            }
            if let Ok(vsize_pages) = parts[0].parse::<u64>() {
                let vsize_bytes = vsize_pages * page_size;
                out.push_str("# HELP process_virtual_memory_bytes Virtual memory size in bytes\n");
                out.push_str("# TYPE process_virtual_memory_bytes gauge\n");
                out.push_str(&format!("process_virtual_memory_bytes {vsize_bytes}\n"));
            }
        }
    }

    // Open file descriptors (Linux only)
    #[cfg(target_os = "linux")]
    if let Ok(fds) = std::fs::read_dir("/proc/self/fd") {
        let count = fds.count() as u64;
        out.push_str("# HELP process_open_fds Number of open file descriptors\n");
        out.push_str("# TYPE process_open_fds gauge\n");
        out.push_str(&format!("process_open_fds {count}\n"));
    }

    // Max file descriptors (Linux only)
    #[cfg(target_os = "linux")]
    if let Ok(limits) = std::fs::read_to_string("/proc/self/limits") {
        for line in limits.lines() {
            if line.starts_with("Max open files") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(Ok(max_fds)) = parts.get(3).map(|s| s.parse::<u64>()) {
                    out.push_str(
                        "# HELP process_max_fds Maximum number of open file descriptors\n",
                    );
                    out.push_str("# TYPE process_max_fds gauge\n");
                    out.push_str(&format!("process_max_fds {max_fds}\n"));
                }
            }
        }
    }

    // Tokio runtime metrics (thread count)
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get() as u64)
        .unwrap_or(1);
    out.push_str("# HELP rpodder_available_cpus Number of available CPUs\n");
    out.push_str("# TYPE rpodder_available_cpus gauge\n");
    out.push_str(&format!("rpodder_available_cpus {num_cpus}\n"));

    // DB pool metrics
    match &*state.db {
        Db::Postgres(pool) => {
            out.push_str("# HELP rpodder_db_pool_size Current pool size\n");
            out.push_str("# TYPE rpodder_db_pool_size gauge\n");
            out.push_str(&format!("rpodder_db_pool_size {}\n", pool.size()));
            out.push_str("# HELP rpodder_db_pool_idle Number of idle connections\n");
            out.push_str("# TYPE rpodder_db_pool_idle gauge\n");
            out.push_str(&format!("rpodder_db_pool_idle {}\n", pool.num_idle()));
        }
        Db::Sqlite(pool) => {
            out.push_str("# HELP rpodder_db_pool_size Current pool size\n");
            out.push_str("# TYPE rpodder_db_pool_size gauge\n");
            out.push_str(&format!("rpodder_db_pool_size {}\n", pool.size()));
            out.push_str("# HELP rpodder_db_pool_idle Number of idle connections\n");
            out.push_str("# TYPE rpodder_db_pool_idle gauge\n");
            out.push_str(&format!("rpodder_db_pool_idle {}\n", pool.num_idle()));
        }
    }

    Ok(out)
}
