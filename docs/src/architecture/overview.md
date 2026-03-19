# Architecture Overview

rpodder is built as a Rust workspace with four crates, designed around a clean separation between domain logic and infrastructure.

## Design principles

1. **Repo trait pattern** — domain logic depends on abstract traits, not concrete database implementations. This is how we support both PostgreSQL and SQLite with the same business logic.

2. **No compile-time database macros** — we use `sqlx::query_as::<_, Row>()` with `FromRow` at runtime, not `sqlx::query!()`. This means you don't need a `DATABASE_URL` at compile time.

3. **Single binary** — the web UI (Svelte SPA) is embedded in the Rust binary via `rust-embed`. One file to deploy.

4. **Feature flags** — `cargo build --no-default-features` produces an API-only binary without the web UI. The `web-ui` feature controls embedding.

## Request flow

```
Client (podcast app / browser)
  │
  ▼
axum Router
  │
  ├─ Public routes (no auth)
  │   ├─ /health, /metrics
  │   ├─ /search.json, /toplist, /api/2/trending
  │   ├─ /api/2/register, /api/2/password-reset
  │   └─ /auth/sso/*
  │
  ├─ Authenticated routes (Basic Auth or session cookie)
  │   ├─ /api/2/subscriptions/*, /api/2/episodes/*
  │   ├─ /api/2/devices/*, /api/2/me
  │   └─ /subscriptions/*
  │
  ├─ Admin routes (auth + is_admin check)
  │   └─ /api/admin/*
  │
  └─ Fallback (web UI SPA)
      └─ Serves index.html for client-side routing
```

## Background tasks

- **Feed updater** — spawned via `tokio::spawn` at startup. Runs every 30 minutes. Respects adaptive intervals and retries.

## State management

```rust
struct AppState {
    db: Arc<Db>,           // Database connection (PG or SQLite)
    config: Arc<AppConfig>, // All configuration
}
```

`AppState` is passed to all handlers via axum's `State` extractor. It's `Clone` and `Send + Sync`.

## Authentication middleware

Two middleware layers, applied in order:

1. **`require_auth_layer`** — extracts session cookie or HTTP Basic Auth, resolves user, inserts `AuthUser` into request extensions
2. **`require_admin_layer`** — checks `AuthUser.is_admin`, returns 403 if not

The admin layer is applied on top of the auth layer. In axum, route layers run inner-first, so:

```rust
.route_layer(require_admin_layer())   // runs second
.route_layer(require_auth_layer())    // runs first
```

## Database abstraction

```rust
enum Db {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}
```

The `Db` enum dispatches to the correct pool. A `with_repo!` macro simplifies calling trait methods:

```rust
let user = with_repo!(state, |repo| {
    UserRepo::find_by_username(&repo, "alice").await
})?;
```
