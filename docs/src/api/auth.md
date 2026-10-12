# Authentication API

## Login

```
POST /api/2/auth/{username}/login.json
```

Authenticates via HTTP Basic Auth and returns a session cookie.

**Headers**: `Authorization: Basic base64(username:password)`

**Response**: `200 OK` with `Set-Cookie: sessionid=...`

```bash
curl -X POST "http://localhost:3005/api/2/auth/myuser/login.json" \
  -H "Authorization: Basic $(echo -n myuser:mypass | base64)" \
  -c cookies.txt
```

## Logout

```
POST /api/2/auth/{username}/logout.json
```

Invalidates the session cookie.

```bash
curl -X POST "http://localhost:3005/api/2/auth/myuser/logout.json" \
  -b cookies.txt
```

## Register

```
POST /api/2/register
```

Create a new account. Behavior depends on the registration mode:

- `open`: always succeeds
- `closed`: returns 403
- `invite`: requires email, sends activation link

**Body**:

```json
{
  "username": "newuser",
  "password": "mypassword",
  "email": "user@example.com"
}
```

**Response**:

```json
{"status": "active", "is_admin": false}
```

Or for invite mode:

```json
{"status": "pending_activation", "message": "Check your email to activate your account"}
```

!!! note
    If no active users exist, registration always succeeds and the first user becomes admin.

## Current user info

```
GET /api/2/me
```

Returns info about the authenticated user.

**Response**:

```json
{
  "username": "myuser",
  "email": "user@example.com",
  "is_admin": true,
  "is_active": true
}
```

## Listening progress (rpodder extension)

Used by the web player. Both endpoints require authentication and only ever return the caller's own data.

```
GET /api/2/me/in-progress?limit=20
```

Episodes you started and have not finished ("continue listening"), newest first. For each episode it takes your latest `play` action from any device and keeps it when the position is past 0, more than 30 seconds remain (using the action's `total`, else the episode duration), and the episode was not marked unplayed (`new`) afterwards. `limit` defaults to 20 and is clamped to 1–50.

```
GET /api/2/me/positions?podcast={feed_url}
```

Your latest `play` position for every episode of one podcast, finished or not. An unknown podcast returns `[]`.

**Response** (both):

```json
[
  {
    "podcast_url": "https://example.com/feed.xml",
    "podcast_title": "Example",
    "podcast_logo_url": "https://example.com/logo.png",
    "episode_url": "https://example.com/ep1.mp3",
    "episode_title": "Episode 1",
    "mimetype": "audio/mpeg",
    "position": 1234,
    "total": 3600,
    "timestamp": "2026-10-12T10:00:00",
    "device": "phone"
  }
]
```

Titles are `null` when rpodder only knows the URL (the episode or podcast was first seen in an action upload).

## Change password

```
POST /api/2/me/password
```

**Body**:

```json
{
  "old_password": "current",
  "new_password": "newpass123"
}
```

SSO users can omit `old_password`.

## Password reset (request)

```
POST /api/2/password-reset
```

**Body**: `{"email": "user@example.com"}`

Always returns 200 (anti-enumeration). Sends email if account exists and SMTP is configured.

## Password reset (confirm)

```
POST /api/2/password-reset/confirm
```

**Body**: `{"token": "uuid-from-email", "new_password": "newpass123"}`

## HTTPS upgrades

```
GET /api/2/me/upgrades
```

Lists subscriptions that have an HTTPS alternative available.

**Response**:

```json
[
  {
    "http_url": "http://example.com/feed.xml",
    "https_url": "https://example.com/feed.xml",
    "title": "My Podcast"
  }
]
```

## SSO

```
GET /auth/sso/login      → redirects to OIDC provider
GET /auth/sso/callback   → handles OAuth callback
GET /auth/sso/info       → returns SSO configuration status
```
