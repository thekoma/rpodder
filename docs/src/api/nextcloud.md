# Nextcloud API

rpodder implements the part of Nextcloud that the "Nextcloud gPodder Sync" provider of AntennaPod and Podcini uses. The app signs in with [Login Flow v2](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/LoginFlow/index.html), which gives it an [app password](auth.md#app-passwords), and then syncs through the API of Nextcloud's gPodder Sync app.

## Login Flow v2

### Start

```
POST /index.php/login/v2
```

No authentication, no body. The app's `User-Agent` becomes the name of the app password.

**Response**:

```json
{
  "poll": {
    "token": "a1b2…",
    "endpoint": "https://podcast.example.com/index.php/login/v2/poll"
  },
  "login": "https://podcast.example.com/login/flow#c3d4…"
}
```

The app opens `login` in the browser and polls `endpoint`. The login token travels after `#`, so browsers don't send it to the server and it stays out of request logs. A request expires after 10 minutes.

The addresses come from `RPODDER_BASE_URL`. Without it they are built from the request's `X-Forwarded-Host` or `Host` and `X-Forwarded-Proto` headers, with `http` when the protocol header is missing.

### Grant access

The page at `/login/flow` asks the signed-in user to grant or deny access. It shows the account, the app name as the app reported it, and when the request was made. It uses these routes, which need a session or Basic Auth and refuse app passwords with 403.

```
POST /api/2/me/login-flows/lookup
```

**Body**: `{"token": "<login token>"}`

**Response**: `{"client_name": "AntennaPod/3.4.0", "created_at": "…", "approved": false, "username": "alice"}`. `approved` is true when the caller has granted it. 404 when the request is unknown, expired, or granted by another user.

```
POST /api/2/me/login-flows/answer
```

**Body**: `{"token": "<login token>", "approve": true}`

| Status | Meaning |
|--------|---------|
| 204 | Granted or denied |
| 404 | Unknown or expired, or denying a request someone else granted |
| 409 | Another user granted it first |

Granting again as the same user returns 204.

### Poll

```
POST /index.php/login/v2/poll
Content-Type: application/x-www-form-urlencoded

token=<poll token>
```

After the grant the first poll returns the credentials, once:

```json
{
  "server": "https://podcast.example.com",
  "loginName": "alice",
  "appPassword": "e5f6…"
}
```

Every other poll returns 404 with an empty body, while the request is pending, after a deny, after the credentials were collected, after expiry, and when the account was deactivated.

## gPodder Sync API

All routes need HTTP Basic Auth with the username and an app password (or the account password) or a session. gPodder Sync has no devices, so all these apps share the rpodder device `gpoddersync` (caption "Nextcloud sync"). It is created on first use and can join a [sync group](../user-guide/sync-groups.md) like any device.

`since` is lowered by 5 minutes. AntennaPod uses the phone's clock for its next `since`, and a phone running fast would otherwise miss changes. Re-sent changes are harmless.

| Route | Same as |
|-------|---------|
| `GET /index.php/apps/gpoddersync/subscriptions?since=N` | `GET /api/2/subscriptions/{user}/gpoddersync.json?since=N` |
| `POST /index.php/apps/gpoddersync/subscription_change/create` | `POST /api/2/subscriptions/{user}/gpoddersync.json` |
| `GET /index.php/apps/gpoddersync/episode_action?since=N` | `GET /api/2/episodes/{user}.json?since=N` |
| `POST /index.php/apps/gpoddersync/episode_action/create` | `POST /api/2/episodes/{user}.json` |

Request and response bodies are those of the gpodder.net routes. Uploaded episode actions without a `device` are recorded for `gpoddersync`, and action names are accepted in any case.
