# Changelog

## 2026.5.4 — 2026-05-30

### Fixed
- **Self-registration failed with "Username already exists or registration failed"** — the registration page posted to the admin-only `/api/admin/users` endpoint (which returns 401 for anonymous visitors) instead of the public `/api/2/register` endpoint. New users can now register again.

### Changed
- **Calendar versioning is now unpadded (`YYYY.M.N`)** — the month no longer carries a leading zero (e.g. `2026.5.4`, not `2026.05.4`). This makes the version a valid SemVer string, so the git tag, Docker image tag, `Cargo.toml` and `package.json` versions all match exactly. Historical `2026.0M.x` tags are unaffected.

### Docs
- **Helm charts document every setting** — the packaged `deploy/helm/rpodder/` chart now lists all `RPODDER_*` environment variables (database, SMTP, OAuth/OIDC, Podcast Index, metrics) as commented examples in `values.yaml`, so the full feature surface is discoverable at a glance.
- **TrueCharts wording corrected** — docs now state that TrueCharts support is being explored and may be added after reviewing their contribution guidelines, rather than implying it is already in progress.

## 2026.05.1 — 2026-05-29

### Fixed
- **SQLite "(code: 5) database is locked"** — on slow disks (e.g. Synology NAS, Raspberry Pi) concurrent writes from the background feed updater and API request handlers collided on SQLite's single write lock, surfacing as `(code: 5) database is locked`. Writes now serialize through a single pooled connection, and connections open with WAL journaling + `synchronous=NORMAL` + a 30s `busy_timeout` so contending writers queue instead of failing immediately. Thanks to @update-freak for the report and @nielmin for the corroborating details (#19)

### Changed
- **Dependency updates** — refreshed the full Rust dependency tree, most notably **sqlx 0.8 → 0.9**. sqlx 0.9 audits runtime-built SQL strings via a new `SqlSafeStr` trait, so dynamically assembled queries are now explicitly wrapped with `sqlx::AssertSqlSafe`. Also bumped axum, axum-extra, clap, config, lettre, reqwest, tokio, tower-http and ~130 transitive crates (#14)

## v0.1.1 — 2026-03-23

### Fixed
- **SSO sessions expired after 24 hours** — OAuth/SSO sessions were hardcoded to 24h while regular login sessions lasted 365 days, causing SSO users to lose access daily until re-login

### Added
- **Configurable session duration** — new `RPODDER_SESSION_DURATION_DAYS` setting (default: 90 days) controls session lifetime for both regular and SSO login. Configurable via env var or `session_duration_days` in TOML config

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
