# Admin API

All admin endpoints require authentication + admin role. Returns `401` if not authenticated, `403` if not admin.

## Server stats

```
GET /api/admin/stats
```

```json
{
  "users": 3,
  "devices": 5,
  "subscriptions": 42,
  "podcasts": 150,
  "episode_actions": 1234
}
```

## List users

```
GET /api/admin/users
```

```json
[
  {
    "username": "admin",
    "email": "admin@example.com",
    "active": true,
    "is_admin": true,
    "devices": 2,
    "subscriptions": 15
  }
]
```

## Create user

```
POST /api/admin/users
```

**Body**: `{"username": "newuser", "password": "pass", "email": "user@example.com"}`

## Delete user

```
DELETE /api/admin/users/{username}
```

Permanently deletes the user and all their data.

## Activate / deactivate

```
POST /api/admin/users/{username}/activate
POST /api/admin/users/{username}/deactivate
```

## Set role

```
POST /api/admin/users/{username}/role
```

**Body**: `{"is_admin": true}`

## Set password

```
POST /api/admin/users/{username}/password
```

**Body**: `{"password": "newpassword"}`

## Send password reset email

```
POST /api/admin/users/{username}/reset-password
```

Sends a password reset email to the user (requires SMTP + user has email).

## Force feed update

```
POST /api/admin/feeds/update
```

Triggers an immediate update of all feeds in the background.

## Force single feed update

```
POST /api/admin/feeds/update/single?url={feed_url}
```

Updates a single feed immediately.
