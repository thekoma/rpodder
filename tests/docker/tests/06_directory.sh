# Directory — toplist, search, tags, suggestions

# Toplist
status=$(http_status "${RPODDER_URL}/toplist/10.json")
assert_status "GET toplist returns 200" "200" "$status"

# Search
status=$(http_status "${RPODDER_URL}/search.json?q=test")
assert_status "GET search returns 200" "200" "$status"

# Tags
status=$(http_status "${RPODDER_URL}/api/2/tags/5.json")
assert_status "GET tags returns 200" "200" "$status"

# Suggestions (requires auth)
status=$(http_status "${RPODDER_URL}/api/2/suggestions/5.json" -u "$AUTH_HEADER")
assert_status "GET suggestions returns 200" "200" "$status"
