<p align="center">
  <img src="web/static/logo.svg" alt="rpodder logo" width="200" />
</p>

<h1 align="center">rpodder</h1>

<p align="center">A modern, fast, <a href="https://gpodder.net">gpodder.net</a>-compatible podcast sync server written in Rust.</p>

<p align="center">
  <a href="https://github.com/thekoma/rpodder/actions/workflows/ci.yml"><img src="https://github.com/thekoma/rpodder/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/thekoma/rpodder/pkgs/container/rpodder"><img src="https://ghcr-badge.egpl.dev/thekoma/rpodder/latest_tag?trim=major&label=latest" alt="Latest Release"></a>
  <a href="https://ghcr.io/thekoma/rpodder"><img src="https://ghcr-badge.egpl.dev/thekoma/rpodder/size" alt="Image Size"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-AGPL--3.0-blue" alt="License: AGPL-3.0"></a>
  <img src="https://img.shields.io/badge/rust-edition%202024-orange?logo=rust" alt="Rust Edition 2024">
  <img src="https://img.shields.io/badge/database-PostgreSQL%20%7C%20SQLite-336791?logo=postgresql" alt="Database: PostgreSQL | SQLite">
  <img src="https://img.shields.io/badge/frontend-Svelte%205-FF3E00?logo=svelte&logoColor=white" alt="Frontend: Svelte 5">
  <img src="https://img.shields.io/badge/API-gpodder%20compatible-green" alt="API: gpodder compatible">
  <img src="https://img.shields.io/badge/arch-amd64%20%7C%20arm64-blueviolet" alt="Arch: amd64 | arm64">
  <img src="https://img.shields.io/badge/deploy-Docker%20%7C%20K8s-2496ED?logo=docker&logoColor=white" alt="Deploy: Docker | K8s">
  <a href="https://thekoma.github.io/rpodder/"><img src="https://img.shields.io/badge/docs-MkDocs-1D6A50?logo=materialformkdocs&logoColor=white" alt="Docs"></a>
  <img src="https://img.shields.io/badge/self--hosted-Raspberry%20Pi%20ready-C51A4A?logo=raspberrypi&logoColor=white" alt="Self-hosted: Raspberry Pi ready">
  <img src="https://img.shields.io/badge/SSO-OIDC%20%2F%20OAuth2-purple?logo=openid" alt="SSO: OIDC / OAuth2">
</p>

<p align="center">
  <img src="docs/src/assets/screenshots/library.png" alt="rpodder Library" width="700" />
</p>

