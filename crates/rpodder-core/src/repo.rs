//! Repository traits — the boundary between domain logic and storage.
//!
//! Each trait is a small, focused interface. Implementations live in rpodder-db.
//! This design allows swapping PostgreSQL ↔ SQLite without touching business logic.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::AppError;
use crate::types::*;

pub type Result<T> = std::result::Result<T, AppError>;

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

pub trait UserRepo: Send + Sync {
    fn create(
        &self,
        username: &str,
        password_hash: &str,
        email: Option<&str>,
    ) -> impl std::future::Future<Output = Result<User>> + Send;
    fn find_by_username(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Result<Option<User>>> + Send;
    fn find_by_id(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<User>>> + Send;
    fn list_all(&self) -> impl std::future::Future<Output = Result<Vec<User>>> + Send;
    fn set_admin(
        &self,
        user_id: Uuid,
        is_admin: bool,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn set_active(
        &self,
        user_id: Uuid,
        is_active: bool,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn delete(&self, user_id: Uuid) -> impl std::future::Future<Output = Result<()>> + Send;
    fn count_active(&self) -> impl std::future::Future<Output = Result<i64>> + Send;
    fn update_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn find_by_email(
        &self,
        email: &str,
    ) -> impl std::future::Future<Output = Result<Option<User>>> + Send;
}

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

pub trait DeviceRepo: Send + Sync {
    fn upsert(&self, device: &Device) -> impl std::future::Future<Output = Result<Device>> + Send;
    fn list_for_user(
        &self,
        user_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<Device>>> + Send;
    fn find_by_uid(
        &self,
        user_id: Uuid,
        device_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Device>>> + Send;
}

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

pub trait SubscriptionRepo: Send + Sync {
    fn subscribe(
        &self,
        user_id: Uuid,
        device_id: Uuid,
        podcast_id: Uuid,
        ref_url: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn unsubscribe(
        &self,
        user_id: Uuid,
        device_id: Uuid,
        podcast_id: Uuid,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get current subscriptions for a device.
    fn list_for_device(
        &self,
        user_id: Uuid,
        device_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<Subscription>>> + Send;

    /// Get all subscriptions for a user (across all devices).
    fn list_for_user(
        &self,
        user_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<Subscription>>> + Send;

    /// Get subscription changes since a timestamp (for delta sync).
    fn changes_since(
        &self,
        user_id: Uuid,
        device_id: Uuid,
        since: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<Vec<SubscriptionChange>>> + Send;

    /// Move all subscriptions from one podcast to another (for dedup merging).
    fn migrate_podcast(
        &self,
        from_podcast_id: Uuid,
        to_podcast_id: Uuid,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Episode Actions
// ---------------------------------------------------------------------------

pub trait EpisodeActionRepo: Send + Sync {
    fn create(
        &self,
        action: &EpisodeAction,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn list(
        &self,
        user_id: Uuid,
        device_id: Option<Uuid>,
        podcast_id: Option<Uuid>,
        since: Option<DateTime<Utc>>,
        limit: i64,
    ) -> impl std::future::Future<Output = Result<Vec<EpisodeAction>>> + Send;
}

// ---------------------------------------------------------------------------
// Podcasts
// ---------------------------------------------------------------------------

pub trait PodcastRepo: Send + Sync {
    fn get_or_create_for_url(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = Result<(Podcast, bool)>> + Send;
    fn add_url(
        &self,
        podcast_id: Uuid,
        url: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn find_by_url(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = Result<Option<Podcast>>> + Send;
    fn find_by_id(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<Podcast>>> + Send;
    fn update(&self, podcast: &Podcast) -> impl std::future::Future<Output = Result<()>> + Send;
    fn delete(&self, id: Uuid) -> impl std::future::Future<Output = Result<()>> + Send;
    fn toplist(
        &self,
        count: i64,
        language: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Vec<Podcast>>> + Send;
    fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Podcast>>> + Send;
}

// ---------------------------------------------------------------------------
// Episodes
// ---------------------------------------------------------------------------

pub trait EpisodeRepo: Send + Sync {
    fn get_or_create_for_url(
        &self,
        podcast_id: Uuid,
        url: &str,
    ) -> impl std::future::Future<Output = Result<(Episode, bool)>> + Send;
    /// Find which podcast owns an episode with the given media URL.
    fn find_podcast_id_by_episode_url(
        &self,
        url: &str,
    ) -> impl std::future::Future<Output = Result<Option<Uuid>>> + Send;
    fn find_by_id(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<Episode>>> + Send;
    fn list_for_podcast(
        &self,
        podcast_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Episode>>> + Send;
    fn update(&self, episode: &Episode) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

pub trait SessionRepo: Send + Sync {
    fn create(&self, session: &Session) -> impl std::future::Future<Output = Result<()>> + Send;
    fn find_by_token(
        &self,
        token: &str,
    ) -> impl std::future::Future<Output = Result<Option<Session>>> + Send;
    fn delete(&self, token: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    fn delete_expired(&self) -> impl std::future::Future<Output = Result<u64>> + Send;
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

pub trait SettingsRepo: Send + Sync {
    fn get(
        &self,
        user_id: Uuid,
        scope: SettingsScope,
        scope_id: Option<Uuid>,
    ) -> impl std::future::Future<Output = Result<Option<UserSettings>>> + Send;
    fn save(&self, settings: &UserSettings)
    -> impl std::future::Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Sync Groups
// ---------------------------------------------------------------------------

pub trait SyncGroupRepo: Send + Sync {
    fn create_group(
        &self,
        user_id: Uuid,
        device_ids: &[Uuid],
        name: &str,
    ) -> impl std::future::Future<Output = Result<SyncGroup>> + Send;
    fn rename_group(
        &self,
        group_id: Uuid,
        user_id: Uuid,
        name: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_groups_for_user(
        &self,
        user_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<(SyncGroup, Vec<Device>)>>> + Send;
    fn remove_device_from_group(
        &self,
        device_id: Uuid,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

pub trait TagRepo: Send + Sync {
    fn top_tags(
        &self,
        count: i64,
    ) -> impl std::future::Future<Output = Result<Vec<(String, i64)>>> + Send;
    fn podcasts_for_tag(
        &self,
        tag: &str,
        count: i64,
    ) -> impl std::future::Future<Output = Result<Vec<Podcast>>> + Send;
    fn set_tags_for_podcast(
        &self,
        podcast_id: Uuid,
        tags: &[Tag],
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Podcast Lists
// ---------------------------------------------------------------------------

pub trait PodcastListRepo: Send + Sync {
    fn create(&self, list: &PodcastList) -> impl std::future::Future<Output = Result<()>> + Send;
    fn find_by_slug(
        &self,
        user_id: Uuid,
        slug: &str,
    ) -> impl std::future::Future<Output = Result<Option<PodcastList>>> + Send;
    fn list_for_user(
        &self,
        user_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<PodcastList>>> + Send;
    fn set_entries(
        &self,
        list_id: Uuid,
        podcast_ids: &[Uuid],
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn get_entries(
        &self,
        list_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<Podcast>>> + Send;
    fn delete(&self, list_id: Uuid) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Chapters
// ---------------------------------------------------------------------------

pub trait ChapterRepo: Send + Sync {
    fn upsert(&self, chapter: &Chapter) -> impl std::future::Future<Output = Result<()>> + Send;
    fn list_for_episode(
        &self,
        user_id: Uuid,
        episode_id: Uuid,
        since: Option<DateTime<Utc>>,
    ) -> impl std::future::Future<Output = Result<Vec<Chapter>>> + Send;
    fn delete(
        &self,
        user_id: Uuid,
        episode_id: Uuid,
        start_sec: i32,
        end_sec: i32,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Favorites
// ---------------------------------------------------------------------------

pub trait FavoriteRepo: Send + Sync {
    fn add(
        &self,
        user_id: Uuid,
        episode_id: Uuid,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn remove(
        &self,
        user_id: Uuid,
        episode_id: Uuid,
    ) -> impl std::future::Future<Output = Result<()>> + Send;
    fn list_for_user(
        &self,
        user_id: Uuid,
    ) -> impl std::future::Future<Output = Result<Vec<Episode>>> + Send;
}
