# API Overview

rpodder implements the [gpodder.net API](https://gpoddernet.readthedocs.io/) and extends it with additional endpoints for admin, search, and user management.

## Base URL

All endpoints are relative to your rpodder instance URL, e.g. `http://localhost:3005`.

## Authentication

Most endpoints require authentication. rpodder supports:

- **HTTP Basic Auth** — standard for podcast clients
- **Session cookies** — used by the web UI after login

See [Authentication](auth.md) for details.

## Response format

All API responses use JSON. The `.json` suffix in endpoint paths is for gpodder.net compatibility and is required where shown.

## Endpoint groups

| Group | Auth required | Description |
|-------|:------------:|-------------|
| [Authentication](auth.md) | Partial | Login, logout, registration |
| [Subscriptions](subscriptions.md) | Yes | Subscribe, unsubscribe, sync changes |
| [Episodes](episodes.md) | Yes | Upload/download episode actions |
| [Devices](devices.md) | Yes | Device management, sync groups |
| [Directory](directory.md) | No | Search, toplist, tags, trending |
| [Admin](admin.md) | Admin only | User management, stats, feeds |

## gpodder.net compatibility

rpodder aims for full compatibility with the gpodder.net API. Notable compatibility details:

- Episode actions accept both `{"actions":[...]}` and bare `[...]` arrays (Kasts sends the latter)
- Timestamps are accepted as both strings and integers
- The `.json` suffix must be used where the gpodder spec requires it
- Route parameters must be entire URL segments (e.g., `/api/2/devices/{username}/{deviceid}.json`)

## Error responses

Errors return appropriate HTTP status codes:

| Code | Meaning |
|------|---------|
| 400 | Bad request (invalid input) |
| 401 | Not authenticated |
| 403 | Forbidden (wrong user or not admin) |
| 404 | Resource not found |
| 409 | Conflict (e.g., duplicate username) |
| 500 | Internal server error |