Sync your podcast subscriptions and episode progress across [AntennaPod](https://antennapod.org), [gPodder](https://gpodder.github.io), [Kasts](https://apps.kde.org/kasts/), and any client that supports the [gpodder.net API](https://gpoddernet.readthedocs.io/).

## Features

### Sync
- **Full gpodder API** — subscriptions, episode actions, devices, sync groups, settings, favorites, chapters, podcast lists. Works with AntennaPod, gPodder, Kasts and other gpodder clients
- **Reliable episode sync** — positions uploaded late still reach your other devices (`since` follows when the server received an action, not the phone's clock), updated positions are re-delivered, `aggregated=true` returns the latest action per episode, no 1000-action cap
- **Sync groups** — keep the subscriptions of several devices identical; the browser can join one too

### Web interface
- **Built in** — embedded in the binary, fonts and icons included: no outside requests except podcast artwork and audio
- **Library** — Continue listening across all your devices, subscriptions filed by letter, which device holds each podcast, last played, filter
- **Web player** — listen in the browser with a player bar that survives navigation and a full-screen view on phones; optional position sync as a gpodder device, so you resume where your phone stopped (and the phone where the browser stopped)
- **Your data** — add any feed by URL (private feeds too), download episodes, export OPML/TXT/JSON for the account or a single device
- **Fresh episodes** — feeds are checked at their own publishing rhythm (hourly for a news show, daily for a weekly one), and opening a podcast re-reads a feed that hasn't been checked for an hour
- **Light and dark themes**, phone layout with a bottom tab bar, History by day, Devices and sync-group editor, admin panel

### Accounts and security
- **User roles** — admin/user, first user becomes admin, full user management panel
- **SSO / OAuth2** — OIDC single sign-on (Authentik, Keycloak, …), group-based admin mapping, login bound to the browser that started it (OAuth `state`)
- **Registration control** — open, closed, or invite-only (email activation)
- **Passwords** — self-service change and email reset, admin set/reset; any credential change or deactivation signs out the user's other sessions
- **Privacy** — private/paid feeds (tokens in the URL) are hidden from the public directory

### Discovery
- **Directory and search** — full-text search over every podcast on the server, top podcasts, categories
- **Podcast Index** — optional search across millions of podcasts and trending charts by language
- **HTTPS upgrade suggestions** — finds HTTP subscriptions that have an HTTPS address

### Operations
- **Dual database** — PostgreSQL for scale, SQLite for self-hosting (Raspberry Pi friendly)
- **Single binary**, multi-arch Docker images (amd64/arm64), Helm chart, Prometheus metrics
- **Self-applying migrations** — every `*.up.sql` in the migrations directory, idempotent on both databases

See the [documentation](https://thekoma.github.io/rpodder/) and the [changelog](CHANGELOG.md) for details.

## Quickstart

### SQLite (self-hosted)

```bash
# Build
cargo build --release

# Initialize database and create an admin user
./target/release/rpodder migrate
./target/release/rpodder user create myuser mypassword --admin

# Start the server
./target/release/rpodder serve
```

The server starts on `http://localhost:3005`. Point your podcast app at this URL.

### Docker (SQLite)

```bash
docker compose --profile sqlite up -d
# Create a user
docker compose exec rpodder-sqlite rpodder user create myuser mypassword --admin
```

### Docker (PostgreSQL)

```bash
docker compose --profile release up -d
```

> The compose profiles above build from this checkout. To run the **published
> image** without building, use the deployment methods below.

## Deployment

Production self-hosting uses the published image
[`ghcr.io/thekoma/rpodder`](https://ghcr.io/thekoma/rpodder) — no build required.

### Docker Compose

Ready-to-use compose files live in [`examples/`](examples/). They import cleanly
into Dockge, Portainer and CasaOS.

```bash
# SQLite — simplest, single container
docker compose -f examples/docker-compose.yml up -d

# PostgreSQL — for multi-user instances
cp examples/.env.example examples/.env   # then set POSTGRES_PASSWORD
docker compose -f examples/docker-compose.postgres.yml up -d
```

Create the first admin user:

```bash
docker compose -f examples/docker-compose.yml exec rpodder rpodder user create <name> <password> --admin
```

### Kubernetes (Helm)

A self-managed chart built on the [bjw-s common library](https://bjw-s-labs.github.io/helm-charts)
lives in [`deploy/helm/rpodder/`](deploy/helm/rpodder/):

```bash
helm dependency build deploy/helm/rpodder
helm install rpodder deploy/helm/rpodder \
  --set ingress.main.enabled=true \
  --set 'ingress.main.hosts[0].host=podcasts.example.com'
```

Defaults to SQLite on a PersistentVolumeClaim; point `RPODDER_DATABASE_URL` at
PostgreSQL for multi-user. See the [chart README](deploy/helm/rpodder/README.md).

### TrueCharts

We're looking into [TrueCharts](https://truecharts.org) support. Once we've
carefully gone through their contribution guidelines, we may add rpodder to the
catalog. Until then, use the Helm chart above.

### Bare metal (systemd)

See [Systemd](#systemd) below.

## Configuration

All settings can be set via environment variables or a TOML config file:

| Env var | Default | Description |
|---------|---------|-------------|
| `RPODDER_DATABASE_URL` | `sqlite://rpodder.db` | Database connection URL |
| `RPODDER_HOST` | `0.0.0.0` | Bind address |
| `RPODDER_PORT` | `3005` | Bind port |
| `RPODDER_RUN_MIGRATIONS` | `true` | Run migrations on startup |
| `RPODDER_REGISTRATION` | `open` | `open` / `closed` / `invite` |
| `RPODDER_SMTP_HOST` | | SMTP server for email features |
| `RPODDER_OAUTH_ISSUER_URL` | | OIDC issuer URL for SSO |
| `RPODDER_OAUTH_CLIENT_ID` | | OAuth2 client ID |
| `RPODDER_OAUTH_CLIENT_SECRET` | | OAuth2 client secret |
| `RPODDER_OAUTH_ADMIN_GROUP` | | OIDC group name for admin role |
| `RPODDER_BASE_URL` | | Public URL for OAuth callbacks and emails |
| `RPODDER_PODCASTINDEX_KEY` | | [Podcast Index](https://podcastindex.org/) API key (free) |
| `RPODDER_PODCASTINDEX_SECRET` | | Podcast Index API secret |

Or use a config file: `rpodder -c config.toml serve`

See [`config/rpodder.example.toml`](config/rpodder.example.toml) for an example.

## CLI

```
rpodder serve                                    # Start the server
rpodder migrate                                  # Run database migrations
rpodder user create <username> <password>         # Create a user
rpodder user create <username> <password> --admin # Create an admin user
rpodder user delete <username>                    # Deactivate a user
rpodder repair                                   # Rebuild FTS5 index (SQLite)
```

## Client Setup

### AntennaPod
Settings → Synchronization → Choose provider → gpodder.net
Server: `http://your-server:3005` • Username/Password as created above

### Kasts (KDE)
Settings → Synchronization → Custom server
`http://your-server:3005`

### gPodder Desktop
Preferences → gpodder.net → Server URL: `http://your-server:3005`

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `POST /api/2/auth/{user}/login.json` | Login (HTTP Basic Auth) |
| `GET/PUT /subscriptions/{user}/{device}.{json,opml,txt}` | Subscription sync |
| `GET/POST /api/2/subscriptions/{user}/{device}.json` | Delta subscription sync |
| `GET/POST /api/2/episodes/{user}.json` | Episode action sync |
| `GET/POST /api/2/devices/{user}/{device}.json` | Device management |
| `GET /api/2/me` | Current user info |
| `POST /api/2/me/password` | Change password |
| `GET /api/2/me/upgrades` | HTTPS upgrade suggestions |
| `POST /api/2/register` | Public registration |
| `POST /api/2/password-reset` | Request password reset |
| `GET /search.json?q=...` | Podcast search |
| `GET /toplist/{count}.json` | Top podcasts |
| `GET /api/admin/users` | Admin: list users |
| `GET /api/admin/stats` | Admin: server statistics |
| `GET /health` | Health check |

See [`docs/API_REFERENCE.md`](docs/API_REFERENCE.md) for the complete API surface.

## Systemd

```bash
sudo cp target/release/rpodder /usr/local/bin/
sudo cp config/rpodder.service /etc/systemd/system/
sudo useradd -r -s /bin/false rpodder
sudo mkdir -p /var/lib/rpodder /usr/share/rpodder
sudo cp -r migrations /usr/share/rpodder/
sudo chown rpodder:rpodder /var/lib/rpodder
sudo systemctl enable --now rpodder
```

## License

AGPL-3.0-or-later
