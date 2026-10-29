# Changelog

## 2026.10.7 — 2026-10-29

### Security
- **Opening many new podcasts at once could still exhaust memory** — since 2026.10.6 only a logged-in user makes rpodder read a feed it doesn't know yet, but nothing limited how many of those reads ran together, and each can hold a feed of up to 32 MB in memory while it is parsed. With open registration anyone can sign up, so one account opening dozens of new podcasts at once could still get the process killed. Now at most two of these first reads run at a time across the server; a request waits up to 10 seconds for its turn, then answers as if the feed couldn't be read. A read also finishes when the client hangs up, so the podcast created for a URL that doesn't work is always removed again.
- **A proxy from the environment could reach the server's own network** — with `HTTP_PROXY`, `HTTPS_PROXY` or `ALL_PROXY` set, the proxy resolved feed host names itself, past the check that keeps fetches off internal addresses. Feed, website and icon fetches now ignore these variables unless `RPODDER_ALLOW_PRIVATE_FEEDS=true`, and rpodder logs a warning at startup when they are set.
- **More non-public address ranges are refused** — multicast, IPv6 site-local (`fec0::/10`), local-use NAT64 (`64:ff9b:1::/48`), Teredo (`2001::/32`), the 6to4 relay (`192.88.99.0/24`), and 6to4 addresses (`2002::/16`) that carry a private IPv4 address.

## 2026.10.6 — 2026-10-29

### Security
- **Anonymous requests could make the server fetch any URL** — `GET /api/2/data/podcast.json` and `/api/2/data/podcast/episodes.json` are public, and for an unknown URL they created a podcast and fetched the URL right away. Anyone could make rpodder request internal or loopback addresses (SSRF), fill the database, or point it at a huge feed that got the process OOM-killed and, because the URL stayed in the database for the updater, kept it in a crash loop. These endpoints now fetch a new URL only for a logged-in user; an anonymous request for an unknown podcast gets `404`. Reported by Peter Gille.
- **Fetches no longer reach the server's own network** — feed, website and icon fetches refuse loopback, private, link-local and other non-public addresses, whether given as an IP or as a host name that resolves to one, on the first request and on every redirect; only `http` and `https` are fetched. Set `RPODDER_ALLOW_PRIVATE_FEEDS=true` if you subscribe to feeds hosted on your LAN.
- **A URL whose first read fails is not kept** — the podcast created for it is removed again (unless someone subscribed meanwhile), so the background updater doesn't retry it every cycle.
- **First open of an unknown feed waits at most 10 seconds** — one try instead of three 30-second tries.
- **Feed size and episode count are capped** — a feed body larger than 32 MB is refused instead of read into memory, and a read stores at most the newest 5,000 episodes. This also covers feeds a logged-in user subscribes to.

## 2026.10.5 — 2026-10-27

