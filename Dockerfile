# --------------------------------------------------------------------------
# Stage 1: Build frontend (Svelte + Tailwind)
# --------------------------------------------------------------------------
FROM oven/bun:1 AS frontend

WORKDIR /web
COPY web/package.json ./
RUN bun install
COPY web/ .
RUN bun run build

# --------------------------------------------------------------------------
# Stage 2: Build backend (Rust)
# --------------------------------------------------------------------------
FROM rust:bookworm AS builder

WORKDIR /build

# Cache dependencies: copy only manifests first, build a dummy to prime cache
COPY Cargo.toml Cargo.lock ./
COPY crates/rpodder-core/Cargo.toml   crates/rpodder-core/Cargo.toml
COPY crates/rpodder-db/Cargo.toml     crates/rpodder-db/Cargo.toml
COPY crates/rpodder-feed/Cargo.toml   crates/rpodder-feed/Cargo.toml
COPY crates/rpodder-server/Cargo.toml crates/rpodder-server/Cargo.toml

# Create dummy lib/main files so cargo can resolve the workspace
RUN mkdir -p crates/rpodder-core/src   && echo "" > crates/rpodder-core/src/lib.rs \
 && mkdir -p crates/rpodder-db/src     && echo "" > crates/rpodder-db/src/lib.rs \
 && mkdir -p crates/rpodder-feed/src   && echo "" > crates/rpodder-feed/src/lib.rs \
 && mkdir -p crates/rpodder-server/src && echo "fn main() {}" > crates/rpodder-server/src/main.rs

# Create empty web/dist so rust-embed compiles during dep caching
RUN mkdir -p web/dist && touch web/dist/index.html

RUN cargo build --release 2>/dev/null || true

# Now copy real source and built frontend
COPY crates/ crates/
COPY --from=frontend /web/dist/ web/dist/

# Touch files so cargo detects changes vs the dummy sources
RUN touch crates/rpodder-core/src/lib.rs \
          crates/rpodder-db/src/lib.rs \
          crates/rpodder-feed/src/lib.rs \
          crates/rpodder-server/src/main.rs

RUN cargo build --release --bin rpodder

# --------------------------------------------------------------------------
# Stage 3: Runtime
# --------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/rpodder /usr/local/bin/rpodder

# Migrations are needed at runtime for the CLI `rpodder migrate` command
COPY migrations/ /app/migrations/

WORKDIR /app

EXPOSE 3005

ENTRYPOINT ["rpodder"]
CMD ["serve"]
