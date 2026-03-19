# Installation

rpodder can be installed in several ways depending on your needs.

## Docker (recommended)

Docker is the easiest way to get started. rpodder provides pre-built images and a `docker-compose.yml` with multiple profiles.

```bash
git clone https://github.com/thekoma/rpodder.git
cd rpodder
```

### SQLite mode (simplest)

Perfect for personal use or a small household. Data is stored in a single file.

```bash
docker compose --profile sqlite up -d
docker compose exec rpodder-sqlite rpodder user create myuser mypass --admin
```

Your server is now running at `http://localhost:3005`.

### PostgreSQL mode (recommended for multi-user)

For multiple users, more than a few hundred podcasts, or production deployments.

```bash
docker compose --profile release up -d
```

This starts PostgreSQL + rpodder with migrations auto-applied.

### Development mode (live reload)

For hacking on rpodder itself:

```bash
docker compose up -d   # PostgreSQL + cargo-watch with live reload
```

Source code is bind-mounted — changes are picked up automatically.

## From source

### Prerequisites

- Rust 1.80+ (edition 2024)
- [Bun](https://bun.sh/) (for building the web UI)

### Build

```bash
# Build the web UI first
cd web && bun install && bun run build && cd ..

# Build rpodder
cargo build --release
```

The binary is at `target/release/rpodder`.

### API-only build (no web UI)

If you don't need the embedded web interface:

```bash
cargo build --release --no-default-features
```

This produces a smaller binary that only serves the API.

## Systemd service

For running rpodder as a system service on Linux:

```bash
sudo cp target/release/rpodder /usr/local/bin/
sudo cp config/rpodder.service /etc/systemd/system/
sudo useradd -r -s /bin/false rpodder
sudo mkdir -p /var/lib/rpodder /usr/share/rpodder
sudo cp -r migrations /usr/share/rpodder/
sudo chown rpodder:rpodder /var/lib/rpodder
sudo systemctl enable --now rpodder
```

The default service file expects:

- Binary at `/usr/local/bin/rpodder`
- Data directory at `/var/lib/rpodder`
- Migrations at `/usr/share/rpodder/migrations`
- Config at `/etc/rpodder/config.toml` (optional)

## Verify installation

Once running, check the health endpoint:

```bash
curl http://localhost:3005/health
```

```json
{
  "status": "ok",
  "version": "0.1.0",
  "database": "sqlite",
  "build_tag": "dev",
  "build_sha": "local"
}
```
