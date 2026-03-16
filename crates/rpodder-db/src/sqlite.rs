use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use rpodder_core::error::AppError;
use rpodder_core::repo::{self, Result};
use rpodder_core::types::*;

#[derive(Clone)]
pub struct SqliteRepo {
    pub pool: SqlitePool,
}

impl SqliteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn uuid_str(u: &Uuid) -> String {
    u.to_string()
}

// ---------------------------------------------------------------------------
// UserRepo
// ---------------------------------------------------------------------------

impl repo::UserRepo for SqliteRepo {
    async fn create(&self, username: &str, password_hash: &str, email: Option<&str>) -> Result<User> {
        let id = Uuid::now_v7();
        let id_s = uuid_str(&id);
        let now = Utc::now();
        let now_s = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO users (id, username, password_hash, email, is_active, created_at)
             VALUES (?, ?, ?, ?, 1, ?)",
        )
        .bind(&id_s)
        .bind(username)
        .bind(password_hash)
        .bind(email)
        .bind(&now_s)
        .execute(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(dbe) if dbe.message().contains("UNIQUE") => {
                AppError::Conflict(format!("user '{username}' already exists"))
            }
            _ => AppError::Internal(e.to_string()),
        })?;

        Ok(User {
            id,
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            email: email.map(|e| e.to_string()),
            is_active: true,
            created_at: now,
        })
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let row: Option<SqliteUserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE username = ? COLLATE NOCASE",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row: Option<SqliteUserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE id = ?",
        )
        .bind(uuid_str(&id))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }
}

#[derive(sqlx::FromRow)]
struct SqliteUserRow {
    id: String,
    username: String,
    password_hash: String,
    email: Option<String>,
    is_active: bool,
    created_at: String,
}

