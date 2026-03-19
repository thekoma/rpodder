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
- [x] Add CI (GitHub Actions: build, test, clippy, fmt)

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
- [x] `DELETE /api/2/devices/{username}/{deviceid}.json` handler (delete)
- [x] Auto-create devices on first subscription sync (gpodder.net compat)
- [x] Real subscriber count per device

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
- [x] URL normalization (strip trailing slashes, lowercase host, strip fragments)

### 1.5 Episode Actions
- [x] Implement `EpisodeRepo.get_or_create_for_url` for both backends
- [x] Implement `EpisodeActionRepo` for both backends
- [x] `POST /api/2/episodes/{username}.json` handler (upload actions)
- [x] `GET /api/2/episodes/{username}.json` handler (download actions with since/device/podcast filters)
- [x] Episode action deduplication
- [x] Validate play action fields (started, position, total)

### 1.6 Server Bootstrap
- [x] Configuration loading (env vars + TOML config file)
- [x] CLI with clap (serve, user create, user delete, migrate, repair)
- [x] axum router with all Phase 1 routes
- [x] CORS middleware
- [x] Request/response logging (tower-http tracing)
- [x] Graceful shutdown (SIGTERM + Ctrl+C)
- [x] Docker build (multi-stage with bun frontend + Rust backend)
- [x] Idempotent migrations (IF NOT EXISTS)
- [x] SQLite FTS5 repair command

### 1.7 Testing
- [x] Integration tests: auth flow (login, use session, logout)
- [x] Integration tests: subscription CRUD + delta sync
- [x] Integration tests: episode action upload + download
- [x] Integration tests: device CRUD
- [x] Test with Kasts (KDE) against running instance
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
- [x] Resilient feed updater (best-effort per operation, no crash on FTS5 errors)
- [x] On-demand feed fetch when opening podcast detail page
- [x] Privacy: skip private feeds with access tokens in URL
- [x] Rate limiting per host (implicit via adaptive intervals + retry backoff)
- [x] Retry with exponential backoff (3 attempts, 500ms/1s/2s)
- [x] Adaptive update interval (1h popular → 7d inactive)

### 2.2 Background Scheduler
- [x] Periodic feed update task (tokio spawn, 30min interval)
- [x] Priority queue: podcasts with more subscribers update first
- [x] Track last update time and next scheduled update
- [x] Manual trigger endpoint (admin only)

### 2.3 Search & Directory API
- [x] PostgreSQL full-text search index on podcasts (title, description, author)
- [x] SQLite FTS5 index on podcasts
- [x] Fuzzy prefix search ("pil" finds "Pillole di Bit")
- [x] LIKE fallback when FTS5 returns no results
- [x] `GET /search.json?q=query` handler
- [x] `GET /toplist/{count}.json` handler (sorted by subscriber count)
- [x] `GET /api/2/data/podcast.json?url=X` handler (podcast info)
- [x] `GET /api/2/data/podcast/episodes.json?url=X` handler (podcast episodes, paginated)
- [x] `GET /api/2/data/episode.json?podcast=X&url=Y` handler (episode info)
- [x] Real subscriber counts (updated on subscribe/unsubscribe)

### 2.4 Tags
- [x] Implement `TagRepo` for both backends
- [x] Extract tags from feed categories during parsing
- [x] `GET /api/2/tags/{count}.json` handler
- [x] `GET /api/2/tag/{tag}/{count}.json` handler (DISTINCT, no duplicates)

