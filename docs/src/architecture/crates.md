# Crate Structure

rpodder is organized as a Cargo workspace with four crates. Dependencies flow one way: server → db → core, and server → feed.

```
rpodder-server (binary)
  ├── rpodder-core (domain types, traits, errors)
  ├── rpodder-db   (PostgreSQL + SQLite implementations)
  └── rpodder-feed (HTTP client + RSS parser)
```

## rpodder-core

**Domain types, repository traits, error types, and utilities.**

No external dependencies beyond `chrono`, `uuid`, `serde`, `url`, `thiserror`. This crate defines the "what" without the "how".

### Key files

- `types.rs` — all domain structs: `User`, `Device`, `Podcast`, `Episode`, `Subscription`, `EpisodeAction`, `Session`, `SyncGroup`, `Setting`, `PodcastList`, `Chapter`, `Favorite`
- `repo.rs` — 13 async traits: `UserRepo`, `SessionRepo`, `DeviceRepo`, `PodcastRepo`, `EpisodeRepo`, `SubscriptionRepo`, `EpisodeActionRepo`, `SyncGroupRepo`, `TagRepo`, `SettingsRepo`, `PodcastListRepo`, `ChapterRepo`, `FavoriteRepo`
- `error.rs` — `AppError` enum with `NotFound`, `Conflict`, `Internal` variants
- `url.rs` — URL normalization (lowercase host, strip trailing slashes)
- `privacy.rs` — heuristic detection of private/token URLs

## rpodder-db

**Database implementations for PostgreSQL and SQLite.**

Implements all 13 repository traits from `rpodder-core` for both backends. Uses `sqlx` with runtime queries (no compile-time macros).

### Key files

- `lib.rs` — `Db` enum, connection logic, dynamic migration runner
- `postgres.rs` — `PgRepo` struct implementing all traits
- `sqlite.rs` — `SqliteRepo` struct implementing all traits, plus unit tests

### Pattern

Each repo implementation follows the same pattern:

```rust
impl repo::UserRepo for PgRepo {
    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, ... FROM users WHERE LOWER(username) = LOWER($1)",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(Into::into))
    }
}
```

- `UserRow` is a `#[derive(FromRow)]` struct matching the SQL columns
- `From<UserRow> for User` converts from the DB row to the domain type
- PostgreSQL uses `$1, $2` placeholders; SQLite uses `?`

## rpodder-feed

**Feed fetching and parsing.**

- `FeedFetcher` — HTTP client with conditional GET (ETag, If-Modified-Since), retry with exponential backoff
- Uses `feed-rs` for RSS/Atom parsing
- Extracts podcast metadata, episodes, and categories

## rpodder-server

**The binary crate — HTTP server, CLI, everything that ties it together.**

### Key modules

- `main.rs` — CLI (clap), server setup, route registration
- `config.rs` — `AppConfig` from env vars + TOML
- `state.rs` — `AppState` with `Arc<Db>` + `Arc<AppConfig>`
- `middleware/auth.rs` — authentication + admin middleware
- `email.rs` — SMTP sender for activation and password reset emails
- `podcast_index.rs` — Podcast Index API client
- `feed_updater.rs` — background feed update loop
- `web_ui.rs` — rust-embed handler for serving the SPA
- `routes/` — all HTTP handlers organized by domain

### Route modules

| Module | Endpoints |
|--------|-----------|
| `auth.rs` | Login, logout |
| `admin.rs` | User management, stats, password, HTTPS upgrades, history, /me |
| `registration.rs` | Public registration |
| `oauth.rs` | SSO login, callback, info |
| `devices.rs` | Device CRUD |
| `subscriptions.rs` | Subscription sync (simple + advanced) |
| `episodes.rs` | Episode action upload/download |
| `directory.rs` | Search, toplist, tags, trending, podcast/episode data |
| `sync.rs` | Sync group management |
| `settings.rs` | User settings |
| `lists.rs` | Podcast lists |
| `chapters.rs` | Chapter marks |
| `favorites.rs` | Favorites |
| `health.rs` | Health check, metrics |

## Web UI (web/)

The Svelte 5 SPA lives in `web/` and is built separately:

```
web/
  src/
    lib/
      api.ts          — TypeScript API client
      auth.svelte.ts  — reactive auth state (runes)
    routes/
      +layout.svelte  — main layout with nav
      discover/       — browse, toplist, trending, tags, podcast detail
      subscriptions/  — user's subscriptions with HTTPS upgrades
      admin/          — admin panel
      settings/       — user settings + password change
      reset-password/ — password reset flow
      login/          — login page
      register/       — registration page
      history/        — episode action history
      devices/        — device management
```

Built with `bun run build` → `web/dist/`, embedded via `rust-embed`.
