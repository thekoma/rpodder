use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Podcast
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Podcast {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub link: Option<String>,
    pub language: Option<String>,
    pub logo_url: Option<String>,
    pub author: Option<String>,
    pub subscribers: i64,
    pub episode_count: i64,
    pub last_update: Option<DateTime<Utc>>,
    pub update_interval_hours: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A podcast can have multiple URLs (redirects, migrations).
/// The first (order=0) is canonical.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastUrl {
    pub id: Uuid,
    pub podcast_id: Uuid,
    pub url: String,
    pub order: i32,
}

// ---------------------------------------------------------------------------
// Episode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: Uuid,
    pub podcast_id: Uuid,
    pub guid: Option<String>,
    pub title: String,
    pub description: String,
    pub link: Option<String>,
    pub released: Option<DateTime<Utc>>,
    pub duration: Option<i64>,
    pub filesize: Option<i64>,
    pub mimetype: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeUrl {
    pub id: Uuid,
    pub episode_id: Uuid,
    pub url: String,
    pub order: i32,
}

// ---------------------------------------------------------------------------
// User
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    /// argon2 hash — never serialized out
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub email: Option<String>,
    pub is_active: bool,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Device (called "Client" in mygpo, "Device" in the API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Desktop,
    Laptop,
    Mobile,
    Server,
    Tablet,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: Uuid,
    pub user_id: Uuid,
    /// User-assigned unique identifier (e.g. "my-phone")
    pub device_id: String,
    pub caption: String,
    pub device_type: DeviceType,
    pub sync_group_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncGroup {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Subscription
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub podcast_id: Uuid,
    /// The URL the client used when subscribing (may differ from canonical)
    pub ref_url: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionAction {
    Subscribe,
    Unsubscribe,
}

/// History of subscription changes, used for delta sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionChange {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub podcast_id: Uuid,
    pub action: SubscriptionAction,
    pub ref_url: String,
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Episode Action
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EpisodeActionType {
    Download,
    Play,
    Delete,
    New,
}

/// An action on an episode, reported by a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeAction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_id: Option<Uuid>,
    pub episode_id: Uuid,
    pub action: EpisodeActionType,
    /// URL the client used to reference the podcast
    pub podcast_ref_url: Option<String>,
    /// URL the client used to reference the episode
    pub episode_ref_url: Option<String>,
    /// Playback position fields (only for Play actions)
    pub started: Option<i32>,
    pub position: Option<i32>,
    pub total: Option<i32>,
    /// Timestamp provided by the client
    pub timestamp: DateTime<Utc>,
    /// Timestamp recorded by the server
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Settings (key-value per scope)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SettingsScope {
    Account,
    Device,
    Podcast,
    Episode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub id: Uuid,
    pub user_id: Uuid,
    pub scope: SettingsScope,
    /// The scoped object id (device_id, podcast_id, or episode_id). None for Account scope.
    pub scope_id: Option<Uuid>,
    /// JSON blob of settings key-value pairs
    pub settings: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tag
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TagSource {
    Feed,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: Uuid,
    pub tag: String,
    pub source: TagSource,
    pub user_id: Option<Uuid>,
    pub podcast_id: Uuid,
}

// ---------------------------------------------------------------------------
// Podcast List
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastList {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastListEntry {
    pub id: Uuid,
    pub list_id: Uuid,
    pub podcast_id: Uuid,
    pub order: i32,
}

// ---------------------------------------------------------------------------
// Chapter (user-defined)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: Uuid,
    pub user_id: Uuid,
    pub episode_id: Uuid,
    pub start_sec: i32,
    pub end_sec: i32,
    pub label: String,
    pub advertisement: bool,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Favorite Episode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteEpisode {
    pub id: Uuid,
    pub user_id: Uuid,
    pub episode_id: Uuid,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Category
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: Uuid,
    pub tag: String,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Session (for auth)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