### 2.5 Suggestions
- [x] Basic suggestion algorithm (popular podcasts in same categories as user's subscriptions)
- [x] `GET /suggestions/{count}.json` handler

## Phase 3 — Advanced Features

### 3.1 Device Synchronization
- [x] Implement `SyncGroupRepo` for both backends
- [x] `GET /api/2/sync-devices/{username}.json` handler
- [x] `POST /api/2/sync-devices/{username}.json` handler
- [x] Propagate subscriptions across synced devices
- [x] `GET /api/2/updates/{username}/{deviceid}.json` handler (combined updates)

### 3.2 Settings API
- [x] Implement `SettingsRepo` for both backends
- [x] `GET /api/2/settings/{username}/{scope}.json` handler
- [x] `POST /api/2/settings/{username}/{scope}.json` handler
- [x] Scope resolution: account / device / podcast / episode

### 3.3 Podcast Lists
- [x] Implement `PodcastListRepo` for both backends
- [x] CRUD handlers for `/api/2/lists/...`

### 3.4 Chapters
- [x] Implement `ChapterRepo` for both backends
- [x] `GET /api/2/chapters/{username}.json` handler
- [x] `POST /api/2/chapters/{username}.json` handler

### 3.5 Favorites
- [x] Implement `FavoriteRepo` for both backends
- [x] `GET /api/2/favorites/{username}.json` handler

## Phase 4 — Production Readiness

- [x] Rate limiting (concurrency limit, 200 max concurrent requests)
- [x] Prometheus metrics endpoint (GET /metrics)
- [x] Health check endpoint (`GET /health`)
- [x] Admin API (user management, feed forcing, stats, password management)
- [x] Dynamic migration system (reads `*.up.sql` from directory, no hardcoded filenames)
- [x] Dockerfile: mold linker for faster release builds
- [ ] TLS termination docs (or built-in via rustls)
- [x] Systemd service file
- [x] docker-compose.yml with profiles (dev/release/sqlite)
- [x] README with quickstart
- [x] Add CI (GitHub Actions: build, test, clippy, fmt)
- [ ] Automated release builds (cross-compile for linux/amd64, arm64, armv7)
- [ ] Performance benchmarks

## Phase 5 — Web UI (Svelte + Tailwind)

Goal: browser-based UI for managing subscriptions, discovering podcasts, and admin.

### 5.1 Core UI
- [x] Svelte 5 + Tailwind v4 project scaffold (`web/`)
- [x] SPA with adapter-static, builds to `web/dist/`
- [x] Embed in Rust binary via `rust-embed` (feature flag `web-ui`)
- [x] Fallback handler for SPA client-side routing
- [x] API client (`web/src/lib/api.ts`)
- [x] Auth state management with Svelte 5 runes (`$effect` pattern)
- [x] Login page
- [x] Home page with server status + popular podcasts (clickable)

### 5.2 Discover Section
- [x] Tab navigation (Browse / Top Podcasts / Trending)
- [x] Browse page: always-visible search bar, categories with podcast lists, tag cloud as pills
- [x] Toplist page: ranked table with logo, subscriber bar
- [x] Tag pages: `/discover/tag/{tag}` with subscribe button
- [x] Search: fuzzy prefix, debounce, combined local + Podcast Index results
- [x] Podcast detail page: logo, title, author, description, subscribe/unsubscribe toggle
- [x] Episode list: paginated with title, description, date, duration
- [x] On-demand feed fetch for unindexed podcasts
- [x] All podcasts clickable everywhere (directory, toplist, tags, search, home)
- [x] SPA navigation fix: reactive URL tracking with `$page` store
- [x] HTTP/HTTPS dedup in directory listings (prefer HTTPS, sum subscribers)

### 5.3 Subscriptions & Devices
- [x] Subscriptions page: grid with logo/title/author from feed metadata
- [x] Clickable subscriptions → podcast detail with episodes
- [x] Unsubscribe button (appears on hover)
- [x] Devices page: real subscription count per device
- [x] Device rename (inline editing)
- [x] Device delete with confirmation
- [x] HTTPS upgrade suggestions: banner with per-subscription and "upgrade all" buttons

### 5.4 Privacy & Security
- [x] Private feed detection (token URLs hidden from public directory)
- [x] Private feeds still visible in user's own subscriptions
- [x] Feed updater skips private feeds (no token leaks via HTTP/logs)

### 5.5 Admin
- [x] Server status page (`/admin`) with user/device/subscription stats
- [x] User roles: admin/user with `is_admin` field + DB migration
- [x] Admin middleware (`require_admin_layer`) protects `/api/admin/*` routes
- [x] Admin panel: full user management (list, create, activate/deactivate, set role, delete)
- [x] First user registered becomes admin automatically
- [x] Registration bypass: always open if no active users exist
- [x] SSO group-based admin mapping (`RPODDER_OAUTH_ADMIN_GROUP`)
- [x] CLI `--admin` flag for `rpodder user create`
- [x] `GET /api/2/me` endpoint for current user info (including `is_admin`)
- [x] Web UI: Admin link/badge visible only for admins
- [x] Admin panel: stats dashboard (users, devices, subscriptions, podcasts, episode actions)
- [x] Admin panel: set/reset password per user
- [x] Admin panel: send password reset email

### 5.6 Password Management
- [x] `POST /api/2/me/password` — change own password (SSO-friendly: old_password optional)
- [x] `POST /api/admin/users/{username}/password` — admin sets password directly
- [x] `POST /api/admin/users/{username}/reset-password` — admin triggers reset email
- [x] `POST /api/2/password-reset` — self-service request (anti-enumeration)
- [x] `POST /api/2/password-reset/confirm` — confirm with token + new password
- [x] Settings page (`/settings`) with profile info and change password form
- [x] Password reset page (`/reset-password`) with dual-mode (request/confirm)
- [x] "Forgot password?" link on login page

### 5.7 Podcast Index Integration
- [x] Podcast Index API client (`podcast_index.rs`) with HMAC-SHA1 auth
- [x] `GET /api/2/search/all` — combined local + Podcast Index search (parallel, dedup)
- [x] `GET /api/2/search/external` — Podcast Index only
- [x] `GET /api/2/trending?lang=it&max=20` — trending podcasts with language filter
- [x] Trending page with language filter pills (All/EN/IT/DE/ES/FR)
- [x] Search UI: separate "Local results" and "Podcast Index" sections
- [x] Subscribe to external podcasts directly (rpodder fetches and indexes the feed)

### 5.8 Previously Completed
- [x] Episode actions history page
- [x] Admin panel: feed management (force update, view status)
- [x] OAuth / OIDC authentication (SSO via generic OIDC — Authentik, Keycloak, etc.)
- [x] User registration endpoint (web form)
- [x] Dockerfile for standalone Svelte container (nginx)
- [x] docker-compose profile for separate frontend

## Deferred / Maybe

- [ ] WebSub (PubSubHubbub) for real-time feed updates
- [ ] ActivityPub / fediverse integration (gpodder2go stretch goal)
- [ ] Podcast grouping (multiple feeds for same show, e.g. audio vs video)
- [ ] Import/export from mygpo database
- [x] Rate limiting per host in feed fetcher (implicit via retry backoff)
- [x] Retry with exponential backoff (3 attempts, 500ms/1s/2s) in feed fetcher
