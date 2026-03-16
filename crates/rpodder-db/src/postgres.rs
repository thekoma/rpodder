use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use rpodder_core::error::AppError;
use rpodder_core::repo::{self, Result};
use rpodder_core::types::*;

#[derive(Clone)]
pub struct PgRepo {
    pub pool: PgPool,
}

impl PgRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn db_err(e: sqlx::Error) -> AppError {
    AppError::Internal(e.to_string())
}

// ---------------------------------------------------------------------------
// UserRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    password_hash: String,
    email: Option<String>,
    is_active: bool,
    created_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(r: UserRow) -> Self {
        User {
            id: r.id,
            username: r.username,
            password_hash: r.password_hash,
            email: r.email,
            is_active: r.is_active,
            created_at: r.created_at,
        }
    }
}

impl repo::UserRepo for PgRepo {
    async fn create(&self, username: &str, password_hash: &str, email: Option<&str>) -> Result<User> {
        let id = Uuid::now_v7();
        let row: UserRow = sqlx::query_as(
            "INSERT INTO users (id, username, password_hash, email)
             VALUES ($1, $2, $3, $4)
             RETURNING id, username, password_hash, email, is_active, created_at",
        )
        .bind(id)
        .bind(username)
        .bind(password_hash)
        .bind(email)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(dbe) if dbe.is_unique_violation() => {
                AppError::Conflict(format!("user '{username}' already exists"))
            }
            _ => db_err(e),
        })?;
        Ok(row.into())
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE LOWER(username) = LOWER($1)",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, email, is_active, created_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }
}

