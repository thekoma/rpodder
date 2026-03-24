#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# rpodder integration test runner
#
# Builds (or reuses) the Docker image, starts rpodder + mock RSS server
# (via compose), runs API tests, cleans up.
#
# Usage:
#   ./tests/docker/run-integration.sh sqlite              # build + test SQLite
#   ./tests/docker/run-integration.sh postgres             # build + test PostgreSQL
#   ./tests/docker/run-integration.sh all                  # both (default)
#
# Environment:
#   RPODDER_IMAGE  — use a pre-built image instead of building (skips build)
#                    e.g. RPODDER_IMAGE=ghcr.io/thekoma/rpodder@sha256:abc...
# ---------------------------------------------------------------------------
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$PROJECT_DIR"

PROFILES="${1:-all}"
EXIT_CODE=0

run_profile() {
  local profile="$1"
  local compose_profiles service

  case "$profile" in
    sqlite)   compose_profiles="--profile sqlite --profile test";  service="rpodder-sqlite"  ;;
    postgres) compose_profiles="--profile release --profile test"; service="rpodder-release" ;;
    *) echo "Unknown profile: $profile"; exit 1 ;;
  esac

  echo ""
  echo "============================================"
  echo "  Integration tests: $profile"
  echo "============================================"

  # Clean slate
  docker compose $compose_profiles down -v --remove-orphans 2>/dev/null || true

  # Build test runner image
  echo "Building test runner..."
  docker build -q -t rpodder-tests:latest tests/docker/ >/dev/null

  # Start rpodder + mock-rss
  if [ -n "${RPODDER_IMAGE:-}" ]; then
    echo "Using pre-built image: $RPODDER_IMAGE"
    docker pull "$RPODDER_IMAGE" 2>/dev/null || true
    # Build only ancillary services (mock-rss, postgres), not rpodder itself
    echo "Building ancillary services..."
    RPODDER_IMAGE="$RPODDER_IMAGE" \
      docker compose $compose_profiles build --quiet mock-rss 2>/dev/null || true
    echo "Starting rpodder ($profile) + mock-rss..."
    RPODDER_IMAGE="$RPODDER_IMAGE" \
      docker compose $compose_profiles up -d --no-build --quiet-pull 2>&1 | grep -v "^#" || true
  else
    echo "Starting rpodder ($profile) + mock-rss..."
    docker compose $compose_profiles up -d --build --quiet-pull 2>&1 | grep -v "^#" || true
  fi

  # Wait for mock-rss healthy
  echo -n "Waiting for mock-rss..."
  for i in $(seq 1 30); do
    if curl -sf http://localhost:8888/stats >/dev/null 2>&1; then
      echo " ready (${i}s)"
      break
    fi
    if [ "$i" -eq 30 ]; then echo " TIMEOUT"; fi
    sleep 1
  done

  # Wait for rpodder healthy
  echo -n "Waiting for rpodder..."
  for i in $(seq 1 60); do
    if curl -sf http://localhost:3005/health >/dev/null 2>&1; then
      echo " ready (${i}s)"
      break
    fi
    if [ "$i" -eq 60 ]; then
      echo " TIMEOUT"
      docker compose $compose_profiles logs 2>&1 | tail -20
      docker compose $compose_profiles down -v 2>/dev/null || true
      return 1
    fi
    sleep 1
  done

  # Create test user as admin (needed for feed update trigger)
  docker compose $compose_profiles exec -T "$service" \
    rpodder user create testuser testpass --admin 2>/dev/null || true

  # Get docker network name for test container to join
  local network
  network=$(docker compose $compose_profiles ps --format json | head -1 | jq -r '.Networks' 2>/dev/null || echo "")
  local network_flag=""
  if [ -n "$network" ]; then
    network_flag="--network=$network"
  fi

  # Run tests — connect to compose network so mock-rss is reachable from rpodder
  docker run --rm \
    --add-host=host.docker.internal:host-gateway \
    $network_flag \
    -e RPODDER_URL=http://host.docker.internal:3005 \
    -e MOCK_URL=http://mock-rss:8888 \
    -e MOCK_FEED_HOST=http://mock-rss:8888 \
    rpodder-tests:latest
  local rc=$?

  # Cleanup
  docker compose $compose_profiles down -v --remove-orphans 2>/dev/null || true

  return $rc
}

case "$PROFILES" in
  all)
    run_profile sqlite   || EXIT_CODE=1
    run_profile postgres || EXIT_CODE=1
    ;;
  sqlite|postgres)
    run_profile "$PROFILES" || EXIT_CODE=1
    ;;
  *)
    echo "Usage: $0 [sqlite|postgres|all]"
    exit 1
    ;;
esac

exit $EXIT_CODE
