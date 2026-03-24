#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# rpodder integration test runner
#
# Builds the Docker image, starts rpodder + mock RSS server (via compose),
# runs API tests, cleans up.
#
# Usage:
#   ./tests/docker/run-integration.sh sqlite     # test SQLite profile
#   ./tests/docker/run-integration.sh postgres    # test PostgreSQL profile
#   ./tests/docker/run-integration.sh all         # both (default)
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
  echo "Starting rpodder ($profile) + mock-rss..."
  docker compose $compose_profiles up -d --build --quiet-pull 2>&1 | grep -v "^#" || true

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