impl From<SqliteUserRow> for User {
    fn from(r: SqliteUserRow) -> Self {
        User {
            id: r.id.parse().unwrap_or_default(),
            username: r.username,
            password_hash: r.password_hash,
            email: r.email,
            is_active: r.is_active,
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// DeviceRepo
// ---------------------------------------------------------------------------

fn parse_device_type(s: &str) -> DeviceType {
    match s {
        "desktop" => DeviceType::Desktop,
        "laptop" => DeviceType::Laptop,
        "mobile" => DeviceType::Mobile,
        "server" => DeviceType::Server,
        "tablet" => DeviceType::Tablet,
        _ => DeviceType::Other,
    }
}

fn device_type_str(dt: DeviceType) -> &'static str {
    match dt {
        DeviceType::Desktop => "desktop",
        DeviceType::Laptop => "laptop",
        DeviceType::Mobile => "mobile",
        DeviceType::Server => "server",
        DeviceType::Tablet => "tablet",
        DeviceType::Other => "other",
    }
}

#[derive(sqlx::FromRow)]
struct SqliteDeviceRow {
    id: String,
    user_id: String,
    device_id: String,
    caption: String,
    device_type: String,
    sync_group_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<SqliteDeviceRow> for Device {
    fn from(r: SqliteDeviceRow) -> Self {
        Device {
            id: r.id.parse().unwrap_or_default(),
            user_id: r.user_id.parse().unwrap_or_default(),
            device_id: r.device_id,
            caption: r.caption,
            device_type: parse_device_type(&r.device_type),
            sync_group_id: r.sync_group_id.and_then(|s| s.parse().ok()),
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl repo::DeviceRepo for SqliteRepo {
    async fn upsert(&self, device: &Device) -> Result<Device> {
        let id_s = uuid_str(&device.id);
        let user_id_s = uuid_str(&device.user_id);
        let dt_s = device_type_str(device.device_type);
        let now_s = device.updated_at.to_rfc3339();
        let created_s = device.created_at.to_rfc3339();

        // SQLite doesn't support RETURNING with ON CONFLICT in older versions,
        // so we do an upsert and then fetch.
        sqlx::query(
            "INSERT INTO devices (id, user_id, device_id, caption, device_type, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_id, device_id)
             DO UPDATE SET caption = excluded.caption, device_type = excluded.device_type, updated_at = excluded.updated_at",
        )
        .bind(&id_s)
        .bind(&user_id_s)
        .bind(&device.device_id)
        .bind(&device.caption)
        .bind(dt_s)
        .bind(&created_s)
        .bind(&now_s)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        // Fetch the row back (may have existing id if it was an update)
        let row: SqliteDeviceRow = sqlx::query_as(
            "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
             FROM devices WHERE user_id = ? AND device_id = ?",
        )
        .bind(&user_id_s)
        .bind(&device.device_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.into())
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Device>> {
        let rows: Vec<SqliteDeviceRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
             FROM devices WHERE user_id = ? ORDER BY created_at",
        )
        .bind(uuid_str(&user_id))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_uid(&self, user_id: Uuid, device_id: &str) -> Result<Option<Device>> {
        let row: Option<SqliteDeviceRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
             FROM devices WHERE user_id = ? AND device_id = ?",
        )
        .bind(uuid_str(&user_id))
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(Into::into))
    }
}

// ---------------------------------------------------------------------------
// PodcastRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SqlitePodcastRow {
    id: String,
    title: String,
    description: String,
    link: Option<String>,
    language: Option<String>,
    logo_url: Option<String>,
    author: Option<String>,
    subscribers: i64,
    episode_count: i64,
    last_update: Option<String>,
    update_interval_hours: i32,
    created_at: String,
    updated_at: String,
}

impl From<SqlitePodcastRow> for Podcast {
    fn from(r: SqlitePodcastRow) -> Self {
        Podcast {
            id: r.id.parse().unwrap_or_default(),
            title: r.title,
            description: r.description,
            link: r.link,
            language: r.language,
            logo_url: r.logo_url,
            author: r.author,
            subscribers: r.subscribers,
            episode_count: r.episode_count,
            last_update: r.last_update.and_then(|s| s.parse().ok()),
            update_interval_hours: r.update_interval_hours,
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl repo::PodcastRepo for SqliteRepo {
    async fn get_or_create_for_url(&self, url: &str) -> Result<(Podcast, bool)> {
        if let Some(podcast) = self.find_by_url(url).await? {
            return Ok((podcast, false));
        }

        let id = Uuid::now_v7();
        let url_id = Uuid::now_v7();
        let now = Utc::now();
        let now_s = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO podcasts (id, title, created_at, updated_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(uuid_str(&id))
        .bind(url)
        .bind(&now_s)
        .bind(&now_s)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO podcast_urls (id, podcast_id, url, \"order\")
             VALUES (?, ?, ?, 0)",
        )
        .bind(uuid_str(&url_id))
        .bind(uuid_str(&id))
        .bind(url)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let podcast = Podcast {
            id,
            title: url.to_string(),
            description: String::new(),
            link: None,
            language: None,
            logo_url: None,
            author: None,
            subscribers: 0,
            episode_count: 0,
            last_update: None,
            update_interval_hours: 168,
            created_at: now,
            updated_at: now,
        };

        Ok((podcast, true))
    }

    async fn find_by_url(&self, url: &str) -> Result<Option<Podcast>> {
        let row: Option<SqlitePodcastRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.created_at, p.updated_at
             FROM podcasts p
             JOIN podcast_urls pu ON pu.podcast_id = p.id
             WHERE pu.url = ?",
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(row.map(Into::into))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Podcast>> {
        let row: Option<SqlitePodcastRow> = sqlx::query_as(
            "SELECT id, title, description, link, language, logo_url, author,
                    subscribers, episode_count, last_update, update_interval_hours,
                    created_at, updated_at
             FROM podcasts WHERE id = ?",
        )
        .bind(uuid_str(&id))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(row.map(Into::into))
    }

    async fn update(&self, podcast: &Podcast) -> Result<()> {
        sqlx::query(
            "UPDATE podcasts SET title = ?, description = ?, link = ?, language = ?,
                    logo_url = ?, author = ?, subscribers = ?, episode_count = ?,
                    last_update = ?, update_interval_hours = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&podcast.title)
        .bind(&podcast.description)
        .bind(&podcast.link)
        .bind(&podcast.language)
        .bind(&podcast.logo_url)
        .bind(&podcast.author)
        .bind(podcast.subscribers)
        .bind(podcast.episode_count)
        .bind(podcast.last_update.map(|t| t.to_rfc3339()))
        .bind(podcast.update_interval_hours)
        .bind(podcast.updated_at.to_rfc3339())
        .bind(uuid_str(&podcast.id))
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn toplist(&self, count: i64, language: Option<&str>) -> Result<Vec<Podcast>> {
        let rows: Vec<SqlitePodcastRow> = if let Some(lang) = language {
            sqlx::query_as(
                "SELECT id, title, description, link, language, logo_url, author,
                        subscribers, episode_count, last_update, update_interval_hours,
                        created_at, updated_at
                 FROM podcasts WHERE language = ? ORDER BY subscribers DESC LIMIT ?",
            )
            .bind(lang)
            .bind(count)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
        } else {
            sqlx::query_as(
                "SELECT id, title, description, link, language, logo_url, author,
                        subscribers, episode_count, last_update, update_interval_hours,
                        created_at, updated_at
                 FROM podcasts ORDER BY subscribers DESC LIMIT ?",
            )
            .bind(count)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
        };
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn search(&self, query: &str, limit: i64) -> Result<Vec<Podcast>> {
        let rows: Vec<SqlitePodcastRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.created_at, p.updated_at
             FROM podcasts p
             JOIN podcasts_fts fts ON fts.rowid = (SELECT rowid FROM podcasts WHERE id = p.id)
             WHERE podcasts_fts MATCH ?
             ORDER BY p.subscribers DESC LIMIT ?",
        )
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ---------------------------------------------------------------------------
// SubscriptionRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SqliteSubscriptionRow {
    id: String,
    user_id: String,
    device_id: String,
    podcast_id: String,
    ref_url: String,
    created_at: String,
}

impl From<SqliteSubscriptionRow> for Subscription {
    fn from(r: SqliteSubscriptionRow) -> Self {
        Subscription {
            id: r.id.parse().unwrap_or_default(),
            user_id: r.user_id.parse().unwrap_or_default(),
            device_id: r.device_id.parse().unwrap_or_default(),
            podcast_id: r.podcast_id.parse().unwrap_or_default(),
            ref_url: r.ref_url,
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct SqliteSubChangeRow {
    id: String,
    user_id: String,
    device_id: String,
    podcast_id: String,
    action: String,
    ref_url: String,
    timestamp: String,
}

impl From<SqliteSubChangeRow> for SubscriptionChange {
    fn from(r: SqliteSubChangeRow) -> Self {
        SubscriptionChange {
            id: r.id.parse().unwrap_or_default(),
            user_id: r.user_id.parse().unwrap_or_default(),
            device_id: r.device_id.parse().unwrap_or_default(),
            podcast_id: r.podcast_id.parse().unwrap_or_default(),
            action: match r.action.as_str() {
                "subscribe" => SubscriptionAction::Subscribe,
                _ => SubscriptionAction::Unsubscribe,
            },
            ref_url: r.ref_url,
            timestamp: r.timestamp.parse().unwrap_or_default(),
        }
    }
}

impl repo::SubscriptionRepo for SqliteRepo {
    async fn subscribe(&self, user_id: Uuid, device_id: Uuid, podcast_id: Uuid, ref_url: &str) -> Result<()> {
        let sub_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO subscriptions (id, user_id, device_id, podcast_id, ref_url)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (user_id, device_id, podcast_id) DO NOTHING",
        )
        .bind(uuid_str(&sub_id))
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&device_id))
        .bind(uuid_str(&podcast_id))
        .bind(ref_url)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let change_id = Uuid::now_v7();
        let now_s = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO subscription_changes (id, user_id, device_id, podcast_id, action, ref_url, timestamp)
             VALUES (?, ?, ?, ?, 'subscribe', ?, ?)",
        )
        .bind(uuid_str(&change_id))
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&device_id))
        .bind(uuid_str(&podcast_id))
        .bind(ref_url)
        .bind(&now_s)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn unsubscribe(&self, user_id: Uuid, device_id: Uuid, podcast_id: Uuid) -> Result<()> {
        let row: Option<SqliteSubscriptionRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, podcast_id, ref_url, created_at
             FROM subscriptions WHERE user_id = ? AND device_id = ? AND podcast_id = ?",
        )
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&device_id))
        .bind(uuid_str(&podcast_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let ref_url = row.map(|r| r.ref_url).unwrap_or_default();

        sqlx::query(
            "DELETE FROM subscriptions WHERE user_id = ? AND device_id = ? AND podcast_id = ?",
        )
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&device_id))
        .bind(uuid_str(&podcast_id))
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let change_id = Uuid::now_v7();
        let now_s = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO subscription_changes (id, user_id, device_id, podcast_id, action, ref_url, timestamp)
             VALUES (?, ?, ?, ?, 'unsubscribe', ?, ?)",
        )
        .bind(uuid_str(&change_id))
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&device_id))
        .bind(uuid_str(&podcast_id))
        .bind(&ref_url)
        .bind(&now_s)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_for_device(&self, user_id: Uuid, device_id: Uuid) -> Result<Vec<Subscription>> {
        let rows: Vec<SqliteSubscriptionRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, podcast_id, ref_url, created_at
             FROM subscriptions WHERE user_id = ? AND device_id = ?",
        )
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&device_id))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Subscription>> {
        let rows: Vec<SqliteSubscriptionRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, podcast_id, ref_url, created_at
             FROM subscriptions WHERE user_id = ?
             GROUP BY podcast_id
             ORDER BY created_at",
        )
        .bind(uuid_str(&user_id))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn changes_since(&self, user_id: Uuid, device_id: Uuid, since: DateTime<Utc>) -> Result<Vec<SubscriptionChange>> {
        let rows: Vec<SqliteSubChangeRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, podcast_id, action, ref_url, timestamp
             FROM subscription_changes
             WHERE user_id = ? AND device_id = ? AND timestamp > ?
             ORDER BY timestamp",
        )
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&device_id))
        .bind(since.to_rfc3339())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ---------------------------------------------------------------------------
// SessionRepo
// ---------------------------------------------------------------------------

impl repo::SessionRepo for SqliteRepo {
    async fn create(&self, session: &Session) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(&session.id))
        .bind(uuid_str(&session.user_id))
        .bind(&session.token)
        .bind(session.expires_at.to_rfc3339())
        .bind(session.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn find_by_token(&self, token: &str) -> Result<Option<Session>> {
        let now = Utc::now().to_rfc3339();
        let row: Option<SqliteSessionRow> = sqlx::query_as(
            "SELECT id, user_id, token, expires_at, created_at
             FROM sessions WHERE token = ? AND expires_at > ?",
        )
        .bind(token)
        .bind(&now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    async fn delete(&self, token: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete_expired(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

#[derive(sqlx::FromRow)]
struct SqliteSessionRow {
    id: String,
    user_id: String,
    token: String,
    expires_at: String,
    created_at: String,
}

impl From<SqliteSessionRow> for Session {
    fn from(r: SqliteSessionRow) -> Self {
        Session {
            id: r.id.parse().unwrap_or_default(),
            user_id: r.user_id.parse().unwrap_or_default(),
            token: r.token,
            expires_at: r.expires_at.parse().unwrap_or_default(),
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rpodder_core::repo::{DeviceRepo, PodcastRepo, SessionRepo, SubscriptionRepo, UserRepo};

    async fn setup() -> SqliteRepo {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let schema = std::fs::read_to_string("../../migrations/sqlite/001_initial.up.sql").unwrap();
        sqlx::raw_sql(&schema).execute(&pool).await.unwrap();
        SqliteRepo::new(pool)
    }

    // === UserRepo tests ===

    #[tokio::test]
    async fn create_user_and_find_by_username() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "Alice", "hash123", Some("alice@example.com")).await.unwrap();

        assert_eq!(user.username, "Alice");
        assert_eq!(user.email.as_deref(), Some("alice@example.com"));
        assert!(user.is_active);

        let found = repo.find_by_username("Alice").await.unwrap().unwrap();
        assert_eq!(found.id, user.id);
        assert_eq!(found.password_hash, "hash123");
    }

    #[tokio::test]
    async fn find_by_username_case_insensitive() {
        let repo = setup().await;
        UserRepo::create(&repo, "Bob", "hash", None).await.unwrap();

        let found = repo.find_by_username("bob").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "Bob");

        let found = repo.find_by_username("BOB").await.unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn find_by_username_not_found() {
        let repo = setup().await;
        let found = repo.find_by_username("nonexistent").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn find_by_id() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "Charlie", "hash", None).await.unwrap();

        let found = UserRepo::find_by_id(&repo, user.id).await.unwrap().unwrap();
        assert_eq!(found.username, "Charlie");
    }

    #[tokio::test]
    async fn find_by_id_not_found() {
        let repo = setup().await;
        let found = UserRepo::find_by_id(&repo, Uuid::now_v7()).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn create_duplicate_username_fails() {
        let repo = setup().await;
        UserRepo::create(&repo, "Dave", "hash1", None).await.unwrap();

        let result = UserRepo::create(&repo, "Dave", "hash2", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::Conflict(msg) => assert!(msg.contains("Dave")),
            other => panic!("expected Conflict, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_user_without_email() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "NoEmail", "hash", None).await.unwrap();
        assert!(user.email.is_none());

        let found = repo.find_by_username("NoEmail").await.unwrap().unwrap();
        assert!(found.email.is_none());
    }

    // === SessionRepo tests ===

    #[tokio::test]
    async fn create_session_and_find_by_token() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "SessionUser", "hash", None).await.unwrap();

        let session = Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: "test-token-abc123".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        };
        SessionRepo::create(&repo, &session).await.unwrap();

        let found = repo.find_by_token("test-token-abc123").await.unwrap().unwrap();
        assert_eq!(found.user_id, user.id);
        assert_eq!(found.token, "test-token-abc123");
    }

    #[tokio::test]
    async fn find_by_token_not_found() {
        let repo = setup().await;
        let found = repo.find_by_token("nonexistent").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn find_by_token_expired_returns_none() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "ExpiredUser", "hash", None).await.unwrap();

        let session = Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: "expired-token".to_string(),
            expires_at: Utc::now() - Duration::hours(1), // already expired
            created_at: Utc::now() - Duration::hours(2),
        };
        SessionRepo::create(&repo, &session).await.unwrap();

        let found = repo.find_by_token("expired-token").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn delete_session() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "DeleteUser", "hash", None).await.unwrap();

        let session = Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: "delete-me".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        };
        SessionRepo::create(&repo, &session).await.unwrap();

        // Confirm it exists
        assert!(repo.find_by_token("delete-me").await.unwrap().is_some());

        // Delete it
        SessionRepo::delete(&repo, "delete-me").await.unwrap();

        // Confirm it's gone
        assert!(repo.find_by_token("delete-me").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_expired_sessions() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "CleanupUser", "hash", None).await.unwrap();

        // Create 2 expired sessions and 1 valid
        for (i, hours_offset) in [(-2i64), (-1), 1].iter().enumerate() {
            let session = Session {
                id: Uuid::now_v7(),
                user_id: user.id,
                token: format!("token-{i}"),
                expires_at: Utc::now() + Duration::hours(*hours_offset),
                created_at: Utc::now() - Duration::hours(3),
            };
            SessionRepo::create(&repo, &session).await.unwrap();
        }

        let deleted = repo.delete_expired().await.unwrap();
        assert_eq!(deleted, 2);

        // The valid one should still exist
        assert!(repo.find_by_token("token-2").await.unwrap().is_some());
        // The expired ones should be gone
        assert!(repo.find_by_token("token-0").await.unwrap().is_none());
        assert!(repo.find_by_token("token-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_session_is_ok() {
        let repo = setup().await;
        // Should not error
        SessionRepo::delete(&repo, "does-not-exist").await.unwrap();
    }

    #[tokio::test]
    async fn multiple_sessions_per_user() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "MultiSession", "hash", None).await.unwrap();

        for i in 0..3 {
            let session = Session {
                id: Uuid::now_v7(),
                user_id: user.id,
                token: format!("multi-token-{i}"),
                expires_at: Utc::now() + Duration::hours(1),
                created_at: Utc::now(),
            };
            SessionRepo::create(&repo, &session).await.unwrap();
        }

        // All three should be findable
        for i in 0..3 {
            let found = repo.find_by_token(&format!("multi-token-{i}")).await.unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().user_id, user.id);
        }
    }

