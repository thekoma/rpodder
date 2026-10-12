# Session revocation — password changes and deactivation log out other clients.

REVOKE_USER="e2e-revoke"
JAR=$(mktemp)
JAR2=$(mktemp)

# Fresh user (admin endpoint; on a re-run reset its password instead)
status=$(http_post_status "${RPODDER_URL}/api/admin/users" -u "$AUTH_HEADER" \
  -d "{\"username\":\"${REVOKE_USER}\",\"password\":\"pass-one\"}")
if [ "$status" = "409" ]; then
  http_post_status "${RPODDER_URL}/api/admin/users/${REVOKE_USER}/password" -u "$AUTH_HEADER" \
    -d '{"password":"pass-one"}' >/dev/null
  http_post_status "${RPODDER_URL}/api/admin/users/${REVOKE_USER}/activate" -u "$AUTH_HEADER" >/dev/null
fi

devices_with() { # cookie-jar
  http_status "${RPODDER_URL}/api/2/devices/${REVOKE_USER}.json" -b "$1"
}

# --- Own password change (Basic Auth: no session to keep) ---
status=$(http_post_status "${RPODDER_URL}/api/2/auth/${REVOKE_USER}/login.json" \
  -u "${REVOKE_USER}:pass-one" -c "$JAR")
assert_status "revocation: login" "200" "$status"
assert_status "revocation: cookie works" "200" "$(devices_with "$JAR")"

status=$(http_post_status "${RPODDER_URL}/api/2/me/password" -u "${REVOKE_USER}:pass-one" \
  -d '{"old_password":"pass-one","new_password":"pass-two"}')
assert_status "revocation: change own password" "200" "$status"
assert_status "revocation: old cookie revoked by password change" "401" "$(devices_with "$JAR")"

status=$(http_status "${RPODDER_URL}/api/2/devices/${REVOKE_USER}.json" -u "${REVOKE_USER}:pass-two")
assert_status "revocation: new password works" "200" "$status"

# --- Own password change from a session: that session stays, others go ---
http_post_status "${RPODDER_URL}/api/2/auth/${REVOKE_USER}/login.json" \
  -u "${REVOKE_USER}:pass-two" -c "$JAR" >/dev/null
http_post_status "${RPODDER_URL}/api/2/auth/${REVOKE_USER}/login.json" \
  -u "${REVOKE_USER}:pass-two" -c "$JAR2" >/dev/null
status=$(http_post_status "${RPODDER_URL}/api/2/me/password" -b "$JAR" \
  -d '{"old_password":"pass-two","new_password":"pass-three"}')
assert_status "revocation: change own password from a session" "200" "$status"
assert_status "revocation: the changing session stays" "200" "$(devices_with "$JAR")"
assert_status "revocation: the other session is revoked" "401" "$(devices_with "$JAR2")"

# --- Password change without the current one (not an SSO session) ---
status=$(http_post_status "${RPODDER_URL}/api/2/me/password" -u "${REVOKE_USER}:pass-three" \
  -d '{"new_password":"pass-four"}')
assert_status "revocation: current password required" "403" "$status"

# --- Deactivate + activate ---
http_post_status "${RPODDER_URL}/api/2/auth/${REVOKE_USER}/login.json" \
  -u "${REVOKE_USER}:pass-three" -c "$JAR2" >/dev/null
assert_status "revocation: second cookie works" "200" "$(devices_with "$JAR2")"
http_post_status "${RPODDER_URL}/api/admin/users/${REVOKE_USER}/deactivate" -u "$AUTH_HEADER" >/dev/null
http_post_status "${RPODDER_URL}/api/admin/users/${REVOKE_USER}/activate" -u "$AUTH_HEADER" >/dev/null
assert_status "revocation: cookie dead after deactivate+activate" "401" "$(devices_with "$JAR2")"

# Cleanup
curl -s -o /dev/null -X DELETE "${RPODDER_URL}/api/admin/users/${REVOKE_USER}" -u "$AUTH_HEADER"
rm -f "$JAR" "$JAR2"
