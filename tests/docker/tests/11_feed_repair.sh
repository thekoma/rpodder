# Feed repair — 2026.10.1-2026.10.2 missed enclosures that followed iTunes tags
# and stored those items under their <link> page. Once the enclosure is read,
# the page copy goes away; text-only items (no enclosure) stay.
#
# Requires mock-rss service in docker-compose (profile: test).

MOCK_URL="${MOCK_URL:-http://host.docker.internal:8888}"
MOCK_FEED_HOST="${MOCK_FEED_HOST:-http://mock-rss:8888}"
MOCK_PUBLIC="http://host.docker.internal:8888"
REPAIR_FEED="${MOCK_FEED_HOST}/feed/repaired"

if ! curl -sf "${MOCK_URL}/stats" >/dev/null 2>&1; then
  skip "feed repair tests" "mock RSS server not running on ${MOCK_URL}"
  return 0
fi
curl -sf "${MOCK_URL}/reset" >/dev/null

REPAIR_ENCODED=$(echo -n "$REPAIR_FEED" | jq -sRr @uri)
repair_refresh() {
  http_post_body "${RPODDER_URL}/api/admin/feeds/update/single?url=${REPAIR_ENCODED}" -u "$AUTH_HEADER"
}
repair_episodes() {
  http_body "${RPODDER_URL}/api/2/data/podcast/episodes.json?url=${REPAIR_ENCODED}" -u "$AUTH_HEADER"
}
repair_url_of() {
  echo "$1" | jq -r ".episodes[] | select(.title == \"$2\") | .url"
}

# Before: the item is stored under its web page (as the faulty versions did).
result=$(repair_refresh)
assert_json "repair: first read" "$result" '.status' "updated"
body=$(repair_episodes)
assert_json_len "repair: two episodes before" "$body" '.episodes' "2"
ep1=$(repair_url_of "$body" "Rep ep1")
if [ "$ep1" = "${MOCK_PUBLIC}/page/rep1" ]; then
  pass "repair: ep1 under its web page before"
else
  fail "repair: ep1 under its web page before" "$ep1"
fi

# The enclosure is now recognised.
curl -sf "${MOCK_URL}/feed/repaired/fix" >/dev/null
result=$(repair_refresh)
assert_json "repair: second read" "$result" '.status' "updated"
assert_json "repair: ep1 found under its enclosure" "$result" '.new_episodes' "1"
body=$(repair_episodes)
assert_json_len "repair: still two episodes (page copy removed)" "$body" '.episodes' "2"
ep1=$(repair_url_of "$body" "Rep ep1")
if [ "$ep1" = "${MOCK_PUBLIC}/audio/rep1.mp3" ]; then
  pass "repair: ep1 plays its enclosure"
else
  fail "repair: ep1 plays its enclosure" "$ep1"
fi
ep2=$(repair_url_of "$body" "Rep ep2")
if [ "$ep2" = "${MOCK_PUBLIC}/page/rep2" ]; then
  pass "repair: text-only ep2 kept"
else
  fail "repair: text-only ep2 kept" "$ep2"
fi

# Nothing churns on the next read.
result=$(repair_refresh)
assert_json "repair: third read finds nothing new" "$result" '.new_episodes' "0"
assert_json_len "repair: two episodes after" "$(repair_episodes)" '.episodes' "2"
