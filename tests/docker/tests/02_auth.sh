# Authentication — login, session cookie, wrong credentials

# Basic auth login
status=$(http_post_status "${RPODDER_URL}/api/2/auth/${RPODDER_USER}/login.json" \
  -u "$AUTH_HEADER")
assert_status "login with correct credentials" "200" "$status"

# Wrong password
status=$(http_post_status "${RPODDER_URL}/api/2/auth/${RPODDER_USER}/login.json" \
  -u "${RPODDER_USER}:wrongpass")
assert_status "login with wrong password returns 401" "401" "$status"

# Wrong username in path (should be 403)
status=$(http_post_status "${RPODDER_URL}/api/2/auth/otheruser/login.json" \
  -u "$AUTH_HEADER")
assert_status "login with wrong username in path returns 403" "403" "$status"
