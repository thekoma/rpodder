# Docker

rpodder provides a multi-stage Dockerfile and a `docker-compose.yml` with several profiles for different use cases.

## Self-hosting (published image)

The quickest way to run rpodder is the published image
[`ghcr.io/thekoma/rpodder`](https://ghcr.io/thekoma/rpodder) — no build, no clone.
Ready-to-use compose files live in [`examples/`](https://github.com/thekoma/rpodder/tree/main/examples)
and import cleanly into **Dockge**, **Portainer** and **CasaOS**.

=== "SQLite (simplest)"

    ```bash
    docker compose -f examples/docker-compose.yml up -d
    docker compose -f examples/docker-compose.yml exec rpodder \
      rpodder user create <name> <password> --admin
    ```

    One container, one volume. Ideal for a single user or small household.

=== "PostgreSQL (multi-user)"

    ```bash
    cp examples/.env.example examples/.env   # then set POSTGRES_PASSWORD
    docker compose -f examples/docker-compose.postgres.yml up -d
    docker compose -f examples/docker-compose.postgres.yml exec rpodder \
      rpodder user create <name> <password> --admin
    ```

Then point your podcast app at `http://<host>:3005`.

!!! tip "Pin a version"
    The examples track `:latest`. For reproducible deployments pin a release tag,
    e.g. `ghcr.io/thekoma/rpodder:2026.05.1`.

## Docker Compose profiles

!!! note "Development / from-source"
    The profiles below build from this checkout and are aimed at development and
    testing. For production self-hosting use the [published image](#self-hosting-published-image) above.

### Default (development)

```bash
docker compose up -d
```

Starts PostgreSQL + rpodder in development mode with `cargo-watch` for live reload. Source code is bind-mounted so changes are picked up automatically.

- rpodder: `http://localhost:3006` (mapped from container port 3005)
- PostgreSQL: `localhost:5432`

### Release (PostgreSQL)

```bash
docker compose --profile release up -d
```

Production-like build with the full release binary + embedded web UI. Good for testing before deployment.

### SQLite

```bash
docker compose --profile sqlite up -d
docker compose exec rpodder-sqlite rpodder user create admin admin --admin
```

Self-contained mode — no external database needed. Data stored in a Docker volume.

### Standalone frontend

```bash
docker compose --profile frontend up -d
```

Runs the Svelte SPA in a separate nginx container, with the API backend as a separate service. Useful for custom frontend deployments.

## Environment files

Sensitive configuration (OAuth secrets, API keys) can be stored in `.env.oauth`:

```bash
# .env.oauth (gitignored)
RPODDER_OAUTH_ISSUER_URL=https://sso.example.com/application/o/rpodder/
RPODDER_OAUTH_CLIENT_ID=your-client-id
RPODDER_OAUTH_CLIENT_SECRET=your-client-secret
RPODDER_OAUTH_ADMIN_GROUP=admins
RPODDER_BASE_URL=http://localhost:3006
RPODDER_PODCASTINDEX_KEY=your-key
RPODDER_PODCASTINDEX_SECRET=your-secret
```

Both the `rpodder` (dev) and `rpodder-release` services load this file automatically (with `required: false`, so it's optional).

## Dockerfile details

The release Dockerfile is a multi-stage build:

1. **Frontend stage** (`oven/bun:1`): installs dependencies, builds Svelte SPA to `web/dist/`
2. **Backend stage** (`rust:bookworm`): compiles Rust binary with `mold` linker for faster builds. Dependencies are cached in a separate layer
3. **Runtime stage** (`debian:bookworm-slim`): ~80MB final image with just the binary, CA certificates, and migrations

```bash
# Build with custom tags
docker build \
  --build-arg RPODDER_BUILD_TAG=v1.0.0 \
  --build-arg RPODDER_BUILD_SHA=$(git rev-parse --short HEAD) \
  -t rpodder .
```

## Volumes

!!! danger "Never destroy volumes without checking"
    The `pgdata` and `sqlite-data` volumes contain your podcast database. Never use `docker compose down -v` unless you want to lose all data.

| Volume | Profile | Purpose |
|--------|---------|---------|
| `pgdata` | default, release | PostgreSQL data |
| `sqlite-data` | sqlite | SQLite database file |
| `cargo-cache` | default (dev) | Rust dependency cache |
| `target-cache` | default (dev) | Rust build cache |

## CLI inside Docker

```bash
# Create a user (release profile)
docker compose --profile release exec rpodder-release rpodder user create myuser mypass --admin

# Run migrations manually
docker compose --profile release exec rpodder-release rpodder migrate

# Repair FTS5 index (SQLite only)
docker compose --profile sqlite exec rpodder-sqlite rpodder repair
```
