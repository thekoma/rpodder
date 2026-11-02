# Database

rpodder supports PostgreSQL and SQLite with identical feature sets. The database is chosen by the `RPODDER_DATABASE_URL` connection string.

## Schema

### Core tables

| Table | Description |
|-------|-------------|
| `users` | User accounts (username, password hash, email, is_active, is_admin) |
| `sessions` | Login sessions and token storage (auth, activation, password reset) |
| `devices` | User devices with device_id, caption, type |
| `podcasts` | Podcast metadata (title, description, author, logo, subscribers) |
| `podcast_urls` | Maps feed URLs to podcasts (supports redirects, multiple URLs) |
| `episodes` | Episode metadata (title, description, duration, released date) |
| `episode_urls` | Maps media URLs to episodes |
| `subscriptions` | User-podcast subscriptions per device |
| `subscription_changes` | Delta history for sync |
| `episode_actions` | Play/download/delete actions with positions |
| `sync_groups` | Device sync group definitions |
| `tags` | Podcast category tags |
| `settings` | User settings (per scope: account/device/podcast/episode) |
| `podcast_lists` | User-created podcast lists |
| `chapters` | Podcast chapter marks |
| `favorites` | Favorited episodes |

### SQLite-specific

- `podcasts_fts` — FTS5 virtual table for full-text search on title, description, author
- Triggers to keep FTS5 in sync with the `podcasts` table

### PostgreSQL-specific

- `tsvector` column on `podcasts` for full-text search
- `GIN` index for fast search queries

## Migrations

Migrations live in `migrations/postgresql/` and `migrations/sqlite/`. The `*.up.sql` files are
compiled into the binary (the `MIGRATIONS` list in `crates/rpodder-db/src/lib.rs`), so a release
binary needs no files next to it. On every start (and with `rpodder migrate`) the backend's
migrations run in list order, `001_*` before `002_*`.

### Idempotency

All migrations are written to be idempotent (safe to re-run):

- **PostgreSQL**: uses `IF NOT EXISTS`, `DO $$ BEGIN ... EXCEPTION WHEN duplicate_column THEN NULL; END $$`
- **SQLite**: uses `IF NOT EXISTS`. For `ALTER TABLE ADD COLUMN` (which isn't idempotent in SQLite), the migration runner catches "duplicate column" errors and skips them

### Adding a new migration

1. Create `migrations/postgresql/003_my_change.up.sql` and `migrations/sqlite/003_my_change.up.sql`
2. Make them idempotent
3. Optionally create `.down.sql` files for rollback
4. Add `"003_my_change"` to the end of `MIGRATIONS` in `crates/rpodder-db/src/lib.rs`. The build fails if
   either backend's file is missing, and a `rpodder-db` test fails if a file is not in the list

## IDs

All entity IDs use UUID v7, which is time-sortable. This means:

- IDs are globally unique
- They sort chronologically
- No need for auto-increment sequences
- Works identically in PostgreSQL (native UUID type) and SQLite (stored as TEXT)

## Connection pools

- **PostgreSQL**: 20 max connections
- **SQLite**: 5 max connections, WAL mode, foreign keys enabled
