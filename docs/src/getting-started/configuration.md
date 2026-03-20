# Configuration

rpodder is configured through environment variables (prefixed with `RPODDER_`) or a TOML config file. Environment variables take precedence over the config file.

## Config file

```bash
rpodder -c /path/to/config.toml serve
```

Example config file:

```toml
database_url = "sqlite:///var/lib/rpodder/rpodder.db"
host = "0.0.0.0"
port = 3005
run_migrations = true
registration = "open"
base_url = "https://podcast.example.com"
```

## Environment variables

All settings use the `RPODDER_` prefix. The prefix is stripped and the remainder becomes the key.

### Core

| Variable | Default | Description |
|----------|---------|-------------|
| `RPODDER_DATABASE_URL` | `sqlite://rpodder.db` | Database connection string. Use `postgres://user:pass@host:5432/db` for PostgreSQL or `sqlite:///path/to/file.db` for SQLite |
| `RPODDER_HOST` | `127.0.0.1` | Bind address. Use `0.0.0.0` to listen on all interfaces |
| `RPODDER_PORT` | `3005` | Bind port |
| `RPODDER_RUN_MIGRATIONS` | `false` | Automatically run database migrations on startup |
| `RPODDER_MIGRATIONS_DIR` | `migrations` | Path to the migrations directory |
| `RPODDER_BASE_URL` | | Public URL for OAuth callbacks and email links (e.g. `https://podcast.example.com`) |

### Registration

| Variable | Default | Description |
|----------|---------|-------------|
| `RPODDER_REGISTRATION` | `open` | Registration mode: `open` (anyone can register), `closed` (admin creates users), `invite` (email activation required) |

!!! note "First user exception"
    If no active users exist in the database, registration is always allowed regardless of this setting. The first user automatically becomes an admin.

### SMTP (email)

Required for `invite` registration mode, password reset emails, and admin-triggered password resets.

| Variable | Default | Description |
|----------|---------|-------------|
| `RPODDER_SMTP_HOST` | | SMTP server hostname |
| `RPODDER_SMTP_PORT` | `25` | SMTP port |
| `RPODDER_SMTP_USER` | | SMTP username (optional) |
| `RPODDER_SMTP_PASSWORD` | | SMTP password (optional) |
| `RPODDER_SMTP_FROM` | `rpodder@{smtp_host}` | From address for emails |
| `RPODDER_SMTP_SECURITY` | `none` | `none`, `starttls`, or `tls` |

### OAuth2 / OIDC (SSO)

| Variable | Default | Description |
|----------|---------|-------------|
| `RPODDER_OAUTH_ISSUER_URL` | | OIDC issuer URL (e.g. `https://sso.example.com/application/o/rpodder/`) |
| `RPODDER_OAUTH_CLIENT_ID` | | OAuth2 client ID |
| `RPODDER_OAUTH_CLIENT_SECRET` | | OAuth2 client secret |
| `RPODDER_OAUTH_PROVIDER_NAME` | `SSO` | Display name for the login button (e.g. `Authentik`, `Google`) |
| `RPODDER_OAUTH_ADMIN_GROUP` | | OIDC group name that grants admin role. Users in this group are automatically made admin on every SSO login |

### Podcast Index

[Podcast Index](https://podcastindex.org/) is an open, free podcast database with over 4 million podcasts. It powers rpodder's external search and trending charts, giving your users access to the entire podcast ecosystem — not just what's already indexed locally.

Get your **free** API key and secret at [api.podcastindex.org](https://api.podcastindex.org/) (takes 30 seconds, no credit card needed).

| Variable | Default | Description |
|----------|---------|-------------|
| `RPODDER_PODCASTINDEX_KEY` | | Podcast Index API key |
| `RPODDER_PODCASTINDEX_SECRET` | | Podcast Index API secret |

!!! tip "Highly recommended"
    Without Podcast Index, search only returns podcasts already subscribed to by users on your instance. With it, users can discover any podcast in existence and subscribe directly from the search results. rpodder fetches and indexes the feed automatically.

!!! warning "Dollar signs in secrets"
    If your Podcast Index secret contains `$`, escape it as `$$` in `.env` files used by Docker Compose. For example: `RPODDER_PODCASTINDEX_SECRET=abc$$def` will be read as `abc$def`.

## Database choice

### SQLite

- **Use when**: personal use, 1-5 users, Raspberry Pi, simplest setup
- **Connection string**: `sqlite:///path/to/rpodder.db` or `sqlite://rpodder.db` (relative)
- **Features**: WAL mode enabled automatically, foreign keys enforced, FTS5 for search

### PostgreSQL

- **Use when**: multiple users, thousands of podcasts, production deployment
- **Connection string**: `postgres://user:password@host:5432/database`
- **Features**: full-text search with `tsvector`, connection pooling (20 max connections)

Both databases support the exact same feature set. All 13 repository traits are implemented for both backends.
