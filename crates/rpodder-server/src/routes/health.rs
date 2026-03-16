use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::state::AppState;
use rpodder_db::Db;

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
