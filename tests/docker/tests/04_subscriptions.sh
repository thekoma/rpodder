# Subscriptions — subscribe, idempotent, list, unsubscribe

DEVICE="testphone"
FEED1="https://feeds.simplecast.com/54nAGcIl"
FEED2="https://changelog.com/podcast/feed"

# Subscribe to 2 podcasts
body=$(http_post_body "${RPODDER_URL}/api/2/subscriptions/${RPODDER_USER}/${DEVICE}.json" \
  -u "$AUTH_HEADER" -d "{\"add\":[\"${FEED1}\",\"${FEED2}\"],\"remove\":[]}")
assert_json "subscribe returns timestamp" "$body" '.timestamp | type' "number"

# Idempotent subscribe (should still be 200, no duplicate changes)
status=$(http_post_status "${RPODDER_URL}/api/2/subscriptions/${RPODDER_USER}/${DEVICE}.json" \
  -u "$AUTH_HEADER" -d "{\"add\":[\"${FEED1}\"],\"remove\":[]}")
assert_status "idempotent subscribe returns 200" "200" "$status"

# Get subscriptions
body=$(http_body "${RPODDER_URL}/api/2/subscriptions/${RPODDER_USER}/${DEVICE}.json" \
  -u "$AUTH_HEADER")
add_count=$(echo "$body" | jq '.add | length' 2>/dev/null)
if [ "$add_count" -ge 2 ] 2>/dev/null; then pass "subscriptions list has >= 2 entries"
else fail "subscriptions list has >= 2 entries" "got $add_count"; fi

# Unsubscribe
body=$(http_post_body "${RPODDER_URL}/api/2/subscriptions/${RPODDER_USER}/${DEVICE}.json" \
  -u "$AUTH_HEADER" -d "{\"add\":[],\"remove\":[\"${FEED2}\"]}")
assert_json "unsubscribe returns timestamp" "$body" '.timestamp | type' "number"

# Verify unsubscribe worked — check delta since=0
body=$(http_body "${RPODDER_URL}/api/2/subscriptions/${RPODDER_USER}/${DEVICE}.json?since=0" \
  -u "$AUTH_HEADER")
# The add list should contain only FEED1 (net result after sub+unsub)
assert_json "after unsubscribe, FEED1 still in add" "$body" '.add[0]' "$FEED1"
