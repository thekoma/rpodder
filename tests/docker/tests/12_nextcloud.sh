# Nextcloud Login Flow v2, app passwords and the gpoddersync API, as
# AntennaPod's and Podcini's "Nextcloud gPodder Sync" provider uses them.

NC_USER="e2e-nextcloud"
NC_PASS="nc-pass-one"
OTHER_USER="e2e-nextcloud-other"
FEED="https://feeds.example.com/nextcloud-e2e.xml"
JAR=$(mktemp)

ensure_user() { # name password
  local status
  status=$(http_post_status "${RPODDER_URL}/api/admin/users" -u "$AUTH_HEADER" \
    -d "{\"username\":\"$1\",\"password\":\"$2\"}")
  if [ "$status" = "409" ]; then
    http_post_status "${RPODDER_URL}/api/admin/users/$1/password" -u "$AUTH_HEADER" \
      -d "{\"password\":\"$2\"}" >/dev/null
    http_post_status "${RPODDER_URL}/api/admin/users/$1/activate" -u "$AUTH_HEADER" >/dev/null
  fi
}
ensure_user "$NC_USER" "$NC_PASS"
ensure_user "$OTHER_USER" "other-pass"

start_flow() {
  curl -s -X POST -A "AntennaPod/3.4.0 e2e" "${RPODDER_URL}/index.php/login/v2"
}
poll_status() { # poll-token
  curl -s -o /dev/null -w '%{http_code}' -X POST \
    -H 'Content-Type: application/x-www-form-urlencoded' \
    --data "token=$1" "${RPODDER_URL}/index.php/login/v2/poll"
}
poll_body() { # poll-token
  curl -s -X POST -H 'Content-Type: application/x-www-form-urlencoded' \
    --data "token=$1" "${RPODDER_URL}/index.php/login/v2/poll"
}
answer() { # credentials login-token approve
  http_post_status "${RPODDER_URL}/api/2/me/login-flows/answer" -u "$1" \
    -d "{\"token\":\"$2\",\"approve\":$3}"
}

# --- Login flow ---
flow=$(start_flow)
poll_token=$(echo "$flow" | jq -r '.poll.token')
login_url=$(echo "$flow" | jq -r '.login')
login_token="${login_url#*#}"
if [ -n "$poll_token" ] && [ "$poll_token" != "null" ] && [ "$login_token" != "$login_url" ]; then
  pass "nextcloud: flow start returns poll token and login link"
else
  fail "nextcloud: flow start returns poll token and login link" "$flow"
fi
assert_status "nextcloud: poll before granting" "404" "$(poll_status "$poll_token")"

info=$(http_post_body "${RPODDER_URL}/api/2/me/login-flows/lookup" -u "${NC_USER}:${NC_PASS}" \
  -d "{\"token\":\"${login_token}\"}")
assert_json "nextcloud: lookup shows the client" "$info" '.client_name' "AntennaPod/3.4.0 e2e"
assert_json "nextcloud: lookup shows the account" "$info" '.username' "$NC_USER"

assert_status "nextcloud: grant" "204" "$(answer "${NC_USER}:${NC_PASS}" "$login_token" true)"
assert_status "nextcloud: grant again" "204" "$(answer "${NC_USER}:${NC_PASS}" "$login_token" true)"
assert_status "nextcloud: another user can't take it" "409" \
  "$(answer "${OTHER_USER}:other-pass" "$login_token" true)"

