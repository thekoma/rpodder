# First Steps

After installing rpodder, here's how to get everything set up.

## 1. Create your first user

The first user you create automatically becomes an admin — no matter what registration mode is configured.

=== "Docker (release)"

    ```bash
    docker compose --profile release exec rpodder-release \
      rpodder user create myuser mypassword --admin
    ```

=== "Docker (SQLite)"

    ```bash
    docker compose --profile sqlite exec rpodder-sqlite \
      rpodder user create myuser mypassword --admin
    ```

=== "Binary"

    ```bash
    rpodder user create myuser mypassword --admin
    ```

## 2. Open the web UI

Navigate to your rpodder instance in a browser:

- Docker default: `http://localhost:3006`
- Docker SQLite: `http://localhost:3005`
- Binary: `http://localhost:3005`

Log in with the credentials you just created.

## 3. Discover podcasts

Go to **Discover** in the navigation bar. You'll see three tabs:

- **Browse** — search your local database and Podcast Index
- **Top Podcasts** — ranked by subscriber count on your instance
- **Trending** — trending podcasts from Podcast Index with language filters

Type a podcast name in the search bar. Results from your local database appear first, followed by results from Podcast Index (if configured). Click **+ Add** to subscribe.

## 4. Connect a podcast app

rpodder is compatible with any app that supports the gpodder.net API:

### AntennaPod (Android)

1. Settings → Synchronization → Choose provider → **gpodder.net**
2. Server: `http://your-server:3005`
3. Enter your username and password

### Kasts (KDE / Linux)

1. Settings → Synchronization → **Custom server**
2. Enter: `http://your-server:3005`
3. Enter your username and password

### gPodder (Desktop)

1. Preferences → gpodder.net
2. Server URL: `http://your-server:3005`
3. Enter your username and password

## 5. Set up SSO (optional)

If you use Authentik, Keycloak, or another OIDC provider, rpodder can delegate authentication to it. Users are auto-created on first SSO login.

See the [SSO guide](../admin-guide/sso.md) for setup instructions.

## 6. Explore the admin panel

As an admin, you'll see an **Admin** link in the navigation bar. The admin panel shows:

- **Server stats** — users, devices, subscriptions, podcasts, episode actions
- **User management** — create, activate/deactivate, set admin role, set/reset passwords, delete
- **Feed management** — force-refresh all feeds or a single feed

## What happens next

rpodder automatically fetches and indexes feeds in the background every 30 minutes. Popular podcasts (more subscribers) are checked more frequently (every hour), while inactive ones are checked less often (up to every 7 days).

When your podcast app syncs, rpodder:

1. Records which podcasts you've subscribed to
2. Tracks your episode progress (play position, completed, etc.)
3. Syncs this data across all your devices
