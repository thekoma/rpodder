#![allow(clippy::type_complexity)]
//! Database layer for rpodder.
//!
//! Provides implementations of the repository traits defined in rpodder-core
//! for both PostgreSQL and SQLite via sqlx.

pub mod postgres;
pub mod sqlite;

/// SQL shared by both backends for `EpisodeActionRepo::latest_plays`. `{user}`,
/// `{podcast}` and `{limit}` are placeholders the caller substitutes with its
/// bind syntax; the bracketed filters are added or removed by the caller.
pub(crate) fn latest_plays_sql(
    user: &str,
    podcast: Option<&str>,
    in_progress_only: bool,
    limit: Option<&str>,
    eff_total_cast: &str,
) -> String {
    // The window only ranks the user's play rows; metadata joins and the
    // podcast URL fallback run afterwards, once per episode.
    // Placeholders appear in bind order (user, podcast, limit): SQLite `?` is positional.
    let podcast_filter = podcast
        .map(|p| format!(" AND r.episode_id IN (SELECT id FROM episodes WHERE podcast_id = {p})"))
        .unwrap_or_default();
    let progress_filter = if in_progress_only {
        " AND pos > 0 AND (eff_total IS NULL OR pos < eff_total - 30)
          AND NOT EXISTS (SELECT 1 FROM episode_actions n
                           WHERE n.user_id = t.user_id AND n.episode_id = t.episode_id
                             AND n.action = 'new' AND n.timestamp > t.ts)"
    } else {
        ""
    };
    let limit = limit.map(|l| format!(" LIMIT {l}")).unwrap_or_default();
    format!(
        "SELECT podcast_url, podcast_title, podcast_logo_url, episode_url, episode_title,
                mimetype, pos, eff_total, ts, device_uid
         FROM (
           SELECT ea.user_id, ea.episode_id,
                  COALESCE(ea.podcast_ref_url,
                           (SELECT pu.url FROM podcast_urls pu
                             WHERE pu.podcast_id = e.podcast_id AND pu.\"order\" = 0)) AS podcast_url,
                  p.title AS podcast_title, p.logo_url AS podcast_logo_url,
                  ea.episode_ref_url AS episode_url, e.title AS episode_title,
                  e.mimetype AS mimetype, ea.position AS pos,
                  COALESCE(NULLIF(ea.total, 0), NULLIF(e.duration, 0)){eff_total_cast} AS eff_total,
                  ea.timestamp AS ts, d.device_id AS device_uid
           FROM (
             SELECT r.id,
                    ROW_NUMBER() OVER (PARTITION BY r.episode_id
                                       ORDER BY r.timestamp DESC, r.created_at DESC, r.id DESC) AS rn
             FROM episode_actions r
             WHERE r.user_id = {user} AND r.action = 'play'
                   AND r.episode_ref_url IS NOT NULL AND r.position IS NOT NULL{podcast_filter}
           ) latest
           JOIN episode_actions ea ON ea.id = latest.id
           JOIN episodes e ON e.id = ea.episode_id
           LEFT JOIN podcasts p ON p.id = e.podcast_id
           LEFT JOIN devices d ON d.id = ea.device_id
           WHERE latest.rn = 1
         ) t
         WHERE podcast_url IS NOT NULL{progress_filter}
         ORDER BY ts DESC{limit}"
    )
}

/// `None` for an empty title or one that is just the URL (episodes created from
/// an action upload get their URL as title).
pub(crate) fn meaningful_title(title: Option<String>, url: &str) -> Option<String> {
    title.filter(|t| !t.trim().is_empty() && t != url)
}

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
            // Apply pragmas at connect time so they hold for the connection.
            // WAL + synchronous=NORMAL keep write transactions short on slow
            // disks; busy_timeout makes a contending writer wait instead of
            // failing immediately. See issue #19.
            let connect_opts = url
                .parse::<sqlx::sqlite::SqliteConnectOptions>()?
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                .busy_timeout(std::time::Duration::from_secs(30))
                .foreign_keys(true);
            // SQLite permits only one writer. A single pooled connection makes
            // all access serialize in the async pool, so contenders queue
            // instead of colliding on the file lock and erroring with
            // "(code: 5) database is locked". A repo method holding a
            // transaction must not query `self.pool` until the transaction
            // ends: the second acquire would wait for the only connection,
            // which it holds itself (deadlock until the acquire timeout).
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(connect_opts)
                .await?;
            info!("Connected to SQLite");
            Ok(Db::Sqlite(pool))
        } else {
            anyhow::bail!("Unsupported database URL scheme: {url}")
        }
    }

    /// Run all `*.up.sql` migrations from the appropriate subdirectory, sorted by filename.
    /// Migrations must be idempotent (e.g. `IF NOT EXISTS`, `DO $$ ... EXCEPTION`).
    pub async fn migrate(&self, migrations_dir: &str) -> anyhow::Result<()> {
        let (subdir, pool_kind) = match self {
            Db::Postgres(_) => ("postgresql", "PostgreSQL"),
            Db::Sqlite(_) => ("sqlite", "SQLite"),
        };

        let files = Self::list_migrations(migrations_dir, subdir)?;

        for path in &files {
            let sql = std::fs::read_to_string(path)?;
            let fname = path.file_name().unwrap_or_default().to_string_lossy();
            match self {
                Db::Postgres(pool) => {
                    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
                        .execute(pool)
                        .await?;
                }
                Db::Sqlite(pool) => {
                    // SQLite ALTER TABLE ADD COLUMN is not idempotent — ignore
                    // "duplicate column" errors so migrations can be re-run safely.
                    match sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(pool).await {
                        Ok(_) => {}
                        Err(e) if e.to_string().contains("duplicate column") => {
                            info!("{pool_kind} migration skipped (already applied): {fname}");
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
            }
            info!("{pool_kind} migration applied: {fname}");
        }

        info!(
            "{pool_kind}: {count} migrations applied",
            count = files.len()
        );
        Ok(())
    }

    /// List migration files that would be applied for the given directory.
    /// Useful for testing and debugging.
    pub fn list_migrations(
        migrations_dir: &str,
        subdir: &str,
    ) -> anyhow::Result<Vec<std::path::PathBuf>> {
        let dir = format!("{migrations_dir}/{subdir}");
        let mut files: Vec<_> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|ext| ext == "sql")
                    && p.file_name()
                        .unwrap_or_default()
                        .to_str()
                        .is_some_and(|n| n.ends_with(".up.sql"))
            })
            .collect();
        files.sort();
        Ok(files)
    }

    /// Repair SQLite FTS5 index if corrupted.
    pub async fn repair(&self) -> anyhow::Result<()> {
        match self {
            Db::Sqlite(pool) => {
                info!("Rebuilding FTS5 index...");
                let repair_sql = r#"
                    DROP TRIGGER IF EXISTS trg_podcasts_fts_insert;
                    DROP TRIGGER IF EXISTS trg_podcasts_fts_update;
                    DROP TRIGGER IF EXISTS trg_podcasts_fts_delete;
                    DROP TABLE IF EXISTS podcasts_fts;
                    CREATE VIRTUAL TABLE IF NOT EXISTS podcasts_fts USING fts5(title, description, author, content=podcasts, content_rowid=rowid);
                    CREATE TRIGGER IF NOT EXISTS trg_podcasts_fts_insert AFTER INSERT ON podcasts BEGIN
                        INSERT INTO podcasts_fts(rowid, title, description, author) VALUES (NEW.rowid, NEW.title, NEW.description, NEW.author);
                    END;
                    CREATE TRIGGER IF NOT EXISTS trg_podcasts_fts_update AFTER UPDATE ON podcasts BEGIN
                        INSERT INTO podcasts_fts(podcasts_fts, rowid, title, description, author) VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.author);
                        INSERT INTO podcasts_fts(rowid, title, description, author) VALUES (NEW.rowid, NEW.title, NEW.description, NEW.author);
                    END;
                    CREATE TRIGGER IF NOT EXISTS trg_podcasts_fts_delete AFTER DELETE ON podcasts BEGIN
                        INSERT INTO podcasts_fts(podcasts_fts, rowid, title, description, author) VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.author);
                    END;
                    INSERT INTO podcasts_fts(podcasts_fts) VALUES('rebuild');
                "#;
                sqlx::raw_sql(repair_sql).execute(pool).await?;
                info!("FTS5 index rebuilt successfully");

                // Update subscriber counts for all podcasts
                sqlx::query(
                    "UPDATE podcasts SET subscribers = COALESCE((
                        SELECT COUNT(DISTINCT s.user_id) FROM subscriptions s WHERE s.podcast_id = podcasts.id
                     ), 0)",
                )
                .execute(pool)
                .await?;
                info!("Subscriber counts updated");
            }
            Db::Postgres(_) => {
                info!("No repair needed for PostgreSQL");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_migrations_finds_sqlite_files_sorted() {
        let files = Db::list_migrations("../../migrations", "sqlite").unwrap();
        assert!(
            files.len() >= 2,
            "expected at least 2 sqlite migrations, got {}",
            files.len()
        );
        // All files must end with .up.sql
        for f in &files {
            let name = f.file_name().unwrap().to_str().unwrap();
            assert!(name.ends_with(".up.sql"), "unexpected file: {name}");
        }
        // Files must be sorted (001 before 002)
        for w in files.windows(2) {
            assert!(
                w[0] < w[1],
                "migrations not sorted: {:?} >= {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn list_migrations_finds_postgresql_files_sorted() {
        let files = Db::list_migrations("../../migrations", "postgresql").unwrap();
        assert!(
            files.len() >= 2,
            "expected at least 2 pg migrations, got {}",
            files.len()
        );
        for w in files.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    #[test]
    fn list_migrations_excludes_down_files() {
        let files = Db::list_migrations("../../migrations", "sqlite").unwrap();
        for f in &files {
            let name = f.file_name().unwrap().to_str().unwrap();
            assert!(
                !name.contains(".down."),
                "down migration should be excluded: {name}"
            );
        }
    }

    #[tokio::test]
    async fn sqlite_migrate_is_idempotent() {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        // Run migrations twice — should not error
        db.migrate("../../migrations").await.unwrap();
        db.migrate("../../migrations").await.unwrap();

        // Verify is_admin column exists and works
        if let Db::Sqlite(pool) = &db {
            let has_admin: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('users') WHERE name = 'is_admin'",
            )
            .fetch_one(pool)
            .await
            .unwrap();
            assert!(has_admin, "is_admin column should exist after migration");
        }
    }

    #[tokio::test]
    async fn sqlite_is_configured_to_avoid_database_is_locked() {
        use std::time::Duration;

        let path = std::env::temp_dir().join(format!("rpodder-test-{}.db", uuid::Uuid::now_v7()));
        let url = format!("sqlite://{}", path.display());
        let db = Db::connect(&url).await.unwrap();

        let Db::Sqlite(pool) = &db else {
            panic!("expected sqlite pool");
        };

        // Pragmas that keep write transactions short and prevent immediate
        // SQLITE_BUSY on slow disks (issue #19).
        let mut c = pool.acquire().await.unwrap();
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert_eq!(journal_mode, "wal", "WAL lets readers run during a write");
        let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert_eq!(
            synchronous, 1,
            "NORMAL (1) avoids fsync-per-write stalls under WAL"
        );
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert_eq!(foreign_keys, 1, "foreign keys must be enforced");
        let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert!(
            busy_timeout > 0,
            "a busy_timeout must be set, got {busy_timeout}"
        );

        // SQLite has a single writer; the pool must serialize access through one
        // connection so contenders queue instead of erroring with "database is
        // locked". A second acquire while the first is held must not succeed.
        let second = tokio::time::timeout(Duration::from_millis(300), pool.acquire()).await;
        assert!(
            second.is_err(),
            "SQLite pool must allow only one connection so writes serialize"
        );

        drop(c);
        pool.close().await;
        let _ = std::fs::remove_file(&path);
    }
}
