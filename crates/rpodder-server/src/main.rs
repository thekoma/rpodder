mod middleware;
mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    middleware as axum_mw,
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use rpodder_db::Db;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "rpodder=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("RPODDER_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://rpodder:rpodder@localhost:5432/rpodder".into());
    let host = std::env::var("RPODDER_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("RPODDER_PORT")
        .unwrap_or_else(|_| "3005".into())
        .parse()?;

    let db = Db::connect(&database_url).await?;

    if std::env::var("RPODDER_RUN_MIGRATIONS").unwrap_or_default() == "true" {
        let migrations_dir = std::env::var("RPODDER_MIGRATIONS_DIR")
            .unwrap_or_else(|_| "migrations".into());
        db.migrate(&migrations_dir).await?;
    }

    let state = AppState { db: Arc::new(db) };
    let app = api_router(state);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    tracing::info!("rpodder listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn api_router(state: AppState) -> Router {
    // Routes that require authentication
    let authenticated = Router::new()
        .route("/api/2/auth/{username}/login.json", post(routes::auth::login))
        .route("/api/2/devices/{username}/{deviceid_json}", post(routes::devices::update_device))
        .route("/api/2/devices/{username_json}", get(routes::devices::list_devices))
        // Simple subscription API
        .route("/subscriptions/{username}/{deviceid_json}", get(routes::subscriptions::get_device_subscriptions).put(routes::subscriptions::put_device_subscriptions))
        .route("/subscriptions/{username_json}", get(routes::subscriptions::get_user_subscriptions))
        // Advanced subscription API
        .route("/api/2/subscriptions/{username}/{deviceid_json}", get(routes::subscriptions::download_subscription_changes).post(routes::subscriptions::upload_subscription_changes))
        // Episode actions API
        .route("/api/2/episodes/{username_json}", get(routes::episodes::download_episode_actions).post(routes::episodes::upload_episode_actions))
        .route_layer(axum_mw::from_fn(
            middleware::auth::require_auth_layer(state.clone()),
        ));

    // Public routes (no auth required)
    let public = Router::new()
        .route("/api/2/auth/{username}/logout.json", post(routes::auth::logout));

    authenticated
        .merge(public)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
