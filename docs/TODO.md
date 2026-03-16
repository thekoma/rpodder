# rpodder — TODO & Roadmap

## Legend
- `[ ]` — Not started
- `[~]` — In progress
- `[x]` — Done

---

## Phase 0 — Project Bootstrapping
- [x] Analyze mygpo API surface and data models
- [x] Choose tech stack (Rust, axum, sqlx, PostgreSQL+SQLite)
- [x] Create Cargo workspace structure
- [x] Define domain types (`rpodder-core/src/types.rs`)
- [x] Define repository traits (`rpodder-core/src/repo.rs`)
- [x] Create contextual documentation (CLAUDE.md, ARCHITECTURE.md, API_REFERENCE.md, TODO.md)
- [x] Docker setup (Dockerfile multi-stage, Dockerfile.dev with cargo-watch)
- [x] docker-compose.yml (PostgreSQL + dev server + release/sqlite profiles)
- [x] Skaffold config (continuous build with docker-compose deployer)
- [x] Initialize git repository
- [x] Verify `cargo build` and `cargo test` pass
- [ ] Add CI (GitHub Actions: build, test, clippy, fmt)

## Phase 1 — Core Sync (MVP)

Goal: a working server that AntennaPod or gPodder can connect to and sync subscriptions + episode actions.

### 1.1 Database Setup
- [x] Write PostgreSQL migrations (users, devices, podcasts, podcast_urls, episodes, episode_urls, subscriptions, subscription_changes, episode_actions, sessions)
- [x] Write SQLite migrations (same schema, SQLite-compatible types)
- [x] Implement DB pool initialization in `rpodder-db` (detect PG vs SQLite from config)
- [x] Implement `UserRepo` for PostgreSQL
- [x] Implement `UserRepo` for SQLite

### 1.2 Authentication
- [x] Implement `SessionRepo` for both backends
- [x] axum middleware: extract HTTP Basic Auth header
- [x] axum middleware: extract session cookie
- [x] axum middleware: verify username in URL matches authenticated user
- [x] Password hashing with argon2 (registration / CLI user creation)
- [x] `POST /api/2/auth/{username}/login.json` handler
- [x] `POST /api/2/auth/{username}/logout.json` handler

### 1.3 Devices
- [x] Implement `DeviceRepo` for both backends
- [x] `POST /api/2/devices/{username}/{deviceid}.json` handler (create/update)
- [x] `GET /api/2/devices/{username}.json` handler (list)

### 1.4 Subscriptions
- [x] Implement `PodcastRepo.get_or_create_for_url` for both backends
- [x] Implement `SubscriptionRepo` for both backends
- [x] Simple API: `GET /subscriptions/{user}/{device}.json` (get current list)
- [x] Simple API: `PUT /subscriptions/{user}/{device}.json` (replace list)
- [x] Simple API: `GET /subscriptions/{user}.json` (all user subscriptions)
- [x] OPML format support for subscriptions (import/export)
- [x] TXT format support for subscriptions
- [x] Advanced API: `POST /api/2/subscriptions/{user}/{device}.json` (delta upload)
- [x] Advanced API: `GET /api/2/subscriptions/{user}/{device}.json?since=T` (delta download)
- [x] URL normalization (strip trailing slashes, force https, etc.)

### 1.5 Episode Actions
- [x] Implement `EpisodeRepo.get_or_create_for_url` for both backends
- [x] Implement `EpisodeActionRepo` for both backends
- [x] `POST /api/2/episodes/{username}.json` handler (upload actions)
- [x] `GET /api/2/episodes/{username}.json` handler (download actions with since/device/podcast filters)
- [x] Episode action deduplication
- [x] Validate play action fields (started, position, total)

### 1.6 Server Bootstrap
- [x] Configuration loading (env vars + config file)
- [x] CLI with clap (serve, user create, user delete, migrate)
- [x] axum router with all Phase 1 routes
- [x] CORS middleware
- [x] Request/response logging (tower-http tracing)
- [x] Graceful shutdown
- [x] Docker build (multi-stage, static musl binary)

