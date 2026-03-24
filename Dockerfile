# syntax=docker/dockerfile:1

# --------------------------------------------------------------------------
# Stage 1: Build frontend (Svelte + Tailwind)
# Force amd64: output is pure static HTML/JS/CSS, identical for all arches.
# Avoids running rolldown native bindings under QEMU (arm64 build fails).
# --------------------------------------------------------------------------
FROM --platform=linux/amd64 oven/bun:1 AS frontend

WORKDIR /web
COPY web/package.json web/bun.lock* ./
RUN bun install --frozen-lockfile
COPY web/ .
RUN bun run build

# --------------------------------------------------------------------------
# Stage 2: Build backend (Rust)
# --------------------------------------------------------------------------
FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends mold clang && rm -rf /var/lib/apt/lists/*

ARG RPODDER_BUILD_TAG=dev
ARG RPODDER_BUILD_SHA=unknown

WORKDIR /build

ENV RUSTFLAGS="-C linker=clang -C link-arg=--ld-path=/usr/bin/mold"
ENV RPODDER_BUILD_TAG=${RPODDER_BUILD_TAG}
ENV RPODDER_BUILD_SHA=${RPODDER_BUILD_SHA}

# Copy everything needed for the build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY --from=frontend /web/dist/ web/dist/

# Build with cargo cache mounts for registry + target dir
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build --release --bin rpodder \
    && cp target/release/rpodder /usr/local/bin/rpodder

# --------------------------------------------------------------------------
# Stage 3: Runtime
# --------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

ARG RPODDER_BUILD_TAG=dev
ARG RPODDER_BUILD_SHA=unknown

LABEL org.opencontainers.image.title="rpodder"
LABEL org.opencontainers.image.description="A modern, self-hostable gpodder.net replacement written in Rust"
LABEL org.opencontainers.image.url="https://thekoma.github.io/rpodder/"
LABEL org.opencontainers.image.source="https://github.com/thekoma/rpodder"
LABEL org.opencontainers.image.documentation="https://thekoma.github.io/rpodder/"
LABEL org.opencontainers.image.vendor="thekoma"
LABEL org.opencontainers.image.licenses="AGPL-3.0-or-later"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 curl \
 && rm -rf /var/lib/apt/lists/*

# Create non-root user for runtime
RUN groupadd --gid 1000 rpodder && \
    useradd --uid 1000 --gid rpodder --shell /bin/false --create-home rpodder

COPY --from=builder /usr/local/bin/rpodder /usr/local/bin/rpodder

# Create data directory for SQLite mode (volume mount target)
RUN mkdir -p /app/data && chown rpodder:rpodder /app/data

# Migrations are needed at runtime for the CLI `rpodder migrate` command
COPY --chown=rpodder:rpodder migrations/ /app/migrations/

# Logo for container management UIs (Portainer, Dockge, etc.)
COPY --chown=rpodder:rpodder web/static/logo.svg /app/logo.svg

# Build info available at runtime
ENV RPODDER_BUILD_TAG=${RPODDER_BUILD_TAG}
ENV RPODDER_BUILD_SHA=${RPODDER_BUILD_SHA}

WORKDIR /app

USER rpodder

EXPOSE 3005
EXPOSE 9091

ENTRYPOINT ["rpodder"]
CMD ["serve"]
