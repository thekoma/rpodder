# User Management

## Roles

rpodder has two roles:

- **User** — can sync subscriptions, browse podcasts, manage their own devices and settings
- **Admin** — can do everything a user can, plus manage other users, force feed refreshes, and view server stats

### First user

The first user registered (or created via CLI) automatically becomes an admin, regardless of the registration mode. This ensures you can always bootstrap your instance.

### Promoting users

Admins can promote other users to admin via the Admin panel or the API:

```bash
curl -X POST "http://localhost:3005/api/admin/users/someuser/role" \
  -b "sessionid=your-session-cookie" \
  -H "Content-Type: application/json" \
  -d '{"is_admin": true}'
```

## Creating users

### Via CLI

```bash
rpodder user create username password --admin          # admin user
rpodder user create username password                  # regular user
rpodder user create username password --email user@ex  # with email
```

### Via the web UI

Admin panel → **+ Create User** → fill in username, password, and optional email.

### Via public registration

If `RPODDER_REGISTRATION=open`, anyone can register at `/register`. If `RPODDER_REGISTRATION=invite`, users must provide an email and click an activation link.

### Via SSO

Users are auto-created on first SSO login. See [SSO guide](sso.md).

## Admin panel

The admin panel (`/admin`) shows:

### Server stats

Five cards showing total counts: Users, Devices, Subscriptions, Podcasts, Episode Actions.

### User list

Each user shows:

- Username, email, status (active/inactive), role (admin badge)
- Device count, subscription count
- Action buttons (for other users, not yourself):
    - **Make admin / Remove admin** — toggle admin role
    - **Set password** — directly set a new password
    - **Reset password** — send a password reset email (requires SMTP + user email)
    - **Deactivate / Activate** — disable/enable login
    - **Delete** — permanently remove the user

### Feed management

- **Force Feed Update** — triggers an immediate refresh of all feeds in the background

## Deactivating vs deleting

- **Deactivate**: the user can't log in, but their data (subscriptions, episode actions, devices) is preserved. Useful for temporary suspension
- **Delete**: the user and all their data is permanently removed. Cannot be undone

## Registration modes

| Mode | Behavior |
|------|----------|
| `open` | Anyone can register via `/register` or the API |
| `closed` | Only admins can create users (via CLI, API, or admin panel) |
| `invite` | Anyone can register, but must provide an email. An activation link is sent via SMTP. Account is inactive until clicked |

!!! note
    The first user can always register, even in `closed` or `invite` mode. This prevents lockout on fresh installations.