### Fixed
- **Subscribe did nothing on a podcast your devices already held under another address** — a feed can be known by several URLs (Radio Capital's GR as `…/833`, `…/833.rss` and an older address). The podcast page and the Library compared URLs as text, so a page opened on one address showed **Subscribe** although your devices held the feed under another, and clicking it changed nothing. Subscriptions are now matched by podcast: the page shows **Subscribed** with every device that holds it, and **Unsubscribe** removes it from all of them. New `GET /api/2/me/holdings`.

## 2026.10.4 — 2026-10-27

### Added
- **Cover art and website for feeds that have none** — when a feed carries no image, rpodder now looks on the podcast's website, in order, for an `apple-touch-icon`, the schema.org logo in the page's JSON-LD, `og:image`, the largest declared icon, then `/favicon.ico`. It looks at most once a day per podcast (**Refresh feed** looks again right away), keeps the cover it has if nothing turns up, never lets a favicon replace a real cover, and only fetches http(s) pages on the feed's own host or a public host — never loopback, private or link-local addresses or LAN names. Radio Capital's GR, for one, now shows the station's logo.

### Fixed
- **Website pointing at the feed itself** — some feeds put their own address in `<link>` (Radio Capital's echoes whatever URL it was requested with). rpodder now uses the root of the feed's site instead, without path or query, so a token in a private feed's URL never shows up as the website.
- **Adding a feed pasted from an OPML file** — text such as `…/833" type="rss" text="…"` (or its percent-encoded form `…/833%22%20type=…`) is cut back to the feed address in **Add feed** and Discover.

## 2026.10.3 — 2026-10-13

### Fixed
- **New episodes of most podcasts stopped appearing (2026.10.1–2026.10.2)** — with feed-rs 3, an item's iTunes tags come before its `<enclosure>` as a separate media object, and rpodder only looked at the first one: items without a `<link>` were skipped, items with one were stored under their web page (not playable). The parser now looks through all media objects, preferring audio/video; the next read of each feed restores the missing episodes and removes the copies stored under the web page.
- **Refresh feed did nothing visible** — it started the update in the background and reloaded the list after 3 seconds, often too early. It now downloads the feed in full (ignoring ETag/Last-Modified), waits for the result (up to 25 seconds) and reports how many new episodes it found, or the error.

### Added
- **Volume and mute in the web player**, on the player bar and the phone's Now playing screen, remembered in the browser. (iOS only allows muting.)

## 2026.10.2 — 2026-10-13

### Added
- **Feeds are checked at their own publishing rhythm** — after each read, rpodder schedules the next check at a quarter of the typical gap between recent episodes, between 1 and 24 hours: a news show with three editions a day is checked hourly, a daily show every 6 hours, a weekly show daily. A show that stops publishing drifts back to daily checks; feeds nobody subscribes to stay weekly. Before, a feed with one subscriber was checked every 12–24 hours, so a morning news edition could show up only in the afternoon.
- **Opening a podcast re-reads a stale feed** — if a feed hasn't been checked for an hour, opening its page reads it first (up to 4 seconds; slower feeds finish in the background and the page reloads the episode list once). `episodes.json` reports `"refreshing": true` while that happens.

### Fixed
- A feed update could overwrite a podcast's subscriber count with the value it read before downloading the feed.
- A podcast whose feed has an empty description was downloaded again on every visit to its page.

## 2026.10.1 — 2026-10-13

### Changed
- **Dependency updates** — argon2 0.6 (existing password hashes keep working), feed-rs 3, tower-http 0.7, base64 0.23 and the rest of the Rust tree; the web UI moves to SvelteKit 3 and adapter-static 4; the Helm chart's `common` library goes to 5.3.0. TypeScript stays on 6 until SvelteKit supports TypeScript 7.

## 2026.10.0 — 2026-10-13

### Added
- **Web player** — play episodes in the browser from a podcast's page or from **Continue listening**, with a player bar that keeps playing while you browse and a full-screen *Now playing* view on phones. The player is a sync device (`rpodder-web`): with **Sync listening positions** on it resumes where your phone stopped (and vice versa); it can also join a device sync group from Settings. `episodes.json` items now carry the episode's `url` and `media` URL; new `GET /api/2/me/in-progress` and `GET /api/2/me/positions`.
- **Add a feed by URL** — subscribe to any RSS/Atom address from the Library or by pasting it into Discover, private tokenized feeds included.
- **OPML export from the web UI** — for the whole account (Library, Settings → Your data) or a single device (Devices). Outlines now carry the podcast title, and the download gets a proper file name (`Content-Disposition`).
- **Per-episode download** button on the podcast page, plus played / minutes-left stamps.
- **Light theme** — the interface follows the system theme, or pick Light/Dark in the account menu or Settings → Appearance.

### Changed
- **Redesigned web interface** — the Library is the home page once signed in (subscriptions filed by letter, which devices hold each podcast, last played, filter), the podcast page opens with a catalog card (feed address with copy, devices, listeners), History is grouped by day, Devices has a reworked sync-group editor, and phones get a bottom tab bar. Fonts and icons are bundled: no requests to outside services except podcast artwork and audio.
- **Refresh feed** is shown to administrators only (it always required admin rights).
- History timestamps from PostgreSQL are ISO 8601, like SQLite's.

### Fixed
- **Episode actions uploaded late never reached other devices** — `since` now filters on the time the server received an action instead of the client's timestamp, updated positions are delivered again, and returned cursors no longer skip rows committed during a read. The 1000-action cap is gone, `aggregated=true` returns the latest action per episode, unknown devices are auto-created on upload, and unknown `device`/`podcast` filters return nothing instead of everything.
- **Subscription deltas could list the same URL in both `add` and `remove`.**
- **Duplicate untitled episodes** when two feed fetches ran at once — creating a podcast or episode by URL is now atomic on both databases.

### Security
- **SSO login CSRF** — the OAuth `state` is now bound to the browser that started the login, and logins started on another host are redirected to `RPODDER_BASE_URL` first.
- **Password reset and activation links worked as session cookies** — they are now rejected as logins.
- **Sessions are revoked on credential changes** — password reset, admin set-password, your own password change and deactivation sign out the user's other sessions. A login that was being verified against the old password (or before the deactivation) can no longer slip a session in after the revocation.
- Changing your password without the current one is allowed only from an SSO session.
- Session and one-time tokens use a 64-character random generator.
- Deactivated users' session cookies are rejected; the SSO callback escapes the username it embeds (XSS).

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
