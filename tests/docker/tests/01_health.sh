# Health and basic connectivity

status=$(http_status "${RPODDER_URL}/health")
assert_status "GET /health returns 200" "200" "$status"

body=$(http_body "${RPODDER_URL}/health")
assert_json "health status is ok" "$body" '.status' "ok"

# Unauthenticated access should fail
status=$(http_status "${RPODDER_URL}/api/2/devices/nobody.json")
assert_status "unauthenticated request returns 401" "401" "$status"
