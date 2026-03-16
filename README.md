# rpodder

A modern, fast, [gpodder.net](https://gpodder.net)-compatible podcast sync server written in Rust.

Sync your podcast subscriptions and episode progress across [AntennaPod](https://antennapod.org), [gPodder](https://gpodder.github.io), [Kasts](https://apps.kde.org/kasts/), and any client that supports the [gpodder.net API](https://gpoddernet.readthedocs.io/).

## Features

- **Full gpodder API** — subscriptions, episode actions, devices, sync groups, settings, favorites, chapters, podcast lists
- **Multi-format** — JSON, OPML, and TXT for subscription import/export
- **Dual database** — PostgreSQL for scale, SQLite for self-hosting
- **Feed indexing** — automatic feed fetching, parsing, and search
- **Lightweight** — single binary, ~15MB, low memory footprint

## Quickstart

### SQLite (self-hosted)

```bash
# Build
cargo build --release

# Initialize database and create a user
./target/release/rpodder migrate
./target/release/rpodder user create myuser mypassword

# Start the server
./target/release/rpodder serve
```

The server starts on `http://localhost:3005`. Point your podcast app at this URL.

### Docker (SQLite)

```bash
docker compose --profile sqlite up -d
# Create a user
docker compose exec rpodder-sqlite rpodder user create myuser mypassword
```

### Docker (PostgreSQL)

```bash
docker compose up -d
# Create a user inside the dev container
docker compose exec rpodder cargo run -p rpodder-server -- user create myuser mypassword
```

## Configuration

All settings can be set via environment variables or a TOML config file:

| Env var | Default | Description |
|---------|---------|-------------|
| `RPODDER_DATABASE_URL` | `sqlite://rpodder.db` | Database connection URL |
| `RPODDER_HOST` | `127.0.0.1` | Bind address |
| `RPODDER_PORT` | `3005` | Bind port |
| `RPODDER_RUN_MIGRATIONS` | `false` | Run migrations on startup |
| `RPODDER_MIGRATIONS_DIR` | `migrations` | Path to migration files |

Or use a config file: `rpodder -c config.toml serve`

See [`config/rpodder.example.toml`](config/rpodder.example.toml) for an example.

## CLI

```
rpodder serve                              # Start the server
rpodder migrate                            # Run database migrations
rpodder user create <username> <password>  # Create a user
rpodder user delete <username>             # Deactivate a user
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
| `GET /search.json?q=...` | Podcast search |
| `GET /toplist/{count}.json` | Top podcasts |
| `GET /health` | Health check |
| `/` | Status dashboard |

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
