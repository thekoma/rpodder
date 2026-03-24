# Feed updater — ETag, content_hash, episode creation, change detection
#
# Requires mock-rss service in docker-compose (profile: test).

MOCK_URL="${MOCK_URL:-http://host.docker.internal:8888}"
MOCK_FEED_HOST="${MOCK_FEED_HOST:-http://mock-rss:8888}"
DEVICE="testphone"
STATIC_FEED="${MOCK_FEED_HOST}/feed/static"
DYNAMIC_FEED="${MOCK_FEED_HOST}/feed/dynamic"
ADMIN_URL="${RPODDER_URL}/api/admin/feeds/update/single"

# Check mock server is reachable
if ! curl -sf "${MOCK_URL}/stats" >/dev/null 2>&1; then
  skip "feed updater tests" "mock RSS server not running on ${MOCK_URL}"
  return 0
fi

# Reset mock state
curl -sf "${MOCK_URL}/reset" >/dev/null

# URL-encode feed URLs for query parameter
STATIC_ENCODED=$(echo -n "$STATIC_FEED" | jq -sRr @uri)
DYNAMIC_ENCODED=$(echo -n "$DYNAMIC_FEED" | jq -sRr @uri)

# --- Subscribe to both mock feeds ---
body=$(http_post_body "${RPODDER_URL}/api/2/subscriptions/${RPODDER_USER}/${DEVICE}.json" \
  -u "$AUTH_HEADER" \
  -d "{\"add\":[\"${STATIC_FEED}\",\"${DYNAMIC_FEED}\"],\"remove\":[]}")
assert_json "subscribe to mock feeds" "$body" '.timestamp | type' "number"

# --- Trigger first feed update ---
status=$(http_post_status "${ADMIN_URL}?url=${STATIC_ENCODED}" -u "$AUTH_HEADER")
assert_status "trigger static feed update" "200" "$status"
status=$(http_post_status "${ADMIN_URL}?url=${DYNAMIC_ENCODED}" -u "$AUTH_HEADER")
assert_status "trigger dynamic feed update" "200" "$status"

# Wait for mock to be hit (polling, no fixed sleep)
wait_mock_stat "static feed fetched" "$MOCK_URL" '.static_fetched' "1"
wait_mock_stat "dynamic feed fetched" "$MOCK_URL" '.dynamic_fetched' "1"

# Verify podcast metadata was updated (poll toplist until title appears)
_check_toplist_has() {
  local title="$1"
  local body
  body=$(curl -sf "${RPODDER_URL}/toplist/50.json" 2>/dev/null)
  echo "$body" | jq -e ".[] | select(.title == \"$title\")" >/dev/null 2>&1
}
wait_until "static podcast in toplist" _check_toplist_has "Static Test Podcast" && pass "static podcast metadata updated"
wait_until "dynamic podcast in toplist" _check_toplist_has "Dynamic Test Podcast" && pass "dynamic podcast metadata updated"

# --- Trigger second feed update ---
http_post_status "${ADMIN_URL}?url=${STATIC_ENCODED}" -u "$AUTH_HEADER" >/dev/null

# Static: should get 304 (ETag saved from first fetch)
wait_mock_stat "static feed: 304 (ETag works)" "$MOCK_URL" '.static_304' "1"
# Verify no additional full fetch happened
mock_stats=$(curl -sf "${MOCK_URL}/stats")
assert_json "static feed: still 1 full fetch" "$mock_stats" '.static_fetched' "1"

# Dynamic: no ETag, should fetch again (2nd episode added)
http_post_status "${ADMIN_URL}?url=${DYNAMIC_ENCODED}" -u "$AUTH_HEADER" >/dev/null
wait_mock_stat "dynamic feed: 2 fetches" "$MOCK_URL" '.dynamic_fetched' "2"

# After 2 dynamic fetches: 1st served 1 ep, 2nd served 2 eps → state now 3
mock_stats=$(curl -sf "${MOCK_URL}/stats")
assert_json "dynamic feed: state incremented" "$mock_stats" '.dynamic_episode_count' "3"