    // === DeviceRepo tests ===

    #[tokio::test]
    async fn create_device_and_find_by_uid() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "DevUser", "hash", None).await.unwrap();

        let now = Utc::now();
        let device = Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "my-phone".to_string(),
            caption: "My Phone".to_string(),
            device_type: DeviceType::Mobile,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        };
        let created = DeviceRepo::upsert(&repo, &device).await.unwrap();
        assert_eq!(created.device_id, "my-phone");
        assert_eq!(created.caption, "My Phone");
        assert_eq!(created.device_type, DeviceType::Mobile);

        let found = DeviceRepo::find_by_uid(&repo, user.id, "my-phone").await.unwrap().unwrap();
        assert_eq!(found.device_id, "my-phone");
        assert_eq!(found.caption, "My Phone");
        assert_eq!(found.device_type, DeviceType::Mobile);
    }

    #[tokio::test]
    async fn upsert_device_updates_existing() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "UpsertUser", "hash", None).await.unwrap();

        let now = Utc::now();
        let device = Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "laptop1".to_string(),
            caption: "Old Caption".to_string(),
            device_type: DeviceType::Laptop,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        };
        DeviceRepo::upsert(&repo, &device).await.unwrap();

        // Upsert with new caption and type
        let updated_device = Device {
            id: Uuid::now_v7(), // different id, but same user_id + device_id
            user_id: user.id,
            device_id: "laptop1".to_string(),
            caption: "New Caption".to_string(),
            device_type: DeviceType::Desktop,
            sync_group_id: None,
            created_at: now,
            updated_at: Utc::now(),
        };
        let result = DeviceRepo::upsert(&repo, &updated_device).await.unwrap();

        assert_eq!(result.caption, "New Caption");
        assert_eq!(result.device_type, DeviceType::Desktop);
        // The original id should be preserved (not the new one)
        assert_eq!(result.device_id, "laptop1");
    }

    #[tokio::test]
    async fn list_devices_for_user() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "ListUser", "hash", None).await.unwrap();
        let other = UserRepo::create(&repo, "OtherUser", "hash", None).await.unwrap();

        let now = Utc::now();
        for (uid, did, user_id) in [
            ("phone", "Phone", user.id),
            ("laptop", "Laptop", user.id),
            ("other-dev", "Other Device", other.id),
        ] {
            let device = Device {
                id: Uuid::now_v7(),
                user_id,
                device_id: uid.to_string(),
                caption: did.to_string(),
                device_type: DeviceType::Other,
                sync_group_id: None,
                created_at: now,
                updated_at: now,
            };
            DeviceRepo::upsert(&repo, &device).await.unwrap();
        }

        let devices = DeviceRepo::list_for_user(&repo, user.id).await.unwrap();
        assert_eq!(devices.len(), 2);

        let other_devices = DeviceRepo::list_for_user(&repo, other.id).await.unwrap();
        assert_eq!(other_devices.len(), 1);
    }

    #[tokio::test]
    async fn find_device_not_found() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "NoDevUser", "hash", None).await.unwrap();
        let found = DeviceRepo::find_by_uid(&repo, user.id, "nonexistent").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn list_devices_empty() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "EmptyUser", "hash", None).await.unwrap();
        let devices = DeviceRepo::list_for_user(&repo, user.id).await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn device_type_roundtrip() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "TypeUser", "hash", None).await.unwrap();

        let now = Utc::now();
        for dt in [
            DeviceType::Desktop,
            DeviceType::Laptop,
            DeviceType::Mobile,
            DeviceType::Server,
            DeviceType::Tablet,
            DeviceType::Other,
        ] {
            let did = format!("dev-{:?}", dt).to_lowercase();
            let device = Device {
                id: Uuid::now_v7(),
                user_id: user.id,
                device_id: did.clone(),
                caption: format!("{:?} device", dt),
                device_type: dt,
                sync_group_id: None,
                created_at: now,
                updated_at: now,
            };
            DeviceRepo::upsert(&repo, &device).await.unwrap();

            let found = DeviceRepo::find_by_uid(&repo, user.id, &did).await.unwrap().unwrap();
            assert_eq!(found.device_type, dt, "roundtrip failed for {:?}", dt);
        }
    }

    // === PodcastRepo tests ===

    #[tokio::test]
    async fn get_or_create_podcast_for_url() {
        let repo = setup().await;
        let (podcast, created) = PodcastRepo::get_or_create_for_url(&repo, "http://example.com/feed.xml").await.unwrap();
        assert!(created);
        assert_eq!(podcast.title, "http://example.com/feed.xml");

        // Second call should return existing
        let (podcast2, created2) = PodcastRepo::get_or_create_for_url(&repo, "http://example.com/feed.xml").await.unwrap();
        assert!(!created2);
        assert_eq!(podcast2.id, podcast.id);
    }

    #[tokio::test]
    async fn find_podcast_by_url() {
        let repo = setup().await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/rss").await.unwrap();

        let found = PodcastRepo::find_by_url(&repo, "http://test.com/rss").await.unwrap().unwrap();
        assert_eq!(found.id, podcast.id);

        let not_found = PodcastRepo::find_by_url(&repo, "http://other.com/rss").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn find_podcast_by_id() {
        let repo = setup().await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed").await.unwrap();

        let found = PodcastRepo::find_by_id(&repo, podcast.id).await.unwrap().unwrap();
        assert_eq!(found.title, "http://test.com/feed");

        let not_found = PodcastRepo::find_by_id(&repo, Uuid::now_v7()).await.unwrap();
        assert!(not_found.is_none());
    }

    // === SubscriptionRepo tests ===

    /// Helper: create user + device + podcast for subscription tests
    async fn setup_subscription_fixtures(repo: &SqliteRepo) -> (User, Device, Podcast) {
        let user = UserRepo::create(repo, "SubUser", "hash", None).await.unwrap();
        let now = Utc::now();
        let device = Device {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: "phone".to_string(),
            caption: "Phone".to_string(),
            device_type: DeviceType::Mobile,
            sync_group_id: None,
            created_at: now,
            updated_at: now,
        };
        let device = DeviceRepo::upsert(repo, &device).await.unwrap();
        let (podcast, _) = PodcastRepo::get_or_create_for_url(repo, "http://example.com/feed.xml").await.unwrap();
        (user, device, podcast)
    }

    #[tokio::test]
    async fn subscribe_and_list() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(&repo, user.id, device.id, podcast.id, "http://example.com/feed.xml").await.unwrap();

        let subs = SubscriptionRepo::list_for_device(&repo, user.id, device.id).await.unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].ref_url, "http://example.com/feed.xml");
    }

    #[tokio::test]
    async fn subscribe_idempotent() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(&repo, user.id, device.id, podcast.id, "http://example.com/feed.xml").await.unwrap();
        SubscriptionRepo::subscribe(&repo, user.id, device.id, podcast.id, "http://example.com/feed.xml").await.unwrap();

        let subs = SubscriptionRepo::list_for_device(&repo, user.id, device.id).await.unwrap();
        assert_eq!(subs.len(), 1);
    }

    #[tokio::test]
    async fn unsubscribe() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(&repo, user.id, device.id, podcast.id, "http://example.com/feed.xml").await.unwrap();
        SubscriptionRepo::unsubscribe(&repo, user.id, device.id, podcast.id).await.unwrap();

        let subs = SubscriptionRepo::list_for_device(&repo, user.id, device.id).await.unwrap();
        assert!(subs.is_empty());
    }

    #[tokio::test]
    async fn list_subscriptions_for_user() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "MultiSubUser", "hash", None).await.unwrap();
        let now = Utc::now();

        // Two devices
        let dev1 = DeviceRepo::upsert(&repo, &Device {
            id: Uuid::now_v7(), user_id: user.id, device_id: "dev1".into(),
            caption: "".into(), device_type: DeviceType::Other,
            sync_group_id: None, created_at: now, updated_at: now,
        }).await.unwrap();
        let dev2 = DeviceRepo::upsert(&repo, &Device {
            id: Uuid::now_v7(), user_id: user.id, device_id: "dev2".into(),
            caption: "".into(), device_type: DeviceType::Other,
            sync_group_id: None, created_at: now, updated_at: now,
        }).await.unwrap();

        // Same podcast on both devices
        let (p1, _) = PodcastRepo::get_or_create_for_url(&repo, "http://feed1.com").await.unwrap();
        let (p2, _) = PodcastRepo::get_or_create_for_url(&repo, "http://feed2.com").await.unwrap();

        SubscriptionRepo::subscribe(&repo, user.id, dev1.id, p1.id, "http://feed1.com").await.unwrap();
        SubscriptionRepo::subscribe(&repo, user.id, dev2.id, p1.id, "http://feed1.com").await.unwrap();
        SubscriptionRepo::subscribe(&repo, user.id, dev1.id, p2.id, "http://feed2.com").await.unwrap();

        // list_for_user should deduplicate by podcast
        let subs = SubscriptionRepo::list_for_user(&repo, user.id).await.unwrap();
        assert_eq!(subs.len(), 2);
    }

    #[tokio::test]
    async fn changes_since_tracks_history() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        let before = Utc::now() - Duration::seconds(1);

        SubscriptionRepo::subscribe(&repo, user.id, device.id, podcast.id, "http://example.com/feed.xml").await.unwrap();
        SubscriptionRepo::unsubscribe(&repo, user.id, device.id, podcast.id).await.unwrap();

        let changes = SubscriptionRepo::changes_since(&repo, user.id, device.id, before).await.unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].action, SubscriptionAction::Subscribe);
        assert_eq!(changes[1].action, SubscriptionAction::Unsubscribe);
    }

    #[tokio::test]
    async fn changes_since_filters_by_time() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(&repo, user.id, device.id, podcast.id, "http://example.com/feed.xml").await.unwrap();

        let after = Utc::now() + Duration::seconds(1);

        let changes = SubscriptionRepo::changes_since(&repo, user.id, device.id, after).await.unwrap();
        assert!(changes.is_empty());
    }
}
