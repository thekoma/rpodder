mod config;
mod feed_updater;
mod middleware;
mod routes;
mod state;
#[cfg(feature = "web-ui")]
mod web_ui;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router, middleware as axum_mw,
    routing::{get, post},
};
use clap::{Parser, Subcommand};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use rpodder_db::Db;
use state::AppState;

/// rpodder — a modern gpodder.net-compatible podcast sync server
#[derive(Parser)]
#[command(name = "rpodder", version, about)]
struct Cli {
    /// Path to config file (TOML)
    #[arg(short, long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HTTP server
    Serve,

    /// Run database migrations
    Migrate,

    /// User management
    User {
        #[command(subcommand)]
        action: UserAction,
    },
}

#[derive(Subcommand)]
enum UserAction {
    /// Create a new user
    Create {
        /// Username
        username: String,
        /// Password
        password: String,
        /// Email (optional)
        #[arg(long)]
        email: Option<String>,
    },
    /// Delete a user
    Delete {
        /// Username
        username: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rpodder=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = config::AppConfig::load(cli.config.as_deref())?;

    match cli.command {
        Commands::Serve => cmd_serve(cfg).await,
        Commands::Migrate => cmd_migrate(cfg).await,
        Commands::User { action } => cmd_user(cfg, action).await,
    }
}

async fn cmd_serve(cfg: config::AppConfig) -> anyhow::Result<()> {
    let db = Db::connect(&cfg.database_url).await?;

    if cfg.run_migrations {
        tracing::info!("running migrations from {}", cfg.migrations_dir);
        db.migrate(&cfg.migrations_dir).await?;
    }

    let db = Arc::new(db);
    let state = AppState { db: db.clone() };
    let app = api_router(state);

    // Spawn background feed updater (every 30 minutes)
    let db_for_updater = db.clone();
    tokio::spawn(async move {
        // Wait a bit before first update to let the server start
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        feed_updater::run_feed_update_loop(db_for_updater, 1800).await;
    });

    let addr: SocketAddr = format!("{}:{}", cfg.host, cfg.port).parse()?;
    tracing::info!("rpodder listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("server shut down gracefully");
    Ok(())
}

async fn cmd_migrate(cfg: config::AppConfig) -> anyhow::Result<()> {
    let db = Db::connect(&cfg.database_url).await?;
    tracing::info!("running migrations from {}", cfg.migrations_dir);
    db.migrate(&cfg.migrations_dir).await?;
    tracing::info!("migrations complete");
    Ok(())
}

async fn cmd_user(cfg: config::AppConfig, action: UserAction) -> anyhow::Result<()> {
    use rpodder_core::repo::UserRepo;
    use rpodder_db::{postgres::PgRepo, sqlite::SqliteRepo};

    let db = Db::connect(&cfg.database_url).await?;

    match action {
        UserAction::Create {
            username,
            password,
            email,
        } => {
            let hash = middleware::auth::hash_password(&password)
                .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;

            match &db {
                Db::Postgres(pool) => {
                    let repo = PgRepo::new(pool.clone());
                    UserRepo::create(&repo, &username, &hash, email.as_deref()).await?;
                }
                Db::Sqlite(pool) => {
                    let repo = SqliteRepo::new(pool.clone());
                    UserRepo::create(&repo, &username, &hash, email.as_deref()).await?;
                }
            }

            tracing::info!("user '{}' created", username);
        }
        UserAction::Delete { username } => {
            // For now, just deactivate the user
            match &db {
                Db::Postgres(pool) => {
                    sqlx::query(
                        "UPDATE users SET is_active = false WHERE LOWER(username) = LOWER($1)",
                    )
                    .bind(&username)
                    .execute(pool)
                    .await?;
                }
                Db::Sqlite(pool) => {
                    sqlx::query("UPDATE users SET is_active = 0 WHERE username = ? COLLATE NOCASE")
                        .bind(&username)
                        .execute(pool)
                        .await?;
                }
            }

            tracing::info!("user '{}' deactivated", username);
        }
    }

    Ok(())
}

fn api_router(state: AppState) -> Router {
    // Routes that require authentication
    let authenticated = Router::new()
        .route(
            "/api/2/auth/{username}/login.json",
            post(routes::auth::login),
        )
        .route(
            "/api/2/devices/{username}/{deviceid_json}",
            post(routes::devices::update_device),
        )
        .route(
            "/api/2/devices/{username_json}",
            get(routes::devices::list_devices),
        )
        // Simple subscription API
        .route(
            "/subscriptions/{username}/{deviceid_json}",
            get(routes::subscriptions::get_device_subscriptions)
                .put(routes::subscriptions::put_device_subscriptions),
        )
        .route(
            "/subscriptions/{username_json}",
            get(routes::subscriptions::get_user_subscriptions),
        )
        // Advanced subscription API
        .route(
            "/api/2/subscriptions/{username}/{deviceid_json}",
            get(routes::subscriptions::download_subscription_changes)
                .post(routes::subscriptions::upload_subscription_changes),
        )
        // Suggestions
        .route(
            "/suggestions/{count_json}",
            get(routes::directory::suggestions),
        )
        // Sync devices
        .route(
            "/api/2/sync-devices/{username_json}",
            get(routes::sync::get_sync_status).post(routes::sync::update_sync_status),
        )
        // Settings
        .route(
            "/api/2/settings/{username}/{scope_json}",
            get(routes::settings::get_settings).post(routes::settings::update_settings),
        )
        // Favorites
        .route(
            "/api/2/favorites/{username_json}",
            get(routes::favorites::get_favorites),
        )
        // Chapters
        .route(
            "/api/2/chapters/{username_json}",
            get(routes::chapters::get_chapters).post(routes::chapters::update_chapters),
        )
        // Podcast lists
        .route(
            "/api/2/lists/{username}/create.json",
            post(routes::lists::create_list),
        )
        .route(
            "/api/2/lists/{username_json}",
            get(routes::lists::get_lists),
        )
        .route(
            "/api/2/lists/{username}/list/{slug_json}",
            get(routes::lists::get_list)
                .put(routes::lists::update_list)
                .delete(routes::lists::delete_list),
        )
        // Episode actions API
        .route(
            "/api/2/episodes/{username_json}",
            get(routes::episodes::download_episode_actions)
                .post(routes::episodes::upload_episode_actions),
        )
        .route_layer(axum_mw::from_fn(middleware::auth::require_auth_layer(
            state.clone(),
        )));

    // Public routes (no auth required)
    let public = Router::new()
        .route("/", get(routes::admin::status_page))
        .route("/health", get(routes::health::health))
        .route(
            "/api/2/auth/{username}/logout.json",
            post(routes::auth::logout),
        )
        // Directory & search (public)
        .route("/search.json", get(routes::directory::search))
        .route("/toplist/{count_json}", get(routes::directory::toplist))
        .route(
            "/api/2/data/podcast.json",
            get(routes::directory::podcast_data),
        )
        .route(
            "/api/2/data/episode.json",
            get(routes::directory::episode_data),
        )
        .route("/api/2/tags/{count_json}", get(routes::directory::top_tags))
        .route(
            "/api/2/tag/{tag}/{count_json}",
            get(routes::directory::podcasts_for_tag),
        );

    let mut app = authenticated
        .merge(public)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Embed web UI if feature is enabled
    #[cfg(feature = "web-ui")]
    {
        app = app.fallback(web_ui::serve_ui);
    }

    app
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl+C"),
        _ = terminate => tracing::info!("received SIGTERM"),
    }
}
