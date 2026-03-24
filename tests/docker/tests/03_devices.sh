# Device management — create, list, update
# Note: gpodder device POST returns 200 with empty body

# Create device
status=$(http_post_status "${RPODDER_URL}/api/2/devices/${RPODDER_USER}/testphone.json" \
  -u "$AUTH_HEADER" -d '{"caption":"Test Phone","type":"mobile"}')
assert_status "create device returns 200" "200" "$status"

# List devices and verify created device
body=$(http_body "${RPODDER_URL}/api/2/devices/${RPODDER_USER}.json" -u "$AUTH_HEADER")
count=$(echo "$body" | jq 'length' 2>/dev/null)
if [ "$count" -ge 1 ] 2>/dev/null; then pass "list devices has >= 1 entry"
else fail "list devices has >= 1 entry" "got $count"; fi

# Find our device in the list
caption=$(echo "$body" | jq -r '.[] | select(.id == "testphone") | .caption' 2>/dev/null)
assert_json "device caption is Test Phone" "$(echo "$body" | jq '.[] | select(.id == "testphone")')" '.caption' "Test Phone"

# Update device
status=$(http_post_status "${RPODDER_URL}/api/2/devices/${RPODDER_USER}/testphone.json" \
  -u "$AUTH_HEADER" -d '{"caption":"My Phone","type":"mobile"}')
assert_status "update device returns 200" "200" "$status"

# Verify update
body=$(http_body "${RPODDER_URL}/api/2/devices/${RPODDER_USER}.json" -u "$AUTH_HEADER")
assert_json "updated device caption" "$(echo "$body" | jq '.[] | select(.id == "testphone")')" '.caption' "My Phone"
