-- rpodder initial schema for SQLite
-- All IDs are UUIDv7 stored as TEXT (36 chars with hyphens).
-- Timestamps stored as TEXT in ISO 8601 format.

-- =========================================================================
-- Users
-- =========================================================================

CREATE TABLE users (
    id              TEXT PRIMARY KEY NOT NULL,
    username        TEXT NOT NULL,
    password_hash   TEXT NOT NULL,
    email           TEXT,
    is_active       INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_users_username ON users (username COLLATE NOCASE);
CREATE UNIQUE INDEX idx_users_email ON users (email COLLATE NOCASE) WHERE email IS NOT NULL;

-- =========================================================================
-- Sessions
-- =========================================================================

CREATE TABLE sessions (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token       TEXT NOT NULL UNIQUE,
    expires_at  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_sessions_token ON sessions (token);
CREATE INDEX idx_sessions_expires ON sessions (expires_at);

-- =========================================================================
-- Sync Groups
-- =========================================================================

CREATE TABLE sync_groups (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- =========================================================================
-- Devices
-- =========================================================================

CREATE TABLE devices (
    id              TEXT PRIMARY KEY NOT NULL,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id       TEXT NOT NULL,
    caption         TEXT NOT NULL DEFAULT '',
    device_type     TEXT NOT NULL DEFAULT 'other' CHECK (device_type IN ('desktop','laptop','mobile','server','tablet','other')),
    sync_group_id   TEXT REFERENCES sync_groups(id) ON DELETE SET NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_devices_user_uid ON devices (user_id, device_id);

-- =========================================================================
-- Podcasts
-- =========================================================================

CREATE TABLE podcasts (
    id                      TEXT PRIMARY KEY NOT NULL,
    title                   TEXT NOT NULL DEFAULT '',
    description             TEXT NOT NULL DEFAULT '',
    link                    TEXT,
    language                TEXT,
    logo_url                TEXT,
    author                  TEXT,
    subscribers             INTEGER NOT NULL DEFAULT 0,
    episode_count           INTEGER NOT NULL DEFAULT 0,
    last_update             TEXT,
    update_interval_hours   INTEGER NOT NULL DEFAULT 168,
    created_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_podcasts_subscribers ON podcasts (subscribers DESC);
CREATE INDEX idx_podcasts_last_update ON podcasts (last_update);

-- =========================================================================
-- Podcast URLs
-- =========================================================================

CREATE TABLE podcast_urls (
    id          TEXT PRIMARY KEY NOT NULL,
    podcast_id  TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    url         TEXT NOT NULL UNIQUE,
    "order"     INTEGER NOT NULL DEFAULT 0,

    UNIQUE (podcast_id, "order")
);

CREATE INDEX idx_podcast_urls_podcast ON podcast_urls (podcast_id);

-- =========================================================================
-- Episodes
-- =========================================================================

CREATE TABLE episodes (
    id          TEXT PRIMARY KEY NOT NULL,
    podcast_id  TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    guid        TEXT,
    title       TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    link        TEXT,
    released    TEXT,
    duration    INTEGER,
    filesize    INTEGER,
    mimetype    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_episodes_podcast ON episodes (podcast_id);
CREATE INDEX idx_episodes_podcast_released ON episodes (podcast_id, released DESC);
CREATE INDEX idx_episodes_guid ON episodes (podcast_id, guid) WHERE guid IS NOT NULL;

-- =========================================================================
-- Episode URLs
-- =========================================================================

CREATE TABLE episode_urls (
    id          TEXT PRIMARY KEY NOT NULL,
    episode_id  TEXT NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    url         TEXT NOT NULL UNIQUE,
    "order"     INTEGER NOT NULL DEFAULT 0,

    UNIQUE (episode_id, "order")
);

CREATE INDEX idx_episode_urls_episode ON episode_urls (episode_id);

-- =========================================================================
-- Subscriptions
-- =========================================================================

CREATE TABLE subscriptions (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id   TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    podcast_id  TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    ref_url     TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    UNIQUE (user_id, device_id, podcast_id)
);

CREATE INDEX idx_subscriptions_user ON subscriptions (user_id);
CREATE INDEX idx_subscriptions_user_device ON subscriptions (user_id, device_id);
CREATE INDEX idx_subscriptions_podcast ON subscriptions (podcast_id);

-- =========================================================================
-- Subscription Changes (append-only)
-- =========================================================================

CREATE TABLE subscription_changes (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id   TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    podcast_id  TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    action      TEXT NOT NULL CHECK (action IN ('subscribe','unsubscribe')),
    ref_url     TEXT NOT NULL,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_subchanges_user_device_ts ON subscription_changes (user_id, device_id, timestamp);

-- =========================================================================
-- Episode Actions (append-only)
-- =========================================================================

CREATE TABLE episode_actions (
    id              TEXT PRIMARY KEY NOT NULL,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id       TEXT REFERENCES devices(id) ON DELETE SET NULL,
    episode_id      TEXT NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    action          TEXT NOT NULL CHECK (action IN ('download','play','delete','new')),
    podcast_ref_url TEXT,
    episode_ref_url TEXT,
    started         INTEGER,
    position        INTEGER,
    total           INTEGER,
    timestamp       TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_epactions_user_ts ON episode_actions (user_id, timestamp);
CREATE INDEX idx_epactions_user_device ON episode_actions (user_id, device_id, timestamp);
CREATE INDEX idx_epactions_user_episode ON episode_actions (user_id, episode_id);
-- Deduplication: same user+episode+device+action+timestamp should not exist twice
CREATE UNIQUE INDEX idx_epactions_dedup
    ON episode_actions (user_id, episode_id, COALESCE(device_id, ''), action, timestamp);

-- =========================================================================
-- User Settings
-- =========================================================================

CREATE TABLE user_settings (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope       TEXT NOT NULL CHECK (scope IN ('account','device','podcast','episode')),
    scope_id    TEXT,
    settings    TEXT NOT NULL DEFAULT '{}',  -- JSON stored as TEXT
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX idx_user_settings_scope ON user_settings (user_id, scope, COALESCE(scope_id, ''));

-- =========================================================================
-- Tags
-- =========================================================================

CREATE TABLE tags (
    id          TEXT PRIMARY KEY NOT NULL,
    tag         TEXT NOT NULL,
    source      TEXT NOT NULL CHECK (source IN ('feed','user')),
    user_id     TEXT REFERENCES users(id) ON DELETE CASCADE,
    podcast_id  TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,

    UNIQUE (tag, source, user_id, podcast_id)
);

CREATE INDEX idx_tags_podcast ON tags (podcast_id);
CREATE INDEX idx_tags_tag ON tags (tag);

-- =========================================================================
-- Podcast Lists
-- =========================================================================

CREATE TABLE podcast_lists (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    slug        TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    UNIQUE (user_id, slug)
);

CREATE TABLE podcast_list_entries (
    id          TEXT PRIMARY KEY NOT NULL,
    list_id     TEXT NOT NULL REFERENCES podcast_lists(id) ON DELETE CASCADE,
    podcast_id  TEXT NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    "order"     INTEGER NOT NULL DEFAULT 0,

    UNIQUE (list_id, podcast_id)
);

CREATE INDEX idx_plist_entries_list ON podcast_list_entries (list_id, "order");

-- =========================================================================
-- Chapters
-- =========================================================================

CREATE TABLE chapters (
    id              TEXT PRIMARY KEY NOT NULL,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    episode_id      TEXT NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    start_sec       INTEGER NOT NULL,
    end_sec         INTEGER NOT NULL,
    label           TEXT NOT NULL DEFAULT '',
    advertisement   INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_chapters_user_episode ON chapters (user_id, episode_id);

-- =========================================================================
-- Favorite Episodes
-- =========================================================================

CREATE TABLE favorite_episodes (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    episode_id  TEXT NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    UNIQUE (user_id, episode_id)
);

-- =========================================================================
-- Categories
-- =========================================================================

CREATE TABLE categories (
    id          TEXT PRIMARY KEY NOT NULL,
    tag         TEXT NOT NULL UNIQUE,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- =========================================================================
-- FTS5 virtual table for podcast search (SQLite-specific)
-- =========================================================================

CREATE VIRTUAL TABLE podcasts_fts USING fts5(
    title,
    author,
    description,
    content='podcasts',
    content_rowid='rowid'
);

-- Triggers to keep FTS index in sync with podcasts table
CREATE TRIGGER trg_podcasts_fts_insert AFTER INSERT ON podcasts BEGIN
    INSERT INTO podcasts_fts(rowid, title, author, description)
    VALUES (NEW.rowid, NEW.title, NEW.author, NEW.description);
END;

CREATE TRIGGER trg_podcasts_fts_update AFTER UPDATE OF title, author, description ON podcasts BEGIN
    DELETE FROM podcasts_fts WHERE rowid = OLD.rowid;
    INSERT INTO podcasts_fts(rowid, title, author, description)
    VALUES (NEW.rowid, NEW.title, NEW.author, NEW.description);
END;

CREATE TRIGGER trg_podcasts_fts_delete AFTER DELETE ON podcasts BEGIN
    DELETE FROM podcasts_fts WHERE rowid = OLD.rowid;
END;