// ---------------------------------------------------------------------------
// SessionRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    token: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<SessionRow> for Session {
    fn from(r: SessionRow) -> Self {
        Session {
            id: r.id,
            user_id: r.user_id,
            token: r.token,
            expires_at: r.expires_at,
            created_at: r.created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// DeviceRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    user_id: Uuid,
    device_id: String,
    caption: String,
    device_type: String,
    sync_group_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<DeviceRow> for Device {
    fn from(r: DeviceRow) -> Self {
        Device {
            id: r.id,
            user_id: r.user_id,
            device_id: r.device_id,
            caption: r.caption,
            device_type: parse_device_type(&r.device_type),
            sync_group_id: r.sync_group_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

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

impl repo::DeviceRepo for PgRepo {
    async fn upsert(&self, device: &Device) -> Result<Device> {
        let row: DeviceRow = sqlx::query_as(
            "INSERT INTO devices (id, user_id, device_id, caption, device_type, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (user_id, device_id)
             DO UPDATE SET caption = EXCLUDED.caption, device_type = EXCLUDED.device_type, updated_at = EXCLUDED.updated_at
             RETURNING id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at",
        )
        .bind(device.id)
        .bind(device.user_id)
        .bind(&device.device_id)
        .bind(&device.caption)
        .bind(device_type_str(device.device_type))
        .bind(device.created_at)
        .bind(device.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.into())
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Device>> {
        let rows: Vec<DeviceRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
             FROM devices WHERE user_id = $1 ORDER BY created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn find_by_uid(&self, user_id: Uuid, device_id: &str) -> Result<Option<Device>> {
        let row: Option<DeviceRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, caption, device_type, sync_group_id, created_at, updated_at
             FROM devices WHERE user_id = $1 AND device_id = $2",
        )
        .bind(user_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }
}

// ---------------------------------------------------------------------------
// PodcastRepo (partial — get_or_create_for_url needed for subscriptions)
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct PodcastRow {
    id: Uuid,
    title: String,
    description: String,
    link: Option<String>,
    language: Option<String>,
    logo_url: Option<String>,
    author: Option<String>,
    subscribers: i64,
    episode_count: i64,
    last_update: Option<DateTime<Utc>>,
    update_interval_hours: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<PodcastRow> for Podcast {
    fn from(r: PodcastRow) -> Self {
        Podcast {
            id: r.id,
            title: r.title,
            description: r.description,
            link: r.link,
            language: r.language,
            logo_url: r.logo_url,
            author: r.author,
            subscribers: r.subscribers,
            episode_count: r.episode_count,
            last_update: r.last_update,
            update_interval_hours: r.update_interval_hours,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

impl repo::PodcastRepo for PgRepo {
    async fn get_or_create_for_url(&self, url: &str) -> Result<(Podcast, bool)> {
        // Check if a podcast with this URL already exists
        if let Some(podcast) = self.find_by_url(url).await? {
            return Ok((podcast, false));
        }

        // Create a stub podcast and its URL entry
        let id = Uuid::now_v7();
        let url_id = Uuid::now_v7();
        let now = Utc::now();

        let row: PodcastRow = sqlx::query_as(
            "INSERT INTO podcasts (id, title, created_at, updated_at)
             VALUES ($1, $2, $3, $3)
             RETURNING id, title, description, link, language, logo_url, author,
                       subscribers, episode_count, last_update, update_interval_hours,
                       created_at, updated_at",
        )
        .bind(id)
        .bind(url) // Use URL as initial title
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        sqlx::query(
            "INSERT INTO podcast_urls (id, podcast_id, url, \"order\")
             VALUES ($1, $2, $3, 0)",
        )
        .bind(url_id)
        .bind(id)
        .bind(url)
        .execute(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(dbe) if dbe.is_unique_violation() => {
                // Race condition: another request created this URL. Fetch it.
                AppError::Conflict("podcast URL already exists".into())
            }
            _ => db_err(e),
        })?;

        Ok((row.into(), true))
    }

    async fn find_by_url(&self, url: &str) -> Result<Option<Podcast>> {
        let row: Option<PodcastRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.created_at, p.updated_at
             FROM podcasts p
             JOIN podcast_urls pu ON pu.podcast_id = p.id
             WHERE pu.url = $1",
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Podcast>> {
        let row: Option<PodcastRow> = sqlx::query_as(
            "SELECT id, title, description, link, language, logo_url, author,
                    subscribers, episode_count, last_update, update_interval_hours,
                    created_at, updated_at
             FROM podcasts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn update(&self, podcast: &Podcast) -> Result<()> {
        sqlx::query(
            "UPDATE podcasts SET title = $2, description = $3, link = $4, language = $5,
                    logo_url = $6, author = $7, subscribers = $8, episode_count = $9,
                    last_update = $10, update_interval_hours = $11, updated_at = $12
             WHERE id = $1",
        )
        .bind(podcast.id)
        .bind(&podcast.title)
        .bind(&podcast.description)
        .bind(&podcast.link)
        .bind(&podcast.language)
        .bind(&podcast.logo_url)
        .bind(&podcast.author)
        .bind(podcast.subscribers)
        .bind(podcast.episode_count)
        .bind(podcast.last_update)
        .bind(podcast.update_interval_hours)
        .bind(podcast.updated_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn toplist(&self, count: i64, language: Option<&str>) -> Result<Vec<Podcast>> {
        let rows: Vec<PodcastRow> = if let Some(lang) = language {
            sqlx::query_as(
                "SELECT id, title, description, link, language, logo_url, author,
                        subscribers, episode_count, last_update, update_interval_hours,
                        created_at, updated_at
                 FROM podcasts WHERE language = $1 ORDER BY subscribers DESC LIMIT $2",
            )
            .bind(lang)
            .bind(count)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
        } else {
            sqlx::query_as(
                "SELECT id, title, description, link, language, logo_url, author,
                        subscribers, episode_count, last_update, update_interval_hours,
                        created_at, updated_at
                 FROM podcasts ORDER BY subscribers DESC LIMIT $1",
            )
            .bind(count)
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
        };
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn search(&self, query: &str, limit: i64) -> Result<Vec<Podcast>> {
        let rows: Vec<PodcastRow> = sqlx::query_as(
            "SELECT id, title, description, link, language, logo_url, author,
                    subscribers, episode_count, last_update, update_interval_hours,
                    created_at, updated_at
             FROM podcasts
             WHERE search_vector @@ plainto_tsquery('english', $1)
             ORDER BY subscribers DESC LIMIT $2",
        )
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ---------------------------------------------------------------------------
// SubscriptionRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct SubscriptionRow {
    id: Uuid,
    user_id: Uuid,
    device_id: Uuid,
    podcast_id: Uuid,
    ref_url: String,
    created_at: DateTime<Utc>,
}

impl From<SubscriptionRow> for Subscription {
    fn from(r: SubscriptionRow) -> Self {
        Subscription {
            id: r.id,
            user_id: r.user_id,
            device_id: r.device_id,
            podcast_id: r.podcast_id,
            ref_url: r.ref_url,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct SubChangeRow {
    id: Uuid,
    user_id: Uuid,
    device_id: Uuid,
    podcast_id: Uuid,
    action: String,
    ref_url: String,
    timestamp: DateTime<Utc>,
}

impl From<SubChangeRow> for SubscriptionChange {
    fn from(r: SubChangeRow) -> Self {
        SubscriptionChange {
            id: r.id,
            user_id: r.user_id,
            device_id: r.device_id,
            podcast_id: r.podcast_id,
            action: match r.action.as_str() {
                "subscribe" => SubscriptionAction::Subscribe,
                _ => SubscriptionAction::Unsubscribe,
            },
            ref_url: r.ref_url,
            timestamp: r.timestamp,
        }
    }
}

impl repo::SubscriptionRepo for PgRepo {
    async fn subscribe(&self, user_id: Uuid, device_id: Uuid, podcast_id: Uuid, ref_url: &str) -> Result<()> {
        let sub_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO subscriptions (id, user_id, device_id, podcast_id, ref_url)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, device_id, podcast_id) DO NOTHING",
        )
        .bind(sub_id)
        .bind(user_id)
        .bind(device_id)
        .bind(podcast_id)
        .bind(ref_url)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        // Record the change for delta sync
        let change_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO subscription_changes (id, user_id, device_id, podcast_id, action, ref_url)
             VALUES ($1, $2, $3, $4, 'subscribe', $5)",
        )
        .bind(change_id)
        .bind(user_id)
        .bind(device_id)
        .bind(podcast_id)
        .bind(ref_url)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(())
    }

    async fn unsubscribe(&self, user_id: Uuid, device_id: Uuid, podcast_id: Uuid) -> Result<()> {
        // Get the ref_url before deleting
        let row: Option<SubscriptionRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, podcast_id, ref_url, created_at
             FROM subscriptions WHERE user_id = $1 AND device_id = $2 AND podcast_id = $3",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(podcast_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        let ref_url = row.map(|r| r.ref_url).unwrap_or_default();

        sqlx::query(
            "DELETE FROM subscriptions WHERE user_id = $1 AND device_id = $2 AND podcast_id = $3",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(podcast_id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        // Record the change
        let change_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO subscription_changes (id, user_id, device_id, podcast_id, action, ref_url)
             VALUES ($1, $2, $3, $4, 'unsubscribe', $5)",
        )
        .bind(change_id)
        .bind(user_id)
        .bind(device_id)
        .bind(podcast_id)
        .bind(&ref_url)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        Ok(())
    }

    async fn list_for_device(&self, user_id: Uuid, device_id: Uuid) -> Result<Vec<Subscription>> {
        let rows: Vec<SubscriptionRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, podcast_id, ref_url, created_at
             FROM subscriptions WHERE user_id = $1 AND device_id = $2",
        )
        .bind(user_id)
        .bind(device_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Subscription>> {
        let rows: Vec<SubscriptionRow> = sqlx::query_as(
            "SELECT DISTINCT ON (podcast_id) id, user_id, device_id, podcast_id, ref_url, created_at
             FROM subscriptions WHERE user_id = $1
             ORDER BY podcast_id, created_at",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn changes_since(&self, user_id: Uuid, device_id: Uuid, since: DateTime<Utc>) -> Result<Vec<SubscriptionChange>> {
        let rows: Vec<SubChangeRow> = sqlx::query_as(
            "SELECT id, user_id, device_id, podcast_id, action, ref_url, timestamp
             FROM subscription_changes
             WHERE user_id = $1 AND device_id = $2 AND timestamp > $3
             ORDER BY timestamp",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ---------------------------------------------------------------------------
// EpisodeRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct EpisodeRow {
    id: Uuid,
    podcast_id: Uuid,
    guid: Option<String>,
    title: String,
    description: String,
    link: Option<String>,
    released: Option<DateTime<Utc>>,
    duration: Option<i64>,
    filesize: Option<i64>,
    mimetype: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<EpisodeRow> for Episode {
    fn from(r: EpisodeRow) -> Self {
        Episode {
            id: r.id,
            podcast_id: r.podcast_id,
            guid: r.guid,
            title: r.title,
            description: r.description,
            link: r.link,
            released: r.released,
            duration: r.duration,
            filesize: r.filesize,
            mimetype: r.mimetype,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

impl repo::EpisodeRepo for PgRepo {
    async fn get_or_create_for_url(&self, podcast_id: Uuid, url: &str) -> Result<(Episode, bool)> {
        // Check if episode URL already exists
        let existing: Option<EpisodeRow> = sqlx::query_as(
            "SELECT e.id, e.podcast_id, e.guid, e.title, e.description, e.link,
                    e.released, e.duration, e.filesize, e.mimetype, e.created_at, e.updated_at
             FROM episodes e
             JOIN episode_urls eu ON eu.episode_id = e.id
             WHERE eu.url = $1",
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;

        if let Some(row) = existing {
            return Ok((row.into(), false));
        }

        let id = Uuid::now_v7();
        let url_id = Uuid::now_v7();
        let now = Utc::now();

        let row: EpisodeRow = sqlx::query_as(
            "INSERT INTO episodes (id, podcast_id, title, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $4)
             RETURNING id, podcast_id, guid, title, description, link,
                       released, duration, filesize, mimetype, created_at, updated_at",
        )
        .bind(id)
        .bind(podcast_id)
        .bind(url)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;

        sqlx::query(
            "INSERT INTO episode_urls (id, episode_id, url, \"order\")
             VALUES ($1, $2, $3, 0)",
        )
        .bind(url_id)
        .bind(id)
        .bind(url)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;

        Ok((row.into(), true))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Episode>> {
        let row: Option<EpisodeRow> = sqlx::query_as(
            "SELECT id, podcast_id, guid, title, description, link,
                    released, duration, filesize, mimetype, created_at, updated_at
             FROM episodes WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn list_for_podcast(&self, podcast_id: Uuid, limit: i64, offset: i64) -> Result<Vec<Episode>> {
        let rows: Vec<EpisodeRow> = sqlx::query_as(
            "SELECT id, podcast_id, guid, title, description, link,
                    released, duration, filesize, mimetype, created_at, updated_at
             FROM episodes WHERE podcast_id = $1
             ORDER BY released DESC NULLS LAST
             LIMIT $2 OFFSET $3",
        )
        .bind(podcast_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn update(&self, episode: &Episode) -> Result<()> {
        sqlx::query(
            "UPDATE episodes SET guid = $2, title = $3, description = $4, link = $5,
                    released = $6, duration = $7, filesize = $8, mimetype = $9, updated_at = $10
             WHERE id = $1",
        )
        .bind(episode.id)
        .bind(&episode.guid)
        .bind(&episode.title)
        .bind(&episode.description)
        .bind(&episode.link)
        .bind(episode.released)
        .bind(episode.duration)
        .bind(episode.filesize)
        .bind(&episode.mimetype)
        .bind(episode.updated_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EpisodeActionRepo
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct EpActionRow {
    id: Uuid,
    user_id: Uuid,
    device_id: Option<Uuid>,
    episode_id: Uuid,
    action: String,
    podcast_ref_url: Option<String>,
    episode_ref_url: Option<String>,
    started: Option<i32>,
    position: Option<i32>,
    total: Option<i32>,
    timestamp: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

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

impl From<EpActionRow> for EpisodeAction {
    fn from(r: EpActionRow) -> Self {
        EpisodeAction {
            id: r.id,
            user_id: r.user_id,
            device_id: r.device_id,
            episode_id: r.episode_id,
            action: parse_action_type(&r.action),
            podcast_ref_url: r.podcast_ref_url,
            episode_ref_url: r.episode_ref_url,
            started: r.started,
            position: r.position,
            total: r.total,
            timestamp: r.timestamp,
            created_at: r.created_at,
        }
    }
}

impl repo::EpisodeActionRepo for PgRepo {
    async fn create(&self, action: &EpisodeAction) -> Result<()> {
        sqlx::query(
            "INSERT INTO episode_actions
                (id, user_id, device_id, episode_id, action, podcast_ref_url, episode_ref_url,
                 started, position, total, timestamp, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (user_id, episode_id, COALESCE(device_id, '00000000-0000-0000-0000-000000000000'), action, timestamp)
             DO UPDATE SET position = EXCLUDED.position, started = EXCLUDED.started, total = EXCLUDED.total",
        )
        .bind(action.id)
        .bind(action.user_id)
        .bind(action.device_id)
        .bind(action.episode_id)
        .bind(action_type_str(action.action))
        .bind(&action.podcast_ref_url)
        .bind(&action.episode_ref_url)
        .bind(action.started)
        .bind(action.position)
        .bind(action.total)
        .bind(action.timestamp)
        .bind(action.created_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
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
        // Build query dynamically based on filters
        let mut sql = String::from(
            "SELECT ea.id, ea.user_id, ea.device_id, ea.episode_id, ea.action,
                    ea.podcast_ref_url, ea.episode_ref_url,
                    ea.started, ea.position, ea.total, ea.timestamp, ea.created_at
             FROM episode_actions ea
             WHERE ea.user_id = $1",
        );
        let mut param_idx = 2u32;

        if device_id.is_some() {
            sql.push_str(&format!(" AND ea.device_id = ${param_idx}"));
            param_idx += 1;
        }
        if podcast_id.is_some() {
            sql.push_str(&format!(
                " AND ea.episode_id IN (SELECT id FROM episodes WHERE podcast_id = ${param_idx})"
            ));
            param_idx += 1;
        }
        if since.is_some() {
            sql.push_str(&format!(" AND ea.timestamp > ${param_idx}"));
            param_idx += 1;
        }
        sql.push_str(&format!(" ORDER BY ea.timestamp LIMIT ${param_idx}"));

        let mut q = sqlx::query_as::<_, EpActionRow>(&sql).bind(user_id);
        if let Some(did) = device_id {
            q = q.bind(did);
        }
        if let Some(pid) = podcast_id {
            q = q.bind(pid);
        }
        if let Some(s) = since {
            q = q.bind(s);
        }
        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await.map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ---------------------------------------------------------------------------
// TagRepo
// ---------------------------------------------------------------------------

impl repo::TagRepo for PgRepo {
    async fn top_tags(&self, count: i64) -> Result<Vec<(String, i64)>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT tag, COUNT(DISTINCT podcast_id) as cnt
             FROM tags
             GROUP BY tag
             ORDER BY cnt DESC
             LIMIT $1",
        )
        .bind(count)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows)
    }

    async fn podcasts_for_tag(&self, tag: &str, count: i64) -> Result<Vec<Podcast>> {
        let rows: Vec<PodcastRow> = sqlx::query_as(
            "SELECT p.id, p.title, p.description, p.link, p.language, p.logo_url, p.author,
                    p.subscribers, p.episode_count, p.last_update, p.update_interval_hours,
                    p.created_at, p.updated_at
             FROM podcasts p
             JOIN tags t ON t.podcast_id = p.id
             WHERE LOWER(t.tag) = LOWER($1)
             ORDER BY p.subscribers DESC
             LIMIT $2",
        )
        .bind(tag)
        .bind(count)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn set_tags_for_podcast(&self, podcast_id: Uuid, tags: &[Tag]) -> Result<()> {
        // Delete existing feed tags for this podcast
        sqlx::query("DELETE FROM tags WHERE podcast_id = $1 AND source = 'feed'")
            .bind(podcast_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;

        for tag in tags {
            sqlx::query(
                "INSERT INTO tags (id, tag, source, user_id, podcast_id)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (tag, source, user_id, podcast_id) DO NOTHING",
            )
            .bind(tag.id)
            .bind(&tag.tag)
            .bind(match tag.source {
                TagSource::Feed => "feed",
                TagSource::User => "user",
            })
            .bind(tag.user_id)
            .bind(tag.podcast_id)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        }

        Ok(())
    }
}

impl repo::SessionRepo for PgRepo {
    async fn create(&self, session: &Session) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(&session.token)
        .bind(session.expires_at)
        .bind(session.created_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn find_by_token(&self, token: &str) -> Result<Option<Session>> {
        let row: Option<SessionRow> = sqlx::query_as(
            "SELECT id, user_id, token, expires_at, created_at
             FROM sessions WHERE token = $1 AND expires_at > NOW()",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }

    async fn delete(&self, token: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token = $1")
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn delete_expired(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= NOW()")
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected())
    }
}
