# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is rpodder

rpodder is a modern, scalable, Rust-based replacement for [gpodder.net](https://gpodder.net) (mygpo). It implements the [gpodder API](https://gpoddernet.readthedocs.io/en/latest/api/index.html) for syncing podcast subscriptions and episode actions across devices. Target: multi-user with tens of thousands of indexed podcasts, but also self-hostable on a Raspberry Pi.

## Project Status

**Early development.** See `docs/TODO.md` for the full roadmap and current progress. See `docs/ARCHITECTURE.md` for design decisions. See `docs/API_REFERENCE.md` for the complete API surface to implement.

## Build & Test

```bash
cargo build                          # build all crates
cargo test                           # run all tests
cargo test -p rpodder-core           # test single crate
cargo run -p rpodder-server          # run the server binary
cargo clippy --workspace             # lint
cargo fmt --all -- --check           # check formatting
```

## Docker & Dev Environment

```bash
# Development with live reload (cargo-watch):
docker compose up                    # starts PostgreSQL + rpodder dev server
docker compose up -d postgres        # just the database

# Continuous build loop (Skaffold watches for changes, rebuilds, redeploys):
skaffold dev                         # dev loop with Dockerfile.dev
skaffold dev -p release              # test production image

# Production-like build:
docker compose --profile release up  # builds Dockerfile, runs release binary

# Self-hosted SQLite mode:
docker compose --profile sqlite up   # no PostgreSQL needed

# One-shot build:
docker compose build rpodder         # dev image
skaffold build                       # same via skaffold
```

- `Dockerfile` — Multi-stage production build (rust:bookworm → debian:bookworm-slim)
- `Dockerfile.dev` — Development build with cargo-watch for live reload
- `docker-compose.yml` — PostgreSQL + rpodder dev server, profiles for release/sqlite
- `skaffold.yaml` — Continuous build using docker-compose deployer (no K8s required)

PostgreSQL connection (from docker-compose): `postgres://rpodder:rpodder@localhost:5432/rpodder`

## Workspace Structure

```
crates/
  rpodder-core/     Domain types, error types, repository traits (no DB dependency)
  rpodder-db/       sqlx implementations of repo traits (PostgreSQL + SQLite)
  rpodder-feed/     Async feed fetching & parsing (feed-rs + reqwest)
  rpodder-server/   axum HTTP server, route handlers, auth middleware, CLI
migrations/
  postgresql/       SQL migrations for PostgreSQL
  sqlite/           SQL migrations for SQLite
docs/               Architecture, API reference, TODO tracker
config/             Example configuration files
```

## Key Design Decisions

- **Repo trait pattern**: `rpodder-core/src/repo.rs` defines async traits per entity. `rpodder-db` implements them for each DB backend. The server only depends on the traits.
- **Multi-DB via sqlx**: PostgreSQL is the primary target. SQLite for lightweight self-hosting. Separate migration files per backend. MySQL is NOT a target.
- **UUID v7**: all entity IDs use UUIDv7 (time-sorted, better index locality than v4).
- **No ORM**: raw sqlx queries with compile-time checking.
- **Auth**: HTTP Basic Auth (gpodder compat) + session tokens stored in DB.

## Reference Projects (read-only, will be removed)

The subdirectories `mygpo/`, `gpodder2go/`, and `nextcloud-gpodder/` are clones of existing gpodder server implementations kept temporarily as reference for API behavior and data models. They are NOT part of the Cargo workspace. They will be removed once rpodder reaches feature parity.

- `mygpo/` — Original gpodder.net (Django/Python). Most complete reference for API behavior.
- `gpodder2go/` — Go implementation. Good reference for lightweight self-hosting patterns.
- `nextcloud-gpodder/` — Nextcloud app (PHP). Good reference for episode action sync.
