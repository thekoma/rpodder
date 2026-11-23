# Client Setup

rpodder implements the [gpodder.net API](https://gpoddernet.readthedocs.io/), so it works with any podcast app that supports gpodder sync. It also implements the API of Nextcloud's gPodder Sync app, which AntennaPod and Podcini offer as "Nextcloud".

## Tested clients

### AntennaPod (Android)

**Status**: Fully tested and working with the gpodder.net provider. The Nextcloud provider is implemented against AntennaPod's client code and covered by tests that replay its requests, but has not been tested on a device yet.

**Nextcloud** signs in through the browser, so it works with SSO and never stores your password in the app.

1. Open AntennaPod → **Settings** → **Synchronization**
2. Tap **Choose provider** → **Nextcloud**
3. Enter your server URL, for example `https://podcast.example.com`
4. The browser opens rpodder. Sign in if asked, check the app name and tap **Grant access**
5. Return to AntennaPod

**gpodder.net** asks for a username and password in the app.

1. Open AntennaPod → **Settings** → **Synchronization**
2. Tap **Choose provider** → **gpodder.net**
3. Enter your server URL: `http://your-server:3005`
4. Enter your rpodder username and your password or an [app password](#app-passwords)
5. Tap **Login**

AntennaPod will sync subscriptions and episode actions automatically.

!!! note "Subscriptions with the Nextcloud provider"
    All apps that sync through the Nextcloud provider share one rpodder device, **Nextcloud sync**. Like any device it starts without subscriptions, and the app uploads its own list on the first sync. To share subscriptions with your other devices, put **Nextcloud sync** in a [sync group](sync-groups.md). Listening positions reach every device either way.

### Podcini (Android)

**Status**: Should work through the Nextcloud provider, the only sync provider Podcini offers. It uses the same protocol as AntennaPod. Not tested yet. Set it up as for AntennaPod above.

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

- **JEMC** — desktop podcast manager

If you test rpodder with another client, please let us know how it goes by [opening an issue](https://github.com/thekoma/rpodder/issues).

## Authentication

rpodder supports these authentication methods.

### HTTP Basic Auth

Used by all podcast clients. The username and password are sent with every request.

```
Authorization: Basic base64(username:password)
```

### Session cookies

Used by the web UI. After logging in, a `sessionid` cookie is set with a 365-day expiration.

### App passwords

An app password is a separate password for one app. It works wherever your password works over HTTP Basic Auth, and you can revoke it without changing your password.

- Apps that sign in through the Nextcloud provider get one automatically.
- For other apps, create one under **Settings** → **App passwords** and enter it instead of your password. With SSO this is the way to sync a gpodder.net app without setting an account password.

App passwords can't manage credentials. They can't create other app passwords, grant access to an app, change your password or use the admin API.

Changing your own password keeps your app passwords. A password reset by email, or an admin setting your password, deletes them, so apps using them have to sign in again.

### SSO / OAuth2

Used by the web UI. See the [SSO guide](../admin-guide/sso.md). Podcast apps sign in through the browser with the Nextcloud provider, or use an [app password](#app-passwords) with the gpodder.net provider.

## HTTPS

We strongly recommend running rpodder behind a reverse proxy (nginx, Caddy, Traefik) with TLS termination. Podcast clients send credentials with every request, so HTTPS is important for security.

Behind a proxy, set `RPODDER_BASE_URL` to the public address, for example `https://podcast.example.com`. The Nextcloud login hands this address to the app. Without it rpodder builds the address from the request's `Host` and `X-Forwarded-Proto` headers and falls back to `http`.

Example with Caddy:

```
podcast.example.com {
    reverse_proxy localhost:3005
}
```
