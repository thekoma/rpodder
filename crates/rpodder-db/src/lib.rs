//! Database layer for rpodder.
//!
//! Provides implementations of the repository traits defined in rpodder-core
//! for both PostgreSQL and SQLite via sqlx.

pub mod postgres;
pub mod sqlite;

use tracing::info;

/// Database backend, determined from the connection URL.
#[derive(Debug, Clone)]
pub enum Db {
    Postgres(sqlx::PgPool),
    Sqlite(sqlx::SqlitePool),
}

impl Db {
    /// Connect to the database based on the URL scheme.
    /// - `postgres://...` → PostgreSQL
    /// - `sqlite://...`   → SQLite
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(20)
                .connect(url)
                .await?;
            info!("Connected to PostgreSQL");
            Ok(Db::Postgres(pool))
        } else if url.starts_with("sqlite://") || url.starts_with("sqlite:") {
            // Ensure create_if_missing is set for file-based SQLite
            let connect_opts = url.parse::<sqlx::sqlite::SqliteConnectOptions>()?
                .create_if_missing(true);
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5)
                .connect_with(connect_opts)
                .await?;
            // Enable WAL mode and foreign keys for SQLite
            sqlx::query("PRAGMA journal_mode=WAL")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA foreign_keys=ON")
                .execute(&pool)
                .await?;
            info!("Connected to SQLite");
            Ok(Db::Sqlite(pool))
        } else {
            anyhow::bail!("Unsupported database URL scheme: {url}")
        }
    }

    /// Run migrations from the appropriate directory.
    pub async fn migrate(&self, migrations_dir: &str) -> anyhow::Result<()> {
        match self {
            Db::Postgres(pool) => {
                let sql = std::fs::read_to_string(format!("{migrations_dir}/postgresql/001_initial.up.sql"))?;
                sqlx::raw_sql(&sql).execute(pool).await?;
                info!("PostgreSQL migrations applied");
            }
            Db::Sqlite(pool) => {
                let sql = std::fs::read_to_string(format!("{migrations_dir}/sqlite/001_initial.up.sql"))?;
                sqlx::raw_sql(&sql).execute(pool).await?;
                info!("SQLite migrations applied");
            }
        }
        Ok(())
    }
}
