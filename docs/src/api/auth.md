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