### 1.7 Testing
- [x] Integration tests: auth flow (login, use session, logout)
- [x] Integration tests: subscription CRUD + delta sync
- [x] Integration tests: episode action upload + download
- [x] Integration tests: device CRUD
- [ ] Test with AntennaPod against running instance
- [ ] Test with gPodder desktop against running instance

## Phase 2 — Directory & Feed Management

Goal: the server fetches and indexes podcast feeds, enabling search and discovery.

### 2.1 Feed Fetching
- [x] HTTP client with conditional GET (ETag, Last-Modified, If-None-Match)
- [x] RSS/Atom parsing via feed-rs → Podcast + Episode domain types
- [x] Update podcast metadata from feed (title, description, logo, language, author)
- [x] Create/update episodes from feed entries
- [x] Handle feed URL redirects (update canonical URL)
- [ ] Rate limiting per host
- [ ] Retry with exponential backoff
- [ ] Adaptive update interval (faster for active feeds, slower for stale)

### 2.2 Background Scheduler
- [x] Periodic feed update task (tokio spawn, configurable interval)
- [x] Priority queue: podcasts with more subscribers update first
- [x] Track last update time and next scheduled update
- [ ] Manual trigger endpoint (admin only)

### 2.3 Search & Directory API
- [x] PostgreSQL full-text search index on podcasts (title, description, author)
- [x] SQLite FTS5 index on podcasts
- [x] `GET /search.json?q=query` handler
- [x] `GET /toplist/{count}.json` handler (sorted by subscriber count)
- [x] `GET /api/2/data/podcast.json?url=X` handler (podcast info)
- [x] `GET /api/2/data/episode.json?podcast=X&url=Y` handler (episode info)

### 2.4 Tags
- [x] Implement `TagRepo` for both backends
- [x] Extract tags from feed categories during parsing
- [x] `GET /api/2/tags/{count}.json` handler
- [x] `GET /api/2/tag/{tag}/{count}.json` handler

### 2.5 Suggestions
- [x] Basic suggestion algorithm (popular podcasts in same categories as user's subscriptions)
- [x] `GET /suggestions/{count}.json` handler

## Phase 3 — Advanced Features

### 3.1 Device Synchronization
- [x] Implement `SyncGroupRepo` for both backends
- [x] `GET /api/2/sync-devices/{username}.json` handler
- [x] `POST /api/2/sync-devices/{username}.json` handler
- [ ] Propagate subscriptions across synced devices
- [ ] `GET /api/2/updates/{username}/{deviceid}.json` handler (combined updates)

### 3.2 Settings API
- [x] Implement `SettingsRepo` for both backends
- [x] `GET /api/2/settings/{username}/{scope}.json` handler
- [x] `POST /api/2/settings/{username}/{scope}.json` handler
- [x] Scope resolution: account / device / podcast / episode

### 3.3 Podcast Lists
- [x] Implement `PodcastListRepo` for both backends
- [ ] CRUD handlers for `/api/2/lists/...`

### 3.4 Chapters
- [x] Implement `ChapterRepo` for both backends
- [ ] `GET /api/2/chapters/{username}.json` handler
- [ ] `POST /api/2/chapters/{username}.json` handler

### 3.5 Favorites
- [x] Implement `FavoriteRepo` for both backends
- [x] `GET /api/2/favorites/{username}.json` handler

## Phase 4 — Production Readiness

- [ ] Rate limiting per user/IP
- [ ] Prometheus metrics endpoint
- [ ] Health check endpoint
- [ ] Admin API (user management, feed forcing, stats)
- [ ] TLS termination docs (or built-in via rustls)
- [ ] Systemd service file
- [ ] Helm chart / docker-compose.yml
- [ ] README with quickstart
- [ ] Automated release builds (cross-compile for linux/amd64, arm64, armv7)
- [ ] Performance benchmarks

## Deferred / Maybe

- [ ] WebSub (PubSubHubbub) for real-time feed updates
- [ ] Web UI (settings, subscription management)
- [ ] ActivityPub / fediverse integration (gpodder2go stretch goal)
- [ ] Podcast grouping (multiple feeds for same show, e.g. audio vs video)
- [ ] User registration endpoint (currently CLI-only)
- [ ] OAuth / OIDC authentication
- [ ] Import/export from mygpo database
