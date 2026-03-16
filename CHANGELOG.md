# Changelog

## v0.1.0 — 2026-03-17

Initial release. Full gpodder.net-compatible podcast sync server with web UI.

### Backend (Rust/axum)
- **Auth**: HTTP Basic Auth + session cookies, argon2 password hashing
- **Devices**: CRUD, auto-create on first sync, real subscriber counts
- **Subscriptions**: Simple API (GET/PUT) + Advanced delta API (POST/GET with since), JSON/OPML/TXT formats
- **Episode Actions**: upload/download with filters (since, device, podcast), deduplication, play validation
- **Directory**: search (FTS5/tsvector with prefix matching + LIKE fallback), toplist, tags, suggestions
- **Feed Fetching**: RSS/Atom parsing via feed-rs, conditional GET, on-demand fetch, background updater (30min)
- **Privacy**: detect and hide private/paid feeds with access tokens in URLs
- **Advanced**: sync groups, settings (per scope), podcast lists, chapters, favorites
- **CLI**: `rpodder serve`, `migrate`, `user create/delete`, `repair`
- **Config**: TOML file + `RPODDER_*` env vars
- **Health**: `GET /health` endpoint
- **Multi-DB**: PostgreSQL (scale) + SQLite (self-hosting)
- **URL normalization**: lowercase host, strip trailing slashes, strip fragments

### Web UI (Svelte 5 + Tailwind v4)
- **Embedded in binary** via `rust-embed` (feature `web-ui`, default on)
- **Disableable**: `cargo build --no-default-features` for API-only mode
- **Pages**: home, login, discover (directory/toplist/tag/search/podcast detail), subscriptions, devices
- **Discover**: gpodder.net-style directory with sidebar, categories, tag cloud, podcast detail with paginated episodes
- **Subscribe/Unsubscribe**: from search results, tag pages, podcast detail, subscriptions page
- **Devices**: real sub count, rename, delete with confirmation
- **Privacy**: private feeds hidden from directory, visible in user's subscriptions

### Infrastructure
- Docker multi-stage build (bun frontend + Rust backend)
- docker-compose profiles: dev (PostgreSQL + cargo-watch), sqlite, release
- GitHub Actions CI (build, test, clippy, fmt)
- Systemd service file with security hardening
- Idempotent migrations (IF NOT EXISTS)
- FTS5 repair command (`rpodder repair`)

### Tested with
- Kasts (KDE) — full sync flow verified
- 82 automated tests (unit + integration)
