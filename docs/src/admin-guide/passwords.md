# Password Management

rpodder provides multiple ways to manage passwords, designed to work with both local and SSO users.

## Self-service password change

Any authenticated user can change their password from **Settings** (`/settings`):

- **Local users**: provide the current password + new password
- **SSO users**: leave "Current Password" blank, just enter the new password. The active session proves identity

API:

```bash
# With old password
curl -X POST "http://localhost:3005/api/2/me/password" \
  -b "sessionid=your-session" \
  -H "Content-Type: application/json" \
  -d '{"old_password": "current", "new_password": "newpass123"}'

# SSO user (no old password)
curl -X POST "http://localhost:3005/api/2/me/password" \
  -b "sessionid=your-session" \
  -H "Content-Type: application/json" \
  -d '{"new_password": "newpass123"}'
```

## Self-service password reset

Users can reset their password via email from the login page:

1. Click **Forgot password?** on the login page
2. Enter email address
3. Receive an email with a reset link (valid 24 hours)
4. Click the link, enter a new password

!!! note "Anti-enumeration"
    The reset endpoint always returns success, even if no account with that email exists. This prevents attackers from discovering which emails are registered.

Requires SMTP to be configured. See [Configuration](../getting-started/configuration.md#smtp-email).

## Admin password management

Admins have two options in the admin panel:

### Set password directly

Click **Set password** next to a user → enter a new password. Useful when:

- The user doesn't have an email address
- SMTP is not configured
- You need to grant immediate access

API:

```bash
curl -X POST "http://localhost:3005/api/admin/users/someuser/password" \
  -b "sessionid=admin-session" \
  -H "Content-Type: application/json" \
  -d '{"password": "temppass123"}'
```

### Send password reset email

Click **Reset password** next to a user (only shown if the user has an email). An email is sent with a reset link.

API:

```bash
curl -X POST "http://localhost:3005/api/admin/users/someuser/reset-password" \
  -b "sessionid=admin-session"
```

## Password requirements

- Minimum 4 characters
- No complexity requirements (we trust users to make reasonable choices)
- Passwords are hashed with Argon2id before storage

## How tokens work

Password reset tokens are stored in the `sessions` table with a `reset-` prefix and a 24-hour expiry. When a user confirms the reset:

1. The token is looked up in the sessions table
2. If found and not expired, the password is updated
3. The token is deleted (one-time use)

Account activation tokens work the same way, with an `activate-` prefix and 48-hour expiry.
