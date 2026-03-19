# Client Setup

rpodder implements the [gpodder.net API](https://gpoddernet.readthedocs.io/), so it works with any podcast app that supports gpodder sync.

## Tested clients

### AntennaPod (Android)

**Status**: Fully tested and working.

1. Open AntennaPod → **Settings** → **Synchronization**
2. Tap **Choose provider** → **gpodder.net**
3. Enter your server URL: `http://your-server:3005`
4. Enter your rpodder username and password
5. Tap **Login**

AntennaPod will sync subscriptions and episode actions automatically.

### Kasts (KDE / Linux / Android)

**Status**: Fully tested and working. rpodder was primarily developed and tested with Kasts.

1. Open Kasts → **Settings** → **Synchronization**
2. Select **Custom server**
3. Enter: `http://your-server:3005`
4. Enter your rpodder username and password

!!! note "Kasts compatibility"
    Kasts sends episode actions as a bare JSON array `[...]` instead of `{"actions":[...]}`. rpodder handles both formats automatically.

### gPodder (Desktop — Linux, macOS, Windows)

**Status**: Should work (gpodder API compatible), not extensively tested.

1. Open gPodder → **Preferences** → **gpodder.net**
2. Set **Server URL** to `http://your-server:3005`
3. Enter your username and password

## Other clients

Any client that implements the gpodder.net API should work. This includes:

- **Podcini** (Android) — gpodder-compatible fork of AntennaPod
- **JEMC** — desktop podcast manager

If you test rpodder with another client, please let us know how it goes by [opening an issue](https://github.com/thekoma/rpodder/issues).

## Authentication

rpodder supports two authentication methods:

### HTTP Basic Auth

Used by all podcast clients. The username and password are sent with every request.

```
Authorization: Basic base64(username:password)
```

### Session cookies

Used by the web UI. After logging in, a `sessionid` cookie is set with a 365-day expiration.

### SSO / OAuth2

Used by the web UI only. See the [SSO guide](../admin-guide/sso.md). After SSO login, a session cookie is set — podcast clients still use HTTP Basic Auth with a local password (which can be set from the Settings page).

## HTTPS

We strongly recommend running rpodder behind a reverse proxy (nginx, Caddy, Traefik) with TLS termination. Podcast clients send credentials with every request, so HTTPS is important for security.

Example with Caddy:

```
podcast.example.com {
    reverse_proxy localhost:3005
}
```
