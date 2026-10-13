# Episode-action sync between devices — since cursor, device field, filters, aggregation

FEED="https://example.com/sync-feed.xml"
EP1="https://example.com/sync-ep1.mp3"
EP2="https://example.com/sync-ep2.mp3"
URL="${RPODDER_URL}/api/2/episodes/${RPODDER_USER}.json"

play_action() { # episode device timestamp position
  echo "{\"podcast\":\"${FEED}\",\"episode\":\"$1\",\"device\":\"$2\",\"action\":\"play\",\"started\":0,\"position\":$4,\"total\":3600,\"timestamp\":\"$3\"}"
}

# Device B syncs and keeps the cursor
body=$(http_body "${URL}?podcast=${FEED}" -u "$AUTH_HEADER")
since=$(echo "$body" | jq '.timestamp')

# Device A uploads an action it recorded offline long ago
http_post_body "$URL" -u "$AUTH_HEADER" \
  -d "[$(play_action "$EP1" sync-a "2020-01-01T10:00:00" 100)]" >/dev/null

body=$(http_body "${URL}?since=${since}&podcast=${FEED}" -u "$AUTH_HEADER")
assert_json_len "late upload reaches device that already synced" "$body" '.actions' "1"
assert_json "action carries its device" "$body" '.actions[0].device' "sync-a"

# The uploading device is auto-registered
body=$(http_body "${RPODDER_URL}/api/2/devices/${RPODDER_USER}.json" -u "$AUTH_HEADER")
assert_json "device auto-created on upload" "$body" '[.[] | select(.id == "sync-a")] | length' "1"

# Re-upload with a new position (same timestamp) is delivered again
http_post_body "$URL" -u "$AUTH_HEADER" \
  -d "[$(play_action "$EP1" sync-a "2020-01-01T10:00:00" 200)]" >/dev/null
body=$(http_body "${URL}?since=${since}&podcast=${FEED}" -u "$AUTH_HEADER")
assert_json "updated position delivered" "$body" '[.actions[] | select(.position == 200)] | length' "1"

# Aggregated: only the latest action per episode
http_post_body "$URL" -u "$AUTH_HEADER" \
  -d "[$(play_action "$EP1" sync-b "2020-01-01T11:00:00" 300),$(play_action "$EP2" sync-a "2020-01-01T10:00:00" 50)]" >/dev/null
body=$(http_body "${URL}?podcast=${FEED}&aggregated=true" -u "$AUTH_HEADER")
assert_json_len "aggregated returns one action per episode" "$body" '.actions' "2"
assert_json "aggregated keeps latest position" "$body" "[.actions[] | select(.episode == \"${EP1}\")][0].position" "300"
assert_json "aggregated keeps latest device" "$body" "[.actions[] | select(.episode == \"${EP1}\")][0].device" "sync-b"

# Unknown filters return nothing (not everything)
body=$(http_body "${URL}?device=no-such-device" -u "$AUTH_HEADER")
assert_json_len "unknown device filter is empty" "$body" '.actions' "0"
body=$(http_body "${URL}?podcast=https://unknown.example/never-seen.xml" -u "$AUTH_HEADER")
assert_json_len "unknown podcast filter is empty" "$body" '.actions' "0"

# Invalid device id: upload still accepted, no device created
status=$(http_post_status "$URL" -u "$AUTH_HEADER" \
  -d "[$(play_action "$EP2" "$(printf 'x%.0s' $(seq 1 70))" "2020-01-01T12:00:00" 60)]")
assert_status "overlong device id does not fail upload" "200" "$status"

# History timestamps are ISO 8601 on both backends (the web UI parses them)
ts=$(http_body "${RPODDER_URL}/api/2/history/${RPODDER_USER}.json" -u "$AUTH_HEADER" | jq -r '.[0].timestamp')
if echo "$ts" | grep -Eq '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]+)?(Z|[+-][0-9]{2}:[0-9]{2})$'; then
  pass "history timestamp is ISO 8601"
else
  fail "history timestamp is ISO 8601" "got '$ts'"
fi