creds=$(poll_body "$poll_token")
assert_json "nextcloud: poll returns the login name" "$creds" '.loginName' "$NC_USER"
APP_PASS=$(echo "$creds" | jq -r '.appPassword')
if [ ${#APP_PASS} -eq 64 ]; then pass "nextcloud: poll returns an app password"
else fail "nextcloud: poll returns an app password" "$creds"; fi
assert_status "nextcloud: a poll token is redeemed once" "404" "$(poll_status "$poll_token")"

# --- gpoddersync with the app password ---
AP="${NC_USER}:${APP_PASS}"
status=$(http_post_status "${RPODDER_URL}/index.php/apps/gpoddersync/subscription_change/create" -u "$AP" \
  -d "{\"add\":[\"${FEED}\"],\"remove\":[]}")
assert_status "nextcloud: upload subscription changes" "200" "$status"
subs=$(http_body "${RPODDER_URL}/index.php/apps/gpoddersync/subscriptions?since=0" -u "$AP")
assert_json "nextcloud: download subscriptions" "$subs" '.add[0]' "$FEED"

status=$(http_post_status "${RPODDER_URL}/index.php/apps/gpoddersync/episode_action/create" -u "$AP" \
  -d "[{\"podcast\":\"${FEED}\",\"episode\":\"https://feeds.example.com/nc-ep1.mp3\",\"action\":\"play\",\"timestamp\":\"2026-11-23T10:00:00\",\"started\":0,\"position\":42,\"total\":600}]")
assert_status "nextcloud: upload episode actions" "200" "$status"
actions=$(http_body "${RPODDER_URL}/index.php/apps/gpoddersync/episode_action?since=0" -u "$AP")
assert_json "nextcloud: download episode actions" "$actions" '.actions[0].position' "42"
gpodder=$(http_body "${RPODDER_URL}/api/2/episodes/${NC_USER}.json?since=0" -u "$AP")
assert_json "nextcloud: actions land on the gpoddersync device" "$gpodder" '.actions[0].device' "gpoddersync"

assert_status "nextcloud: app password on the gpodder API" "200" \
  "$(http_status "${RPODDER_URL}/api/2/devices/${NC_USER}.json" -u "$AP")"
assert_status "nextcloud: app password can't list app passwords" "403" \
  "$(http_status "${RPODDER_URL}/api/2/me/app-passwords" -u "$AP")"

# --- Sessions from an app password end with it ---
status=$(http_post_status "${RPODDER_URL}/api/2/auth/${NC_USER}/login.json" -u "$AP" -c "$JAR")
assert_status "nextcloud: login.json with the app password" "200" "$status"
assert_status "nextcloud: its cookie works" "200" \
  "$(http_status "${RPODDER_URL}/api/2/devices/${NC_USER}.json" -b "$JAR")"

list=$(http_body "${RPODDER_URL}/api/2/me/app-passwords" -u "${NC_USER}:${NC_PASS}")
flow_id=$(echo "$list" | jq -r '.[0].id')
status=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE \
  "${RPODDER_URL}/api/2/me/app-passwords/${flow_id}" -u "${NC_USER}:${NC_PASS}")
assert_status "nextcloud: revoke" "204" "$status"
assert_status "nextcloud: revoked app password refused" "401" \
  "$(http_status "${RPODDER_URL}/api/2/devices/${NC_USER}.json" -u "$AP")"
assert_status "nextcloud: its session is gone too" "401" \
  "$(http_status "${RPODDER_URL}/api/2/devices/${NC_USER}.json" -b "$JAR")"

# --- Created by hand ---
status=$(http_post_status "${RPODDER_URL}/api/2/me/app-passwords" -u "${NC_USER}:${NC_PASS}" \
  -d '{"name":"Kasts"}')
assert_status "nextcloud: manual create needs the current password" "403" "$status"
created=$(http_post_body "${RPODDER_URL}/api/2/me/app-passwords" -u "${NC_USER}:${NC_PASS}" \
  -d "{\"name\":\"Kasts\",\"current_password\":\"${NC_PASS}\"}")
assert_json "nextcloud: manual create" "$created" '.name' "Kasts"
MANUAL=$(echo "$created" | jq -r '.password')
assert_status "nextcloud: manual app password works" "200" \
  "$(http_status "${RPODDER_URL}/api/2/devices/${NC_USER}.json" -u "${NC_USER}:${MANUAL}")"

# --- Deny ---
flow=$(start_flow)
poll_token=$(echo "$flow" | jq -r '.poll.token')
login_url=$(echo "$flow" | jq -r '.login')
assert_status "nextcloud: deny" "204" "$(answer "${NC_USER}:${NC_PASS}" "${login_url#*#}" false)"
assert_status "nextcloud: poll after deny" "404" "$(poll_status "$poll_token")"

# --- Recovery by an admin revokes app passwords ---
http_post_status "${RPODDER_URL}/api/admin/users/${NC_USER}/password" -u "$AUTH_HEADER" \
  -d '{"password":"nc-pass-two"}' >/dev/null
assert_status "nextcloud: admin-set password revokes app passwords" "401" \
  "$(http_status "${RPODDER_URL}/api/2/devices/${NC_USER}.json" -u "${NC_USER}:${MANUAL}")"

# Cleanup
curl -s -o /dev/null -X DELETE "${RPODDER_URL}/api/admin/users/${NC_USER}" -u "$AUTH_HEADER"
curl -s -o /dev/null -X DELETE "${RPODDER_URL}/api/admin/users/${OTHER_USER}" -u "$AUTH_HEADER"
rm -f "$JAR"
