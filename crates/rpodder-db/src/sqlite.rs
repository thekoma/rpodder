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
    async fn create(
        &self,
        username: &str,
        password_hash: &str,
        email: Option<&str>,
    ) -> Result<User> {
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
            is_admin: false,
            created_at: now,
        })
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let row: Option<SqliteUserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, is_admin, created_at
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
            "SELECT id, username, password_hash, email, is_active, is_admin, created_at
             FROM users WHERE id = ?",
        )
        .bind(uuid_str(&id))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    async fn list_all(&self) -> Result<Vec<User>> {
        let rows: Vec<SqliteUserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, is_admin, created_at
             FROM users ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn set_admin(&self, user_id: Uuid, is_admin: bool) -> Result<()> {
        sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
            .bind(is_admin)
            .bind(uuid_str(&user_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn set_active(&self, user_id: Uuid, is_active: bool) -> Result<()> {
        sqlx::query("UPDATE users SET is_active = ? WHERE id = ?")
            .bind(is_active)
            .bind(uuid_str(&user_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, user_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(uuid_str(&user_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn count_active(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_active = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(row.0)
    }

    async fn update_password(&self, user_id: Uuid, password_hash: &str) -> Result<()> {
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(password_hash)
            .bind(uuid_str(&user_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let row: Option<SqliteUserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, is_admin, created_at
             FROM users WHERE email = ? COLLATE NOCASE",
        )
        .bind(email)
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
    is_admin: bool,
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
            is_admin: r.is_admin,
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
    content_hash: i64,
    etag: Option<String>,
    http_last_modified: Option<String>,
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
            content_hash: r.content_hash,
            etag: r.etag,
            http_last_modified: r.http_last_modified,
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl repo::PodcastRepo for SqliteRepo {
    async fn add_url(&self, podcast_id: Uuid, url: &str) -> Result<()> {
        let max_order: Option<(i32,)> = sqlx::query_as(
            "SELECT COALESCE(MAX(\"order\"), -1) FROM podcast_urls WHERE podcast_id = ?",
        )
        .bind(uuid_str(&podcast_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        let next_order = max_order.map(|r| r.0 + 1).unwrap_or(0);

        sqlx::query(
            "INSERT OR IGNORE INTO podcast_urls (id, podcast_id, url, \"order\") VALUES (?, ?, ?, ?)",
        )
        .bind(uuid_str(&Uuid::now_v7()))
        .bind(uuid_str(&podcast_id))
        .bind(url)
        .bind(next_order)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

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
            content_hash: 0,
            etag: None,
            http_last_modified: None,
            created_at: now,
            updated_at: now,
        };

        Ok((podcast, true))
    }

    async fn find_by_url(&self, url: &str) -> Result<Option<Podcast>> {
        let row: Option<SqlitePodcastRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.content_hash, p.etag, p.http_last_modified, p.created_at, p.updated_at
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
                    content_hash, etag, http_last_modified, created_at, updated_at
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
                    last_update = ?, update_interval_hours = ?,
                    content_hash = ?, etag = ?, http_last_modified = ?, updated_at = ?
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
        .bind(podcast.content_hash)
        .bind(&podcast.etag)
        .bind(&podcast.http_last_modified)
        .bind(podcast.updated_at.to_rfc3339())
        .bind(uuid_str(&podcast.id))
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM podcast_urls WHERE podcast_id = ?")
            .bind(uuid_str(&id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sqlx::query("DELETE FROM podcasts WHERE id = ?")
            .bind(uuid_str(&id))
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
                        content_hash, etag, http_last_modified, created_at, updated_at
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
                        content_hash, etag, http_last_modified, created_at, updated_at
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
        // Add prefix matching: "wil" -> "wil*" so partial words match
        let fts_query: String = query
            .split_whitespace()
            .map(|word| {
                if word.ends_with('*') {
                    word.to_string()
                } else {
                    format!("{word}*")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        // Try FTS5 first
        let rows: Vec<SqlitePodcastRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.content_hash, p.etag, p.http_last_modified, p.created_at, p.updated_at
             FROM podcasts p
             JOIN podcasts_fts fts ON fts.rowid = (SELECT rowid FROM podcasts WHERE id = p.id)
             WHERE podcasts_fts MATCH ?
             ORDER BY p.subscribers DESC LIMIT ?",
        )
        .bind(&fts_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        if !rows.is_empty() {
            return Ok(rows.into_iter().map(Into::into).collect());
        }

        // Fallback: LIKE search for when FTS5 doesn't match
        let like_pattern = format!("%{query}%");
        let rows: Vec<SqlitePodcastRow> = sqlx::query_as(
            "SELECT id, title, description, link, language, logo_url, author,
                    subscribers, episode_count, last_update, update_interval_hours,
                    content_hash, etag, http_last_modified, created_at, updated_at
             FROM podcasts
             WHERE title LIKE ? COLLATE NOCASE
                OR author LIKE ? COLLATE NOCASE
                OR description LIKE ? COLLATE NOCASE
             ORDER BY subscribers DESC LIMIT ?",
        )
        .bind(&like_pattern)
        .bind(&like_pattern)
        .bind(&like_pattern)
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
    async fn subscribe(
        &self,
        user_id: Uuid,
        device_id: Uuid,
        podcast_id: Uuid,
        ref_url: &str,
    ) -> Result<()> {
        let sub_id = Uuid::now_v7();
        let result = sqlx::query(
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

        // Only log change and update count if a row was actually inserted
        if result.rows_affected() > 0 {
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

            let _ = sqlx::query(
                "UPDATE podcasts SET subscribers = (
                    SELECT COUNT(DISTINCT user_id) FROM subscriptions WHERE podcast_id = ?
                 ) WHERE id = ?",
            )
            .bind(uuid_str(&podcast_id))
            .bind(uuid_str(&podcast_id))
            .execute(&self.pool)
            .await;
        }

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

        // Update subscriber count
        let _ = sqlx::query(
            "UPDATE podcasts SET subscribers = (
                SELECT COUNT(DISTINCT user_id) FROM subscriptions WHERE podcast_id = ?
             ) WHERE id = ?",
        )
        .bind(uuid_str(&podcast_id))
        .bind(uuid_str(&podcast_id))
        .execute(&self.pool)
        .await;

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

    async fn changes_since(
        &self,
        user_id: Uuid,
        device_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Vec<SubscriptionChange>> {
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

    async fn migrate_podcast(&self, from_podcast_id: Uuid, to_podcast_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE subscriptions SET podcast_id = ? WHERE podcast_id = ?")
            .bind(uuid_str(&to_podcast_id))
            .bind(uuid_str(&from_podcast_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        sqlx::query("UPDATE subscription_changes SET podcast_id = ? WHERE podcast_id = ?")
            .bind(uuid_str(&to_podcast_id))
            .bind(uuid_str(&from_podcast_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EpisodeRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SqliteEpisodeRow {
    id: String,
    podcast_id: String,
    guid: Option<String>,
    title: String,
    description: String,
    link: Option<String>,
    released: Option<String>,
    duration: Option<i64>,
    filesize: Option<i64>,
    mimetype: Option<String>,
    content_hash: i64,
    created_at: String,
    updated_at: String,
}

impl From<SqliteEpisodeRow> for Episode {
    fn from(r: SqliteEpisodeRow) -> Self {
        Episode {
            id: r.id.parse().unwrap_or_default(),
            podcast_id: r.podcast_id.parse().unwrap_or_default(),
            guid: r.guid,
            title: r.title,
            description: r.description,
            link: r.link,
            released: r.released.and_then(|s| s.parse().ok()),
            duration: r.duration,
            filesize: r.filesize,
            mimetype: r.mimetype,
            content_hash: r.content_hash,
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl repo::EpisodeRepo for SqliteRepo {
    async fn find_podcast_id_by_episode_url(&self, url: &str) -> Result<Option<Uuid>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT e.podcast_id FROM episodes e JOIN episode_urls eu ON eu.episode_id = e.id WHERE eu.url = ? LIMIT 1",
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(row.and_then(|r| r.0.parse().ok()))
    }

    async fn get_or_create_for_url(&self, podcast_id: Uuid, url: &str) -> Result<(Episode, bool)> {
        let existing: Option<SqliteEpisodeRow> = sqlx::query_as(
            "SELECT e.id, e.podcast_id, e.guid, e.title, e.description, e.link,
                    e.released, e.duration, e.filesize, e.mimetype, e.content_hash,
                    e.created_at, e.updated_at
             FROM episodes e
             JOIN episode_urls eu ON eu.episode_id = e.id
             WHERE eu.url = ?",
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        if let Some(row) = existing {
            return Ok((row.into(), false));
        }

        let id = Uuid::now_v7();
        let url_id = Uuid::now_v7();
        let now = Utc::now();
        let now_s = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO episodes (id, podcast_id, title, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(&id))
        .bind(uuid_str(&podcast_id))
        .bind(url)
        .bind(&now_s)
        .bind(&now_s)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO episode_urls (id, episode_id, url, \"order\")
             VALUES (?, ?, ?, 0)",
        )
        .bind(uuid_str(&url_id))
        .bind(uuid_str(&id))
        .bind(url)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let episode = Episode {
            id,
            podcast_id,
            guid: None,
            title: url.to_string(),
            description: String::new(),
            link: None,
            released: None,
            duration: None,
            filesize: None,
            mimetype: None,
            content_hash: 0,
            created_at: now,
            updated_at: now,
        };

        Ok((episode, true))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Episode>> {
        let row: Option<SqliteEpisodeRow> = sqlx::query_as(
            "SELECT id, podcast_id, guid, title, description, link,
                    released, duration, filesize, mimetype, content_hash, created_at, updated_at
             FROM episodes WHERE id = ?",
        )
        .bind(uuid_str(&id))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(row.map(Into::into))
    }

    async fn list_for_podcast(
        &self,
        podcast_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Episode>> {
        let rows: Vec<SqliteEpisodeRow> = sqlx::query_as(
            "SELECT id, podcast_id, guid, title, description, link,
                    released, duration, filesize, mimetype, content_hash, created_at, updated_at
             FROM episodes WHERE podcast_id = ?
             ORDER BY released DESC
             LIMIT ? OFFSET ?",
        )
        .bind(uuid_str(&podcast_id))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update(&self, episode: &Episode) -> Result<()> {
        sqlx::query(
            "UPDATE episodes SET guid = ?, title = ?, description = ?, link = ?,
                    released = ?, duration = ?, filesize = ?, mimetype = ?,
                    content_hash = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&episode.guid)
        .bind(&episode.title)
        .bind(&episode.description)
        .bind(&episode.link)
        .bind(episode.released.map(|t| t.to_rfc3339()))
        .bind(episode.duration)
        .bind(episode.filesize)
        .bind(&episode.mimetype)
        .bind(episode.content_hash)
        .bind(episode.updated_at.to_rfc3339())
        .bind(uuid_str(&episode.id))
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EpisodeActionRepo
// ---------------------------------------------------------------------------

fn parse_action_type(s: &str) -> EpisodeActionType {
    match s {
        "download" => EpisodeActionType::Download,
        "play" => EpisodeActionType::Play,
        "delete" => EpisodeActionType::Delete,
        _ => EpisodeActionType::New,
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

#[derive(sqlx::FromRow)]
struct SqliteEpActionRow {
    id: String,
    user_id: String,
    device_id: Option<String>,
    episode_id: String,
    action: String,
    podcast_ref_url: Option<String>,
    episode_ref_url: Option<String>,
    started: Option<i32>,
    position: Option<i32>,
    total: Option<i32>,
    timestamp: String,
    created_at: String,
}

impl From<SqliteEpActionRow> for EpisodeAction {
    fn from(r: SqliteEpActionRow) -> Self {
        EpisodeAction {
            id: r.id.parse().unwrap_or_default(),
            user_id: r.user_id.parse().unwrap_or_default(),
            device_id: r.device_id.and_then(|s| s.parse().ok()),
            episode_id: r.episode_id.parse().unwrap_or_default(),
            action: parse_action_type(&r.action),
            podcast_ref_url: r.podcast_ref_url,
            episode_ref_url: r.episode_ref_url,
            started: r.started,
            position: r.position,
            total: r.total,
            timestamp: r.timestamp.parse().unwrap_or_default(),
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}

impl repo::EpisodeActionRepo for SqliteRepo {
    async fn create(&self, action: &EpisodeAction) -> Result<()> {
        sqlx::query(
            "INSERT INTO episode_actions
                (id, user_id, device_id, episode_id, action, podcast_ref_url, episode_ref_url,
                 started, position, total, timestamp, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_id, episode_id, COALESCE(device_id, ''), action, timestamp)
             DO UPDATE SET position = excluded.position, started = excluded.started, total = excluded.total",
        )
        .bind(uuid_str(&action.id))
        .bind(uuid_str(&action.user_id))
        .bind(action.device_id.map(|d| uuid_str(&d)))
        .bind(uuid_str(&action.episode_id))
        .bind(action_type_str(action.action))
        .bind(&action.podcast_ref_url)
        .bind(&action.episode_ref_url)
        .bind(action.started)
        .bind(action.position)
        .bind(action.total)
        .bind(action.timestamp.to_rfc3339())
        .bind(action.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn list(
        &self,
        user_id: Uuid,
        device_id: Option<Uuid>,
        podcast_id: Option<Uuid>,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<EpisodeAction>> {
        // SQLite doesn't support numbered params, so we build a query with ?
        // and bind in order. We use a simpler approach: always filter, use impossible defaults.
        let user_id_s = uuid_str(&user_id);

        let mut conditions = vec!["ea.user_id = ?".to_string()];
        let mut bind_values: Vec<String> = vec![user_id_s];

        if let Some(did) = device_id {
            conditions.push("ea.device_id = ?".to_string());
            bind_values.push(uuid_str(&did));
        }
        if let Some(pid) = podcast_id {
            conditions.push(
                "ea.episode_id IN (SELECT id FROM episodes WHERE podcast_id = ?)".to_string(),
            );
            bind_values.push(uuid_str(&pid));
        }
        if let Some(s) = since {
            conditions.push("ea.timestamp > ?".to_string());
            bind_values.push(s.to_rfc3339());
        }

        let sql = format!(
            "SELECT ea.id, ea.user_id, ea.device_id, ea.episode_id, ea.action,
                    ea.podcast_ref_url, ea.episode_ref_url,
                    ea.started, ea.position, ea.total, ea.timestamp, ea.created_at
             FROM episode_actions ea
             WHERE {}
             ORDER BY ea.timestamp
             LIMIT ?",
            conditions.join(" AND ")
        );

        let mut q = sqlx::query_as::<_, SqliteEpActionRow>(sqlx::AssertSqlSafe(sql));
        for v in &bind_values {
            q = q.bind(v);
        }
        q = q.bind(limit);

        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ---------------------------------------------------------------------------
// TagRepo
// ---------------------------------------------------------------------------

impl repo::TagRepo for SqliteRepo {
    async fn top_tags(&self, count: i64) -> Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT tag, COUNT(DISTINCT podcast_id) as cnt
             FROM tags
             GROUP BY tag
             ORDER BY cnt DESC
             LIMIT ?",
        )
        .bind(count)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows)
    }

    async fn podcasts_for_tag(&self, tag: &str, count: i64) -> Result<Vec<Podcast>> {
        let rows: Vec<SqlitePodcastRow> = sqlx::query_as(
            "SELECT DISTINCT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.content_hash, p.etag, p.http_last_modified, p.created_at, p.updated_at
             FROM podcasts p
             JOIN tags t ON t.podcast_id = p.id
             WHERE t.tag = ? COLLATE NOCASE
             ORDER BY p.subscribers DESC
             LIMIT ?",
        )
        .bind(tag)
        .bind(count)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn set_tags_for_podcast(&self, podcast_id: Uuid, tags: &[Tag]) -> Result<()> {
        sqlx::query("DELETE FROM tags WHERE podcast_id = ? AND source = 'feed'")
            .bind(uuid_str(&podcast_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        for tag in tags {
            sqlx::query(
                "INSERT INTO tags (id, tag, source, user_id, podcast_id)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (tag, source, user_id, podcast_id) DO NOTHING",
            )
            .bind(uuid_str(&tag.id))
            .bind(&tag.tag)
            .bind(match tag.source {
                TagSource::Feed => "feed",
                TagSource::User => "user",
            })
            .bind(tag.user_id.map(|u| uuid_str(&u)))
            .bind(uuid_str(&tag.podcast_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SyncGroupRepo
// ---------------------------------------------------------------------------

impl repo::SyncGroupRepo for SqliteRepo {
    async fn create_group(
        &self,
        user_id: Uuid,
        device_ids: &[Uuid],
        name: &str,
    ) -> Result<SyncGroup> {
        let id = Uuid::now_v7();
        let now = Utc::now();

        sqlx::query("INSERT INTO sync_groups (id, user_id, name, created_at) VALUES (?, ?, ?, ?)")
            .bind(uuid_str(&id))
            .bind(uuid_str(&user_id))
            .bind(name)
            .bind(now.to_rfc3339())
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        for did in device_ids {
            sqlx::query("UPDATE devices SET sync_group_id = ? WHERE id = ? AND user_id = ?")
                .bind(uuid_str(&id))
                .bind(uuid_str(did))
                .bind(uuid_str(&user_id))
                .execute(&self.pool)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }

        Ok(SyncGroup {
            id,
            user_id,
            name: name.to_string(),
            created_at: now,
        })
    }

    async fn rename_group(&self, group_id: Uuid, user_id: Uuid, name: &str) -> Result<()> {
        sqlx::query("UPDATE sync_groups SET name = ? WHERE id = ? AND user_id = ?")
            .bind(name)
            .bind(uuid_str(&group_id))
            .bind(uuid_str(&user_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn get_groups_for_user(&self, user_id: Uuid) -> Result<Vec<(SyncGroup, Vec<Device>)>> {
        let groups: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT id, user_id, name, created_at FROM sync_groups WHERE user_id = ?",
        )
        .bind(uuid_str(&user_id))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let mut result = Vec::new();
        for (id_s, uid_s, name, created_s) in groups {
            let devices: Vec<SqliteDeviceRow> = sqlx::query_as(
                "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
                 FROM devices WHERE sync_group_id = ?",
            )
            .bind(&id_s)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

            let group = SyncGroup {
                id: id_s.parse().unwrap_or_default(),
                user_id: uid_s.parse().unwrap_or_default(),
                name,
                created_at: created_s.parse().unwrap_or_default(),
            };
            result.push((group, devices.into_iter().map(Into::into).collect()));
        }
        Ok(result)
    }

    async fn remove_device_from_group(&self, device_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE devices SET sync_group_id = NULL WHERE id = ?")
            .bind(uuid_str(&device_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SettingsRepo
// ---------------------------------------------------------------------------

fn scope_str(s: SettingsScope) -> &'static str {
    match s {
        SettingsScope::Account => "account",
        SettingsScope::Device => "device",
        SettingsScope::Podcast => "podcast",
        SettingsScope::Episode => "episode",
    }
}

fn parse_scope(s: &str) -> SettingsScope {
    match s {
        "device" => SettingsScope::Device,
        "podcast" => SettingsScope::Podcast,
        "episode" => SettingsScope::Episode,
        _ => SettingsScope::Account,
    }
}

impl repo::SettingsRepo for SqliteRepo {
    async fn get(
        &self,
        user_id: Uuid,
        scope: SettingsScope,
        scope_id: Option<Uuid>,
    ) -> Result<Option<UserSettings>> {
        let row: Option<(String, String, String, Option<String>, String, String)> = sqlx::query_as(
            "SELECT id, user_id, scope, scope_id, settings, updated_at
             FROM user_settings
             WHERE user_id = ? AND scope = ? AND COALESCE(scope_id, '') = COALESCE(?, '')",
        )
        .bind(uuid_str(&user_id))
        .bind(scope_str(scope))
        .bind(scope_id.map(|s| uuid_str(&s)))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(row.map(
            |(id, uid, scope_s, sid, settings_s, updated)| UserSettings {
                id: id.parse().unwrap_or_default(),
                user_id: uid.parse().unwrap_or_default(),
                scope: parse_scope(&scope_s),
                scope_id: sid.and_then(|s| s.parse().ok()),
                settings: serde_json::from_str(&settings_s).unwrap_or_default(),
                updated_at: updated.parse().unwrap_or_default(),
            },
        ))
    }

    async fn save(&self, settings: &UserSettings) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_settings (id, user_id, scope, scope_id, settings, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_id, scope, COALESCE(scope_id, ''))
             DO UPDATE SET settings = excluded.settings, updated_at = excluded.updated_at",
        )
        .bind(uuid_str(&settings.id))
        .bind(uuid_str(&settings.user_id))
        .bind(scope_str(settings.scope))
        .bind(settings.scope_id.map(|s| uuid_str(&s)))
        .bind(settings.settings.to_string())
        .bind(settings.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PodcastListRepo
// ---------------------------------------------------------------------------

impl repo::PodcastListRepo for SqliteRepo {
    async fn create(&self, list: &PodcastList) -> Result<()> {
        sqlx::query(
            "INSERT INTO podcast_lists (id, user_id, title, slug, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid_str(&list.id))
        .bind(uuid_str(&list.user_id))
        .bind(&list.title)
        .bind(&list.slug)
        .bind(list.created_at.to_rfc3339())
        .bind(list.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn find_by_slug(&self, user_id: Uuid, slug: &str) -> Result<Option<PodcastList>> {
        let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, user_id, title, slug, created_at, updated_at
             FROM podcast_lists WHERE user_id = ? AND slug = ?",
        )
        .bind(uuid_str(&user_id))
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(
            row.map(|(id, uid, title, slug, created, updated)| PodcastList {
                id: id.parse().unwrap_or_default(),
                user_id: uid.parse().unwrap_or_default(),
                title,
                slug,
                created_at: created.parse().unwrap_or_default(),
                updated_at: updated.parse().unwrap_or_default(),
            }),
        )
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<PodcastList>> {
        let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, user_id, title, slug, created_at, updated_at
             FROM podcast_lists WHERE user_id = ? ORDER BY created_at",
        )
        .bind(uuid_str(&user_id))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|(id, uid, title, slug, created, updated)| PodcastList {
                id: id.parse().unwrap_or_default(),
                user_id: uid.parse().unwrap_or_default(),
                title,
                slug,
                created_at: created.parse().unwrap_or_default(),
                updated_at: updated.parse().unwrap_or_default(),
            })
            .collect())
    }

    async fn set_entries(&self, list_id: Uuid, podcast_ids: &[Uuid]) -> Result<()> {
        sqlx::query("DELETE FROM podcast_list_entries WHERE list_id = ?")
            .bind(uuid_str(&list_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        for (i, pid) in podcast_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO podcast_list_entries (id, list_id, podcast_id, \"order\") VALUES (?, ?, ?, ?)",
            )
            .bind(uuid_str(&Uuid::now_v7()))
            .bind(uuid_str(&list_id))
            .bind(uuid_str(pid))
            .bind(i as i32)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        }
        Ok(())
    }

    async fn get_entries(&self, list_id: Uuid) -> Result<Vec<Podcast>> {
        let rows: Vec<SqlitePodcastRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.content_hash, p.etag, p.http_last_modified, p.created_at, p.updated_at
             FROM podcasts p
             JOIN podcast_list_entries ple ON ple.podcast_id = p.id
             WHERE ple.list_id = ?
             ORDER BY ple.\"order\"",
        )
        .bind(uuid_str(&list_id))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn delete(&self, list_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM podcast_lists WHERE id = ?")
            .bind(uuid_str(&list_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ChapterRepo
// ---------------------------------------------------------------------------

impl repo::ChapterRepo for SqliteRepo {
    async fn upsert(&self, chapter: &Chapter) -> Result<()> {
        sqlx::query(
            "INSERT INTO chapters (id, user_id, episode_id, start_sec, end_sec, label, advertisement, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET label = excluded.label, advertisement = excluded.advertisement",
        )
        .bind(uuid_str(&chapter.id))
        .bind(uuid_str(&chapter.user_id))
        .bind(uuid_str(&chapter.episode_id))
        .bind(chapter.start_sec)
        .bind(chapter.end_sec)
        .bind(&chapter.label)
        .bind(chapter.advertisement)
        .bind(chapter.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn list_for_episode(
        &self,
        user_id: Uuid,
        episode_id: Uuid,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Chapter>> {
        let rows: Vec<(String, String, String, i32, i32, String, bool, String)> = if let Some(
            since,
        ) = since
        {
            sqlx::query_as(
                "SELECT id, user_id, episode_id, start_sec, end_sec, label, advertisement, created_at
                 FROM chapters WHERE user_id = ? AND episode_id = ? AND created_at > ?
                 ORDER BY start_sec",
            )
            .bind(uuid_str(&user_id))
            .bind(uuid_str(&episode_id))
            .bind(since.to_rfc3339())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
        } else {
            sqlx::query_as(
                "SELECT id, user_id, episode_id, start_sec, end_sec, label, advertisement, created_at
                 FROM chapters WHERE user_id = ? AND episode_id = ?
                 ORDER BY start_sec",
            )
            .bind(uuid_str(&user_id))
            .bind(uuid_str(&episode_id))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
        };

        Ok(rows
            .into_iter()
            .map(|(id, uid, eid, ss, es, label, adv, created)| Chapter {
                id: id.parse().unwrap_or_default(),
                user_id: uid.parse().unwrap_or_default(),
                episode_id: eid.parse().unwrap_or_default(),
                start_sec: ss,
                end_sec: es,
                label,
                advertisement: adv,
                created_at: created.parse().unwrap_or_default(),
            })
            .collect())
    }

    async fn delete(
        &self,
        user_id: Uuid,
        episode_id: Uuid,
        start_sec: i32,
        end_sec: i32,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM chapters WHERE user_id = ? AND episode_id = ? AND start_sec = ? AND end_sec = ?",
        )
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&episode_id))
        .bind(start_sec)
        .bind(end_sec)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// FavoriteRepo
// ---------------------------------------------------------------------------

impl repo::FavoriteRepo for SqliteRepo {
    async fn add(&self, user_id: Uuid, episode_id: Uuid) -> Result<()> {
        sqlx::query(
            "INSERT INTO favorite_episodes (id, user_id, episode_id) VALUES (?, ?, ?)
             ON CONFLICT (user_id, episode_id) DO NOTHING",
        )
        .bind(uuid_str(&Uuid::now_v7()))
        .bind(uuid_str(&user_id))
        .bind(uuid_str(&episode_id))
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn remove(&self, user_id: Uuid, episode_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM favorite_episodes WHERE user_id = ? AND episode_id = ?")
            .bind(uuid_str(&user_id))
            .bind(uuid_str(&episode_id))
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Episode>> {
        let rows: Vec<SqliteEpisodeRow> = sqlx::query_as(
            "SELECT e.id, e.podcast_id, e.guid, e.title, e.description, e.link,
                    e.released, e.duration, e.filesize, e.mimetype, e.created_at, e.updated_at
             FROM episodes e
             JOIN favorite_episodes fe ON fe.episode_id = e.id
             WHERE fe.user_id = ?
             ORDER BY fe.created_at DESC",
        )
        .bind(uuid_str(&user_id))
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
    use rpodder_core::repo::{
        DeviceRepo, EpisodeActionRepo, EpisodeRepo, PodcastRepo, SessionRepo, SubscriptionRepo,
        UserRepo,
    };

    async fn setup() -> SqliteRepo {
        let db = crate::Db::connect("sqlite::memory:").await.unwrap();
        db.migrate("../../migrations").await.unwrap();
        match db {
            crate::Db::Sqlite(pool) => SqliteRepo::new(pool),
            _ => unreachable!(),
        }
    }

    // === UserRepo tests ===

    #[tokio::test]
    async fn create_user_and_find_by_username() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "Alice", "hash123", Some("alice@example.com"))
            .await
            .unwrap();

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
        let user = UserRepo::create(&repo, "Charlie", "hash", None)
            .await
            .unwrap();

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
        UserRepo::create(&repo, "Dave", "hash1", None)
            .await
            .unwrap();

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
        let user = UserRepo::create(&repo, "NoEmail", "hash", None)
            .await
            .unwrap();
        assert!(user.email.is_none());

        let found = repo.find_by_username("NoEmail").await.unwrap().unwrap();
        assert!(found.email.is_none());
    }

    #[tokio::test]
    async fn list_all_users() {
        let repo = setup().await;
        UserRepo::create(&repo, "user1", "hash", Some("a@b.com"))
            .await
            .unwrap();
        UserRepo::create(&repo, "user2", "hash", None)
            .await
            .unwrap();
        let all = UserRepo::list_all(&repo).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].username, "user1");
        assert_eq!(all[1].username, "user2");
    }

    #[tokio::test]
    async fn set_admin_and_check() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "admin_test", "hash", None)
            .await
            .unwrap();
        assert!(!user.is_admin);

        UserRepo::set_admin(&repo, user.id, true).await.unwrap();
        let found = UserRepo::find_by_id(&repo, user.id).await.unwrap().unwrap();
        assert!(found.is_admin);

        UserRepo::set_admin(&repo, user.id, false).await.unwrap();
        let found = UserRepo::find_by_id(&repo, user.id).await.unwrap().unwrap();
        assert!(!found.is_admin);
    }

    #[tokio::test]
    async fn set_active_and_check() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "active_test", "hash", None)
            .await
            .unwrap();
        assert!(user.is_active);

        UserRepo::set_active(&repo, user.id, false).await.unwrap();
        let found = UserRepo::find_by_id(&repo, user.id).await.unwrap().unwrap();
        assert!(!found.is_active);

        UserRepo::set_active(&repo, user.id, true).await.unwrap();
        let found = UserRepo::find_by_id(&repo, user.id).await.unwrap().unwrap();
        assert!(found.is_active);
    }

    #[tokio::test]
    async fn delete_user() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "delete_me", "hash", None)
            .await
            .unwrap();
        UserRepo::delete(&repo, user.id).await.unwrap();
        let found = UserRepo::find_by_id(&repo, user.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn count_active_users() {
        let repo = setup().await;
        assert_eq!(UserRepo::count_active(&repo).await.unwrap(), 0);

        let u1 = UserRepo::create(&repo, "u1", "hash", None).await.unwrap();
        UserRepo::create(&repo, "u2", "hash", None).await.unwrap();
        assert_eq!(UserRepo::count_active(&repo).await.unwrap(), 2);

        UserRepo::set_active(&repo, u1.id, false).await.unwrap();
        assert_eq!(UserRepo::count_active(&repo).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn update_password() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "pw_test", "old_hash", None)
            .await
            .unwrap();
        assert_eq!(user.password_hash, "old_hash");

        UserRepo::update_password(&repo, user.id, "new_hash")
            .await
            .unwrap();
        let found = UserRepo::find_by_id(&repo, user.id).await.unwrap().unwrap();
        assert_eq!(found.password_hash, "new_hash");
    }

    #[tokio::test]
    async fn find_by_email() {
        let repo = setup().await;
        UserRepo::create(&repo, "email_user", "hash", Some("test@example.com"))
            .await
            .unwrap();

        let found = UserRepo::find_by_email(&repo, "test@example.com")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "email_user");

        // Case insensitive
        let found = UserRepo::find_by_email(&repo, "TEST@Example.COM")
            .await
            .unwrap();
        assert!(found.is_some());

        // Not found
        let found = UserRepo::find_by_email(&repo, "nobody@example.com")
            .await
            .unwrap();
        assert!(found.is_none());
    }

    // === SessionRepo tests ===

    #[tokio::test]
    async fn create_session_and_find_by_token() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "SessionUser", "hash", None)
            .await
            .unwrap();

        let session = Session {
            id: Uuid::now_v7(),
            user_id: user.id,
            token: "test-token-abc123".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            created_at: Utc::now(),
        };
        SessionRepo::create(&repo, &session).await.unwrap();

        let found = repo
            .find_by_token("test-token-abc123")
            .await
            .unwrap()
            .unwrap();
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
        let user = UserRepo::create(&repo, "ExpiredUser", "hash", None)
            .await
            .unwrap();

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
        let user = UserRepo::create(&repo, "DeleteUser", "hash", None)
            .await
            .unwrap();

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
        let user = UserRepo::create(&repo, "CleanupUser", "hash", None)
            .await
            .unwrap();

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
        let user = UserRepo::create(&repo, "MultiSession", "hash", None)
            .await
            .unwrap();

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
            let found = repo
                .find_by_token(&format!("multi-token-{i}"))
                .await
                .unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().user_id, user.id);
        }
    }

    // === DeviceRepo tests ===

    #[tokio::test]
    async fn create_device_and_find_by_uid() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "DevUser", "hash", None)
            .await
            .unwrap();

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

        let found = DeviceRepo::find_by_uid(&repo, user.id, "my-phone")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.device_id, "my-phone");
        assert_eq!(found.caption, "My Phone");
        assert_eq!(found.device_type, DeviceType::Mobile);
    }

    #[tokio::test]
    async fn upsert_device_updates_existing() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "UpsertUser", "hash", None)
            .await
            .unwrap();

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
        let user = UserRepo::create(&repo, "ListUser", "hash", None)
            .await
            .unwrap();
        let other = UserRepo::create(&repo, "OtherUser", "hash", None)
            .await
            .unwrap();

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
        let user = UserRepo::create(&repo, "NoDevUser", "hash", None)
            .await
            .unwrap();
        let found = DeviceRepo::find_by_uid(&repo, user.id, "nonexistent")
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn list_devices_empty() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "EmptyUser", "hash", None)
            .await
            .unwrap();
        let devices = DeviceRepo::list_for_user(&repo, user.id).await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn device_type_roundtrip() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "TypeUser", "hash", None)
            .await
            .unwrap();

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

            let found = DeviceRepo::find_by_uid(&repo, user.id, &did)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(found.device_type, dt, "roundtrip failed for {:?}", dt);
        }
    }

    // === PodcastRepo tests ===

    #[tokio::test]
    async fn get_or_create_podcast_for_url() {
        let repo = setup().await;
        let (podcast, created) =
            PodcastRepo::get_or_create_for_url(&repo, "http://example.com/feed.xml")
                .await
                .unwrap();
        assert!(created);
        assert_eq!(podcast.title, "http://example.com/feed.xml");

        // Second call should return existing
        let (podcast2, created2) =
            PodcastRepo::get_or_create_for_url(&repo, "http://example.com/feed.xml")
                .await
                .unwrap();
        assert!(!created2);
        assert_eq!(podcast2.id, podcast.id);
    }

    #[tokio::test]
    async fn find_podcast_by_url() {
        let repo = setup().await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/rss")
            .await
            .unwrap();

        let found = PodcastRepo::find_by_url(&repo, "http://test.com/rss")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, podcast.id);

        let not_found = PodcastRepo::find_by_url(&repo, "http://other.com/rss")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn find_podcast_by_id() {
        let repo = setup().await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();

        let found = PodcastRepo::find_by_id(&repo, podcast.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.title, "http://test.com/feed");

        let not_found = PodcastRepo::find_by_id(&repo, Uuid::now_v7())
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    // === SubscriptionRepo tests ===

    /// Helper: create user + device + podcast for subscription tests
    async fn setup_subscription_fixtures(repo: &SqliteRepo) -> (User, Device, Podcast) {
        let user = UserRepo::create(repo, "SubUser", "hash", None)
            .await
            .unwrap();
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
        let (podcast, _) = PodcastRepo::get_or_create_for_url(repo, "http://example.com/feed.xml")
            .await
            .unwrap();
        (user, device, podcast)
    }

    #[tokio::test]
    async fn subscribe_and_list() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(
            &repo,
            user.id,
            device.id,
            podcast.id,
            "http://example.com/feed.xml",
        )
        .await
        .unwrap();

        let subs = SubscriptionRepo::list_for_device(&repo, user.id, device.id)
            .await
            .unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].ref_url, "http://example.com/feed.xml");
    }

    #[tokio::test]
    async fn subscribe_idempotent() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(
            &repo,
            user.id,
            device.id,
            podcast.id,
            "http://example.com/feed.xml",
        )
        .await
        .unwrap();
        SubscriptionRepo::subscribe(
            &repo,
            user.id,
            device.id,
            podcast.id,
            "http://example.com/feed.xml",
        )
        .await
        .unwrap();

        let subs = SubscriptionRepo::list_for_device(&repo, user.id, device.id)
            .await
            .unwrap();
        assert_eq!(subs.len(), 1);
    }

    #[tokio::test]
    async fn unsubscribe() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(
            &repo,
            user.id,
            device.id,
            podcast.id,
            "http://example.com/feed.xml",
        )
        .await
        .unwrap();
        SubscriptionRepo::unsubscribe(&repo, user.id, device.id, podcast.id)
            .await
            .unwrap();

        let subs = SubscriptionRepo::list_for_device(&repo, user.id, device.id)
            .await
            .unwrap();
        assert!(subs.is_empty());
    }

    #[tokio::test]
    async fn list_subscriptions_for_user() {
        let repo = setup().await;
        let user = UserRepo::create(&repo, "MultiSubUser", "hash", None)
            .await
            .unwrap();
        let now = Utc::now();

        // Two devices
        let dev1 = DeviceRepo::upsert(
            &repo,
            &Device {
                id: Uuid::now_v7(),
                user_id: user.id,
                device_id: "dev1".into(),
                caption: "".into(),
                device_type: DeviceType::Other,
                sync_group_id: None,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        let dev2 = DeviceRepo::upsert(
            &repo,
            &Device {
                id: Uuid::now_v7(),
                user_id: user.id,
                device_id: "dev2".into(),
                caption: "".into(),
                device_type: DeviceType::Other,
                sync_group_id: None,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();

        // Same podcast on both devices
        let (p1, _) = PodcastRepo::get_or_create_for_url(&repo, "http://feed1.com")
            .await
            .unwrap();
        let (p2, _) = PodcastRepo::get_or_create_for_url(&repo, "http://feed2.com")
            .await
            .unwrap();

        SubscriptionRepo::subscribe(&repo, user.id, dev1.id, p1.id, "http://feed1.com")
            .await
            .unwrap();
        SubscriptionRepo::subscribe(&repo, user.id, dev2.id, p1.id, "http://feed1.com")
            .await
            .unwrap();
        SubscriptionRepo::subscribe(&repo, user.id, dev1.id, p2.id, "http://feed2.com")
            .await
            .unwrap();

        // list_for_user should deduplicate by podcast
        let subs = SubscriptionRepo::list_for_user(&repo, user.id)
            .await
            .unwrap();
        assert_eq!(subs.len(), 2);
    }

    #[tokio::test]
    async fn changes_since_tracks_history() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        let before = Utc::now() - Duration::seconds(1);

        SubscriptionRepo::subscribe(
            &repo,
            user.id,
            device.id,
            podcast.id,
            "http://example.com/feed.xml",
        )
        .await
        .unwrap();
        SubscriptionRepo::unsubscribe(&repo, user.id, device.id, podcast.id)
            .await
            .unwrap();

        let changes = SubscriptionRepo::changes_since(&repo, user.id, device.id, before)
            .await
            .unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].action, SubscriptionAction::Subscribe);
        assert_eq!(changes[1].action, SubscriptionAction::Unsubscribe);
    }

    #[tokio::test]
    async fn changes_since_filters_by_time() {
        let repo = setup().await;
        let (user, device, podcast) = setup_subscription_fixtures(&repo).await;

        SubscriptionRepo::subscribe(
            &repo,
            user.id,
            device.id,
            podcast.id,
            "http://example.com/feed.xml",
        )
        .await
        .unwrap();

        let after = Utc::now() + Duration::seconds(1);

        let changes = SubscriptionRepo::changes_since(&repo, user.id, device.id, after)
            .await
            .unwrap();
        assert!(changes.is_empty());
    }

    // === EpisodeRepo tests ===

    #[tokio::test]
    async fn get_or_create_episode_for_url() {
        let repo = setup().await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();

        let (episode, created) =
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://test.com/ep1.mp3")
                .await
                .unwrap();
        assert!(created);
        assert_eq!(episode.title, "http://test.com/ep1.mp3");

        let (episode2, created2) =
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://test.com/ep1.mp3")
                .await
                .unwrap();
        assert!(!created2);
        assert_eq!(episode2.id, episode.id);
    }

    #[tokio::test]
    async fn find_episode_by_id() {
        let repo = setup().await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();
        let (episode, _) =
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://test.com/ep1.mp3")
                .await
                .unwrap();

        let found = EpisodeRepo::find_by_id(&repo, episode.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, episode.id);

        let not_found = EpisodeRepo::find_by_id(&repo, Uuid::now_v7())
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn list_episodes_for_podcast() {
        let repo = setup().await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();

        for i in 0..3 {
            EpisodeRepo::get_or_create_for_url(
                &repo,
                podcast.id,
                &format!("http://test.com/ep{i}.mp3"),
            )
            .await
            .unwrap();
        }

        let eps = EpisodeRepo::list_for_podcast(&repo, podcast.id, 10, 0)
            .await
            .unwrap();
        assert_eq!(eps.len(), 3);
    }

    // === EpisodeActionRepo tests ===

    #[tokio::test]
    async fn create_and_list_episode_actions() {
        let repo = setup().await;
        let (user, device, _) = setup_subscription_fixtures(&repo).await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();
        let (episode, _) =
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://test.com/ep1.mp3")
                .await
                .unwrap();

        let action = EpisodeAction {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: Some(device.id),
            episode_id: episode.id,
            action: EpisodeActionType::Play,
            podcast_ref_url: Some("http://test.com/feed".into()),
            episode_ref_url: Some("http://test.com/ep1.mp3".into()),
            started: Some(0),
            position: Some(120),
            total: Some(3600),
            timestamp: Utc::now(),
            created_at: Utc::now(),
        };
        EpisodeActionRepo::create(&repo, &action).await.unwrap();

        let actions = EpisodeActionRepo::list(&repo, user.id, None, None, None, 100)
            .await
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action, EpisodeActionType::Play);
        assert_eq!(actions[0].position, Some(120));
    }

    #[tokio::test]
    async fn episode_action_deduplication() {
        let repo = setup().await;
        let (user, device, _) = setup_subscription_fixtures(&repo).await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();
        let (episode, _) =
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://test.com/ep1.mp3")
                .await
                .unwrap();

        let ts = Utc::now();
        let action = EpisodeAction {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: Some(device.id),
            episode_id: episode.id,
            action: EpisodeActionType::Play,
            podcast_ref_url: None,
            episode_ref_url: None,
            started: Some(0),
            position: Some(100),
            total: Some(3600),
            timestamp: ts,
            created_at: Utc::now(),
        };
        EpisodeActionRepo::create(&repo, &action).await.unwrap();

        // Same user+episode+device+action+timestamp, different position → should update
        let action2 = EpisodeAction {
            id: Uuid::now_v7(),
            position: Some(200),
            ..action.clone()
        };
        EpisodeActionRepo::create(&repo, &action2).await.unwrap();

        let actions = EpisodeActionRepo::list(&repo, user.id, None, None, None, 100)
            .await
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].position, Some(200));
    }

    #[tokio::test]
    async fn episode_action_filter_by_device() {
        let repo = setup().await;
        let (user, device, _) = setup_subscription_fixtures(&repo).await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();
        let (episode, _) =
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://test.com/ep.mp3")
                .await
                .unwrap();

        // Action with device
        let a1 = EpisodeAction {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: Some(device.id),
            episode_id: episode.id,
            action: EpisodeActionType::Download,
            podcast_ref_url: None,
            episode_ref_url: None,
            started: None,
            position: None,
            total: None,
            timestamp: Utc::now(),
            created_at: Utc::now(),
        };
        EpisodeActionRepo::create(&repo, &a1).await.unwrap();

        // Action without device
        let a2 = EpisodeAction {
            id: Uuid::now_v7(),
            device_id: None,
            action: EpisodeActionType::New,
            timestamp: Utc::now() + Duration::seconds(1),
            ..a1.clone()
        };
        EpisodeActionRepo::create(&repo, &a2).await.unwrap();

        let all = EpisodeActionRepo::list(&repo, user.id, None, None, None, 100)
            .await
            .unwrap();
        assert_eq!(all.len(), 2);

        let filtered = EpisodeActionRepo::list(&repo, user.id, Some(device.id), None, None, 100)
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].action, EpisodeActionType::Download);
    }

    #[tokio::test]
    async fn episode_action_filter_by_since() {
        let repo = setup().await;
        let (user, device, _) = setup_subscription_fixtures(&repo).await;
        let (podcast, _) = PodcastRepo::get_or_create_for_url(&repo, "http://test.com/feed")
            .await
            .unwrap();
        let (episode, _) =
            EpisodeRepo::get_or_create_for_url(&repo, podcast.id, "http://test.com/ep.mp3")
                .await
                .unwrap();

        let before = Utc::now();

        let action = EpisodeAction {
            id: Uuid::now_v7(),
            user_id: user.id,
            device_id: Some(device.id),
            episode_id: episode.id,
            action: EpisodeActionType::Play,
            podcast_ref_url: None,
            episode_ref_url: None,
            started: Some(0),
            position: Some(60),
            total: Some(300),
            timestamp: Utc::now(),
            created_at: Utc::now(),
        };
        EpisodeActionRepo::create(&repo, &action).await.unwrap();

        let since_before = EpisodeActionRepo::list(&repo, user.id, None, None, Some(before), 100)
            .await
            .unwrap();
        assert_eq!(since_before.len(), 1);

        let since_after = EpisodeActionRepo::list(
            &repo,
            user.id,
            None,
            None,
            Some(Utc::now() + Duration::seconds(1)),
            100,
        )
        .await
        .unwrap();
        assert!(since_after.is_empty());
    }
}
