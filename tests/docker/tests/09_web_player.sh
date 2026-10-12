# Web player API — media URLs in episodes.json, /api/2/me/in-progress, /api/2/me/positions
#
# Requires mock-rss service in docker-compose (profile: test).

MOCK_URL="${MOCK_URL:-http://host.docker.internal:8888}"
MOCK_FEED_HOST="${MOCK_FEED_HOST:-http://mock-rss:8888}"
# Base URL mock-rss writes into /feed/player enclosures (its MOCK_PUBLIC_URL default)
MOCK_PUBLIC="http://host.docker.internal:8888"
PLAYER_FEED="${MOCK_FEED_HOST}/feed/player"
DEVICE="e2e-phone"

if ! curl -sf "${MOCK_URL}/stats" >/dev/null 2>&1; then
  skip "web player tests" "mock RSS server not running on ${MOCK_URL}"
  return 0
fi

FEED_ENCODED=$(echo -n "$PLAYER_FEED" | jq -sRr @uri)

# --- Subscribe and fetch the player feed ---
http_post_body "${RPODDER_URL}/api/2/subscriptions/${RPODDER_USER}/${DEVICE}.json" \
  -u "$AUTH_HEADER" -d "{\"add\":[\"${PLAYER_FEED}\"],\"remove\":[]}" >/dev/null
http_post_status "${RPODDER_URL}/api/admin/feeds/update/single?url=${FEED_ENCODED}" \
  -u "$AUTH_HEADER" >/dev/null

player_episodes() {
  http_body "${RPODDER_URL}/api/2/data/podcast/episodes.json?url=${FEED_ENCODED}" -u "$AUTH_HEADER"
}
# A burst of on-demand fetches (the podcast is still untitled) racing the
# admin update above: concurrent episode creation must not fail or duplicate.
BURST_DIR=$(mktemp -d)
pids=""
for i in 1 2 3 4 5; do
  http_status "${RPODDER_URL}/api/2/data/podcast/episodes.json?url=${FEED_ENCODED}" \
    -u "$AUTH_HEADER" >"${BURST_DIR}/${i}" &
  pids="$pids $!"
done
# shellcheck disable=SC2086
wait $pids
for i in 1 2 3 4 5; do
  assert_status "concurrent on-demand fetch ${i}" "200" "$(cat "${BURST_DIR}/${i}")"
done
rm -rf "$BURST_DIR"

episodes_ready() {
  [ "$(player_episodes | jq '[.episodes[] | select((.title | startswith("Player ep")) and .url != null)] | length' 2>/dev/null)" = "3" ]
}
wait_until "player feed episodes indexed" episodes_ready

body=$(player_episodes)
# The concurrent fetches above may not leave an untitled duplicate row
# without a URL behind.
assert_json_len "no orphan episodes from concurrent fetches" "$body" '.episodes' "3"
EP1_URL=$(echo "$body" | jq -r '.episodes[] | select(.title == "Player ep1") | .url')
assert_json "ep1 url is the WAV enclosure" "$body" \
  '.episodes[] | select(.title == "Player ep1") | .url' "${MOCK_PUBLIC}/media/tone.wav"
assert_json "ep1 is media" "$body" '.episodes[] | select(.title == "Player ep1") | .media' "true"
assert_json "ep1 mimetype" "$body" '.episodes[] | select(.title == "Player ep1") | .mimetype' "audio/wav"
assert_json "ep2 is media" "$body" '.episodes[] | select(.title == "Player ep2") | .media' "true"
assert_json "ep3 url is its page" "$body" \
  '.episodes[] | select(.title == "Player ep3") | .url' "${MOCK_PUBLIC}/page/ep3"
assert_json "ep3 is not media" "$body" '.episodes[] | select(.title == "Player ep3") | .media' "false"

# --- Play actions covering the in-progress rules ---
play() { # episode position total timestamp
  echo "{\"podcast\":\"${PLAYER_FEED}\",\"episode\":\"$1\",\"device\":\"${DEVICE}\",\"action\":\"play\",\"started\":0,\"position\":$2,\"total\":$3,\"timestamp\":\"$4\"}"
}
other() { # episode action timestamp
  echo "{\"podcast\":\"${PLAYER_FEED}\",\"episode\":\"$1\",\"device\":\"${DEVICE}\",\"action\":\"$2\",\"timestamp\":\"$3\"}"
}
FINISHED="${MOCK_PUBLIC}/media/case-finished.mp3"
ZERO="${MOCK_PUBLIC}/media/case-zero.mp3"
DOWNLOAD="${MOCK_PUBLIC}/media/case-download.mp3"
RESET="${MOCK_PUBLIC}/media/case-reset.mp3"

status=$(http_post_status "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json" -u "$AUTH_HEADER" -d "[
  $(play "$EP1_URL" 100 180 2026-01-01T10:00:00),
  $(play "$EP1_URL" 120 180 2026-01-01T11:00:00),
  $(play "$FINISHED" 175 180 2026-01-01T10:00:00),
  $(play "$ZERO" 0 180 2026-01-01T10:00:00),
  $(other "$DOWNLOAD" download 2026-01-01T10:00:00),
  $(play "$RESET" 60 180 2026-01-01T10:00:00),
  $(other "$RESET" new 2026-01-01T12:00:00)
]")
assert_status "upload player actions" "200" "$status"

body=$(http_body "${RPODDER_URL}/api/2/me/in-progress?limit=50" -u "$AUTH_HEADER")
mine=$(echo "$body" | jq -c --arg f "$PLAYER_FEED" '[.[] | select(.podcast_url == $f)]')
assert_json_len "in-progress: only the unfinished episode" "$mine" '.' "1"
assert_json "in-progress: latest position" "$mine" '.[0].position' "120"
assert_json "in-progress: episode url" "$mine" '.[0].episode_url' "$EP1_URL"
assert_json "in-progress: episode title" "$mine" '.[0].episode_title' "Player ep1"
assert_json "in-progress: device" "$mine" '.[0].device' "$DEVICE"

body=$(http_body "${RPODDER_URL}/api/2/me/in-progress?limit=1" -u "$AUTH_HEADER")
assert_json_len "in-progress: limit=1" "$body" '.' "1"

body=$(http_body "${RPODDER_URL}/api/2/me/positions?podcast=${FEED_ENCODED}" -u "$AUTH_HEADER")
assert_json "positions: ep1 latest" "$body" \
  "[.[] | select(.episode_url == \"${EP1_URL}\")][0].position" "120"
assert_json "positions: finished episode included" "$body" \
  "[.[] | select(.episode_url == \"${FINISHED}\")][0].position" "175"
assert_json "positions: reset episode keeps its play" "$body" \
  "[.[] | select(.episode_url == \"${RESET}\")][0].position" "60"
assert_json "positions: download-only episode absent" "$body" \
  "[.[] | select(.episode_url == \"${DOWNLOAD}\")] | length" "0"

status=$(http_status "${RPODDER_URL}/api/2/me/in-progress")
assert_status "in-progress requires auth" "401" "$status"

body=$(http_body "${RPODDER_URL}/api/2/devices/${RPODDER_USER}.json" -u "$AUTH_HEADER")
assert_json "uploading device listed" "$body" "[.[] | select(.id == \"${DEVICE}\")] | length" "1"
