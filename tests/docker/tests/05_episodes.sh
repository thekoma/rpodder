# Episode actions — upload, download, deduplication

FEED="https://feeds.simplecast.com/54nAGcIl"
EP="https://example.com/ep1.mp3"

# Upload episode action
body=$(http_post_body "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json" \
  -u "$AUTH_HEADER" \
  -d "{\"actions\":[{\"podcast\":\"${FEED}\",\"episode\":\"${EP}\",\"action\":\"play\",\"position\":120,\"started\":0,\"total\":3600,\"timestamp\":\"2026-03-24T10:00:00\"}]}")
assert_json "upload action returns timestamp" "$body" '.timestamp | type' "number"

# Download episode actions
body=$(http_body "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json?since=0" \
  -u "$AUTH_HEADER")
count=$(echo "$body" | jq '.actions | length' 2>/dev/null)
if [ "$count" -ge 1 ] 2>/dev/null; then pass "download returns >= 1 action"
else fail "download returns >= 1 action" "got $count"; fi
assert_json "action type is play" "$body" '.actions[0].action' "play"
assert_json "action position is 120" "$body" '.actions[0].position' "120"

# Upload same action again (should not duplicate — ON CONFLICT)
http_post_body "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json" \
  -u "$AUTH_HEADER" \
  -d "{\"actions\":[{\"podcast\":\"${FEED}\",\"episode\":\"${EP}\",\"action\":\"play\",\"position\":120,\"started\":0,\"total\":3600,\"timestamp\":\"2026-03-24T10:00:00\"}]}" >/dev/null

before_count="$count"
body=$(http_body "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json?since=0" \
  -u "$AUTH_HEADER")
after_count=$(echo "$body" | jq '.actions | length' 2>/dev/null)
if [ "$after_count" = "$before_count" ]; then pass "duplicate action not created"
else fail "duplicate action not created" "was $before_count, now $after_count"; fi

# Upload with bare array (Kasts compat)
status=$(http_post_status "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json" \
  -u "$AUTH_HEADER" \
  -d "[{\"podcast\":\"${FEED}\",\"episode\":\"https://example.com/ep2.mp3\",\"action\":\"download\",\"timestamp\":\"2026-03-24T11:00:00\"}]")
assert_status "bare array upload returns 200" "200" "$status"

# Invalid action type
status=$(http_post_status "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json" \
  -u "$AUTH_HEADER" \
  -d "{\"actions\":[{\"podcast\":\"${FEED}\",\"episode\":\"${EP}\",\"action\":\"invalid\",\"timestamp\":\"2026-03-24T12:00:00\"}]}")
assert_status "invalid action type returns 400" "400" "$status"

# Play without position
status=$(http_post_status "${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json" \
  -u "$AUTH_HEADER" \
  -d "{\"actions\":[{\"podcast\":\"${FEED}\",\"episode\":\"${EP}\",\"action\":\"play\",\"timestamp\":\"2026-03-24T12:00:00\"}]}")
assert_status "play without position returns 400" "400" "$status"
