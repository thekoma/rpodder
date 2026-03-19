# SSO / OAuth2

rpodder supports Single Sign-On via any OpenID Connect (OIDC) provider. Tested with Authentik, but should work with Keycloak, Dex, Google, Azure AD, and others.

## How it works

1. User clicks **Sign in with {provider}** on the login page
2. rpodder redirects to the OIDC provider's authorization endpoint
3. User authenticates with the provider
4. Provider redirects back to rpodder with an authorization code
5. rpodder exchanges the code for tokens, extracts user info
6. rpodder creates a local user (if new) or logs in the existing one
7. A session cookie is set and the user is redirected to the home page

## Configuration

Set these environment variables:

```bash
RPODDER_OAUTH_ISSUER_URL=https://sso.example.com/application/o/rpodder/
RPODDER_OAUTH_CLIENT_ID=your-client-id
RPODDER_OAUTH_CLIENT_SECRET=your-client-secret
RPODDER_OAUTH_PROVIDER_NAME=Authentik
RPODDER_BASE_URL=https://podcast.example.com
```

The `BASE_URL` is critical — it's used to construct the OAuth callback URL: `{BASE_URL}/auth/sso/callback`.

## Provider setup

### Authentik

1. In Authentik, create a new **OAuth2/OpenID Provider**
2. Set the redirect URI to `https://podcast.example.com/auth/sso/callback`
3. Scopes: `openid`, `profile`, `email`, `groups` (if using admin group mapping)
4. Create an Application and link it to the provider
5. Copy the Client ID, Client Secret, and Issuer URL

### Keycloak

1. Create a new Client in your realm
2. Client Protocol: `openid-connect`
3. Access Type: `confidential`
4. Valid Redirect URIs: `https://podcast.example.com/auth/sso/callback`
5. Copy the Client ID and Client Secret from the Credentials tab

### Other providers

Any OIDC-compliant provider should work. rpodder uses the OIDC discovery endpoint (`{issuer}/.well-known/openid-configuration`) to find the authorization, token, and userinfo endpoints automatically.

## Admin group mapping

If your OIDC provider sends group information in the `groups` claim (most do), you can automatically grant admin privileges:

```bash
RPODDER_OAUTH_ADMIN_GROUP=admins
```

When a user logs in via SSO:

- If they belong to the `admins` group → they become admin
- If they don't (or were removed from the group) → admin role is revoked
- This check happens on **every login**, so group changes are reflected immediately

rpodder requests the `groups` scope when this setting is configured.

## SSO users and passwords

SSO users are created with a random password hash (a UUID). They can:

- **Log in via SSO only** — they don't need a local password
- **Set a local password** — from the Settings page (`/settings`), they can set a password without providing the "old password" (since they never had one). This enables them to also log in with username/password or use podcast clients that require HTTP Basic Auth
- **Have an admin set their password** — via the admin panel

!!! tip "Best practice for podcast apps"
    If your users primarily authenticate via SSO but also need to use podcast apps (which require HTTP Basic Auth), have them set a local password from the Settings page after their first SSO login.

## SSO info endpoint

The web UI checks `/auth/sso/info` to determine whether to show the SSO button:

```bash
curl http://localhost:3005/auth/sso/info
```

```json
{
  "enabled": true,
  "provider_name": "Authentik",
  "registration": "open"
}
```
