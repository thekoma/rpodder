#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# rpodder integration test runner
#
# Runs all test scripts in /tests/ (except this file) in sorted order.
# Optionally pass test names as args to run specific tests:
#   run-all.sh health subscriptions
#
# Environment:
#   RPODDER_URL   — base URL of the rpodder instance (default: http://host.docker.internal:3005)
#   RPODDER_USER  — test username (created by pre-hook)
#   RPODDER_PASS  — test password
# ---------------------------------------------------------------------------
set -uo pipefail

export RPODDER_URL="${RPODDER_URL:-http://host.docker.internal:3005}"
export RPODDER_USER="${RPODDER_USER:-testuser}"
export RPODDER_PASS="${RPODDER_PASS:-testpass}"

PASS=0 FAIL=0 SKIP=0
FAILURES=()

# ---------------------------------------------------------------------------
# Helpers (sourced by test scripts)
# ---------------------------------------------------------------------------
_c() { printf '\033[%sm' "$1"; }
GREEN=$(_c 32) RED=$(_c 31) YELLOW=$(_c 33) CYAN=$(_c 36) BOLD=$(_c 1) NC=$(_c 0)

pass() { echo -e "  ${GREEN}PASS${NC} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  ${RED}FAIL${NC} $1${2:+ — $2}"; FAIL=$((FAIL + 1)); FAILURES+=("$1"); }
skip() { echo -e "  ${YELLOW}SKIP${NC} $1${2:+ — $2}"; SKIP=$((SKIP + 1)); }

# HTTP helpers — all use RPODDER_URL
export AUTH_HEADER="${RPODDER_USER}:${RPODDER_PASS}"

http_status() {
  curl -s -o /dev/null -w '%{http_code}' "$@" 2>/dev/null
}

http_body() {
  curl -sf "$@" 2>/dev/null
}

http_post_status() {
  local url="$1"; shift
  curl -s -o /dev/null -w '%{http_code}' -X POST \
    -H 'Content-Type: application/json' "$url" "$@" 2>/dev/null
}

http_post_body() {
  local url="$1"; shift
  curl -sf -X POST -H 'Content-Type: application/json' "$url" "$@" 2>/dev/null
}

http_put_status() {
  local url="$1"; shift
  curl -s -o /dev/null -w '%{http_code}' -X PUT \
    -H 'Content-Type: application/json' "$url" "$@" 2>/dev/null
}

http_put_body() {
  local url="$1"; shift
  curl -sf -X PUT -H 'Content-Type: application/json' "$url" "$@" 2>/dev/null
}

assert_status() {
  local label="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then pass "$label"
  else fail "$label" "expected HTTP $expected, got $actual"; fi
}

assert_json() {
  local label="$1" json="$2" expr="$3" expected="$4"
  local actual
  actual=$(echo "$json" | jq -r "$expr" 2>/dev/null)
  if [ "$actual" = "$expected" ]; then pass "$label"
  else fail "$label" "jq '$expr': expected '$expected', got '$actual'"; fi
}

assert_json_len() {
  local label="$1" json="$2" expr="$3" expected="$4"
  local actual
  actual=$(echo "$json" | jq "$expr | length" 2>/dev/null)
  if [ "$actual" = "$expected" ]; then pass "$label"
  else fail "$label" "jq '$expr | length': expected $expected, got $actual"; fi
}

# Poll until a condition is met (max 60s, incremental backoff: 1,2,3,4...)
# Usage: wait_until "label" command_that_returns_0_on_success
wait_until() {
  local label="$1"; shift
  local elapsed=0 delay=1
  while [ $elapsed -lt 60 ]; do
    if "$@" 2>/dev/null; then return 0; fi
    sleep "$delay"
    elapsed=$((elapsed + delay))
    delay=$((delay < 5 ? delay + 1 : 5))
  done
  fail "$label" "timed out after ${elapsed}s"
  return 1
}

# Poll mock stats until a field matches expected value
wait_mock_stat() {
  local label="$1" mock_url="$2" field="$3" expected="$4"
  wait_until "$label" sh -c "[ \"\$(curl -sf '${mock_url}/stats' | jq -r '${field}')\" = '${expected}' ]"
}

export -f pass fail skip http_status http_body http_post_status http_post_body
export -f http_put_status http_put_body assert_status assert_json assert_json_len
export -f wait_until wait_mock_stat
export GREEN RED YELLOW CYAN BOLD NC PASS FAIL SKIP

# ---------------------------------------------------------------------------
# Wait for rpodder to be ready
# ---------------------------------------------------------------------------
echo -e "${BOLD}rpodder integration tests${NC}"
echo -e "target: ${CYAN}${RPODDER_URL}${NC}"
echo ""

echo -n "waiting for rpodder..."
for i in $(seq 1 30); do
  if curl -sf "${RPODDER_URL}/health" >/dev/null 2>&1; then
    echo -e " ${GREEN}ready${NC} (${i}s)"
    break
  fi
  if [ "$i" -eq 30 ]; then
    echo -e " ${RED}timeout${NC}"
    exit 1
  fi
  sleep 1
done

# ---------------------------------------------------------------------------
# Discover and run test scripts
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
filter=("$@")

for test_file in "$SCRIPT_DIR"/[0-9]*.sh; do
  [ -f "$test_file" ] || continue
  test_name="$(basename "$test_file" .sh)"
  # Strip numeric prefix for display
  display_name="${test_name#[0-9]*_}"

  # If filter specified, skip non-matching tests
  if [ ${#filter[@]} -gt 0 ]; then
    match=false
    for f in "${filter[@]}"; do
      [[ "$display_name" == *"$f"* ]] && match=true
    done
    $match || continue
  fi

  echo -e "\n${BOLD}--- ${display_name} ---${NC}"
  source "$test_file"
done

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo -e "\n${BOLD}========================${NC}"
echo -e "Results: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}, ${YELLOW}${SKIP} skipped${NC}"

if [ ${#FAILURES[@]} -gt 0 ]; then
  echo -e "\n${RED}Failed:${NC}"
  for f in "${FAILURES[@]}"; do echo "  - $f"; done
  exit 1
fi

exit 0
