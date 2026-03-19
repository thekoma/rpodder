# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is rpodder

rpodder is a modern, scalable, Rust-based replacement for [gpodder.net](https://gpodder.net) (mygpo). It implements the [gpodder API](https://gpoddernet.readthedocs.io/en/latest/api/index.html) for syncing podcast subscriptions and episode actions across devices. Target: multi-user with tens of thousands of indexed podcasts, but also self-hostable on a Raspberry Pi.

## Project Status

**Functional.** All core gpodder API endpoints implemented, web UI working, tested with Kasts client, SSO via Authentik working. See `docs/TODO.md` for remaining items (mostly nice-to-haves). See `docs/ARCHITECTURE.md` for design decisions. See `CHANGELOG.md` for release notes.

## Build & Test

```bash
cargo build                          # build all crates (includes web UI)
cargo build --no-default-features    # API-only, no embedded web UI
cargo test                           # run all tests (~85 tests)
cargo test -p rpodder-core           # test single crate
cargo clippy --workspace -- -D warnings  # lint (CI enforces this)
cargo fmt --all -- --check           # check formatting

# Web UI (Svelte)
cd web && bun install && bun run build  # build frontend → web/dist/
cd web && bun run dev                   # dev server on :5173, proxies API to :3005
```

**Important**: The CI workflow needs `web/dist/` to exist for rust-embed. The CI creates a placeholder automatically. For local builds, run `bun run build` in `web/` first, or use `--no-default-features`.

## Docker & Dev Environment

```bash
# PostgreSQL + release build (recommended for testing):
docker compose --profile release up -d postgres rpodder-release

# Self-hosted SQLite mode:
docker compose --profile sqlite up -d rpodder-sqlite

# Create a user:
docker compose --profile sqlite exec rpodder-sqlite rpodder user create myuser mypass

# Standalone frontend (nginx) + API backend:
docker compose --profile frontend up -d
```

- `Dockerfile` — Multi-stage: `oven/bun` (frontend) → `rust:bookworm` (backend) → `debian:bookworm-slim` (runtime)
- `Dockerfile.dev` — cargo-watch for live reload
- `docker-compose.yml` — Profiles: default (PG+dev), release (PG+release), sqlite, frontend
- `.env.oauth` — OAuth2 config (gitignored), loaded by release profile

**NEVER destroy Docker volumes** (`-v` flag) without asking — the user's podcast data persists there.

## Workspace Structure

```
crates/
  rpodder-core/     Domain types, error types, repo traits, URL normalization, privacy detection
  rpodder-db/       sqlx implementations for PostgreSQL + SQLite (all 13 repo traits)
  rpodder-feed/     Feed fetching (reqwest + retry) & parsing (feed-rs)
  rpodder-server/   axum HTTP server, route handlers, auth, CLI, email, OAuth, web UI embed
web/                Svelte 5 + SvelteKit + Tailwind v4 (adapter-static → dist/)
migrations/
  postgresql/       Idempotent SQL migrations (IF NOT EXISTS, DO blocks)
  sqlite/           Idempotent SQL migrations (IF NOT EXISTS)
config/             Example config, systemd service
.github/workflows/  CI (test/clippy/fmt) + Release (build+push to ghcr.io)
```

## Key Design Decisions

- **Repo trait pattern**: `rpodder-core/src/repo.rs` defines 13 async traits. `rpodder-db` implements them for PG + SQLite.
- **Multi-DB via sqlx**: Raw queries with `query_as::<_, Row>()` + `FromRow`. No compile-time macros (no DATABASE_URL needed).
- **UUID v7**: all entity IDs, time-sorted.
- **Auth**: HTTP Basic Auth + session cookies (gpodder compat) + OAuth2/OIDC SSO.
- **Privacy**: URLs with detected tokens are hidden from public directory but visible to subscribed users.
- **Web UI**: Svelte 5 SPA embedded via rust-embed, `ssr=false` + `prerender=false` globally, `$effect` for data loading (not `onMount`).
- **Feed updater**: Background loop every 30min, adaptive intervals (1h-7d based on subscribers), retry with backoff, skips private feeds.
- **Episode actions**: Accepts both `{"actions":[...]}` and bare `[...]` array (Kasts compat). Timestamp as string or integer.
- **AppState**: `{ db: Arc<Db>, config: Arc<AppConfig> }`.

## Configuration

All via `RPODDER_*` env vars or TOML config file (`-c config.toml`):

| Env var | Default | Description |
|---------|---------|-------------|
| `RPODDER_DATABASE_URL` | `sqlite://rpodder.db` | PG or SQLite |
| `RPODDER_HOST` | `127.0.0.1` | Bind address |
| `RPODDER_PORT` | `3005` | Bind port |
| `RPODDER_RUN_MIGRATIONS` | `false` | Auto-migrate on start |
| `RPODDER_REGISTRATION` | `open` | `open`/`closed`/`invite` |
| `RPODDER_SMTP_HOST` | | SMTP server for email activation |
| `RPODDER_OAUTH_ISSUER_URL` | | OIDC issuer (e.g. Authentik) |
| `RPODDER_OAUTH_CLIENT_ID` | | OAuth2 client ID |
| `RPODDER_OAUTH_CLIENT_SECRET` | | OAuth2 client secret |
| `RPODDER_BASE_URL` | | Public URL for callbacks |

## Gotchas / Known Issues

- **axum route params**: `{param}` must be entire segment. `{id}.json` doesn't work — capture whole segment, strip suffix in handler.
- **Svelte 5 + adapter-static**: Must use `$effect` with `browser` guard, NOT `onMount`, for data fetching. `ssr=false` in `+layout.ts`.
- **PG migrations**: No `CREATE TRIGGER IF NOT EXISTS` — use `DROP IF EXISTS` + `CREATE`. No `ADD COLUMN IF NOT EXISTS` — use `DO $$ BEGIN ... EXCEPTION WHEN duplicate_column`.
- **Docker build cache**: Frontend changes may not invalidate Rust layer. Use `--no-cache` when UI changes don't appear.
- **Kasts episode format**: Sends bare `[...]` not `{"actions":[...]}`. Handler tries both.
- **feed-rs date parsing**: Fails on `<pubDate>` with trailing whitespace — feed producer bug, not ours.
- **Config env parsing**: Uses `prefix_separator("_")` — the prefix `RPODDER_` is stripped, remainder is the key name (e.g. `DATABASE_URL`).
