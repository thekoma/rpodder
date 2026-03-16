# Architecture

## Overview

rpodder is a gpodder.net-compatible podcast sync server written in Rust. It aims for feature parity with [mygpo](https://github.com/gpodder/mygpo) while being modern, fast, and easy to self-host.

## Crate Dependency Graph

```
rpodder-server
  ├── rpodder-core   (domain types, repo traits)
  ├── rpodder-db     (sqlx repo implementations)
  │     └── rpodder-core
  └── rpodder-feed   (feed fetching/parsing)
        └── rpodder-core
```

## rpodder-core

Zero infrastructure dependencies. Contains:

- **`types.rs`** — All domain structs: Podcast, Episode, User, Device, Subscription, SubscriptionChange, EpisodeAction, UserSettings, SyncGroup, Tag, PodcastList, Chapter, FavoriteEpisode, Category, Session.
- **`repo.rs`** — Async repository traits (one per aggregate root): UserRepo, DeviceRepo, SubscriptionRepo, EpisodeActionRepo, PodcastRepo, EpisodeRepo, SessionRepo, SettingsRepo, SyncGroupRepo, TagRepo, PodcastListRepo, ChapterRepo, FavoriteRepo.
- **`error.rs`** — AppError enum covering NotFound, Unauthorized, BadRequest, Conflict, Internal.

All types use `Uuid` (v7) for IDs and `chrono::DateTime<Utc>` for timestamps.

## rpodder-db

Implements repo traits for PostgreSQL and SQLite via sqlx. Each backend has its own migration files in `migrations/{postgresql,sqlite}/`.

Key considerations:
- PostgreSQL uses `tsvector` for full-text search. SQLite uses FTS5.
- Subscription change history is append-only (no updates, no deletes).
- Episode actions are append-only with deduplication.

## rpodder-feed

Handles:
- Fetching podcast RSS/Atom feeds with conditional GET (ETag, Last-Modified).
- Parsing via feed-rs into rpodder-core types.
- Scheduling periodic updates based on `update_interval_hours`.
- Respecting rate limits and retries.

## rpodder-server

axum-based HTTP server. Structure:

```
src/
  main.rs          CLI (clap) + server bootstrap
  routes/          Route handler modules per API area
    auth.rs        POST login/logout
    subscriptions.rs  Simple + Advanced subscriptions API
    episodes.rs    Episode actions API
    devices.rs     Device CRUD + sync
    directory.rs   Search, toplist, tags, podcast/episode info
    settings.rs    Settings API
    lists.rs       Podcast lists API
    chapters.rs    Chapters API
    favorites.rs   Favorites API
  middleware/
    auth.rs        HTTP Basic Auth + session token extraction
  state.rs         App state (holds DB pool, config)
  config.rs        Configuration loading
```

## Authentication Flow

gpodder clients use HTTP Basic Auth. The flow:

1. Client sends `POST /api/2/auth/{username}/login.json` with Basic Auth header
2. Server validates credentials, creates a session, returns session cookie
3. Subsequent requests use either the session cookie OR Basic Auth
4. `POST /api/2/auth/{username}/logout.json` destroys the session

## Data Model Notes

### Subscription Sync (delta protocol)

The Advanced Subscriptions API uses a delta sync protocol:
- Client sends `{add: [...urls], remove: [...urls]}`
- Server responds with `{timestamp: N}`
- On next sync, client sends `since=N` to get changes since that timestamp
- Server responds with `{add: [...], remove: [...], timestamp: M}`

This requires maintaining a `subscription_changes` history table alongside the current `subscriptions` table.

### Episode Actions

Episode actions are events reported by clients:
- `download` — episode was downloaded
- `play` — episode was played (includes `started`, `position`, `total` seconds)
- `delete` — episode was deleted
- `new` — episode was marked as new

Actions are append-only. The `play` action is the most important — it enables cross-device playback position sync. Deduplication prevents storing the same action twice.

### Device Sync

Devices can be grouped for synchronization. When devices are synced, subscriptions on one device propagate to all devices in the group. The sync status API returns `{synchronized: [[dev1, dev2], [dev3, dev4]], not-synchronized: [dev5]}`.

### Multi-URL Support

Both podcasts and episodes can have multiple URLs (due to feed URL changes, redirects, etc.). The URL with `order=0` is canonical. When a client references a URL, the server normalizes it and looks up across all known URLs.

## Development Environment

### Docker Setup

Two Dockerfiles serve different purposes:

- **`Dockerfile`** — Production multi-stage build. Stage 1 compiles with `rust:1.86-bookworm`, caching dependency builds by copying `Cargo.toml`/`Cargo.lock` first with dummy sources. Stage 2 copies only the release binary into `debian:bookworm-slim` (~80MB final image).
- **`Dockerfile.dev`** — Dev container with `cargo-watch`. Source is bind-mounted (not copied), so changes on the host trigger automatic recompilation.

### docker-compose.yml

Services and profiles:

| Service | Profile | Purpose |
|---------|---------|---------|
| `postgres` | (default) | PostgreSQL 17, port 5432, healthchecked |
| `rpodder` | (default) | Dev server with cargo-watch, live reload, port 3005 |
| `rpodder-release` | `release` | Production image, port 3006 |
| `rpodder-sqlite` | `sqlite` | Self-hosted mode with SQLite volume, port 3007 |

Named volumes:
- `pgdata` — PostgreSQL data (persistent)
- `cargo-cache` — Cargo registry cache (speeds up rebuilds across container restarts)
- `target-cache` — Rust build cache (avoids recompiling everything on restart)
- `sqlite-data` — SQLite database file

### Skaffold

Skaffold provides a continuous build loop for development. It watches source files and triggers Docker rebuilds + redeploy via docker-compose. No Kubernetes cluster is needed — it uses the `docker` deployer with `useCompose: true`.

- `skaffold dev` — default profile, uses `Dockerfile.dev`, file sync for `.rs` files
- `skaffold dev -p release` — builds production `Dockerfile`, full rebuild on changes
- `skaffold build` — one-shot image build
- `skaffold run` / `skaffold delete` — deploy / tear down

The `sync.manual` block allows Skaffold to copy changed `.rs` files into the running container without a full image rebuild, where cargo-watch picks them up.
