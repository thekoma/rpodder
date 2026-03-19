# Feed Management

rpodder automatically fetches and indexes podcast feeds in the background.

## Background updater

The feed updater runs every **30 minutes** as a background task. It:

1. Lists all podcasts with active subscribers
2. Sorts by subscriber count (popular podcasts first)
3. Skips podcasts that were recently updated (adaptive interval)
4. Fetches each feed with conditional GET (ETag, If-Modified-Since)
5. Updates podcast metadata (title, description, author, logo)
6. Creates/updates episodes from feed entries
7. Extracts tags from feed categories

### Adaptive intervals

| Subscribers | Update interval |
|-------------|----------------|
| 5+ | Every 1 hour |
| 2-4 | Every 4 hours |
| 1 | Every 12 hours |
| 0 | Every 7 days |

### Retry with backoff

Failed fetches are retried 3 times with exponential backoff (500ms, 1s, 2s).

### Private feeds

Feeds with access tokens in the URL (detected by heuristics like `?token=`, `?key=`, etc.) are **skipped** by the background updater to prevent token leakage in logs and HTTP requests. These feeds are only fetched on-demand when a subscribed user visits the podcast detail page.

## Force update

### All feeds

From the admin panel, click **Force Feed Update**. This spawns a background task that immediately processes all feeds.

API:

```bash
curl -X POST "http://localhost:3005/api/admin/feeds/update" \
  -b "sessionid=admin-session"
```

### Single feed

API:

```bash
curl -X POST "http://localhost:3005/api/admin/feeds/update/single?url=https://example.com/feed.xml" \
  -b "sessionid=admin-session"
```

This also works from the podcast detail page — click **Refresh Feed** when viewing a podcast.

## On-demand fetch

When a user visits a podcast detail page and the podcast has no metadata yet (title equals URL, empty description), rpodder fetches the feed immediately. This provides instant results for newly added podcasts.

## Feed URL handling

### Normalization

Feed URLs are normalized before storage:

- Scheme and host are lowercased
- Trailing slashes are removed
- Empty fragments are stripped
- Query strings are preserved (some feeds use query params)

### Redirects

If a feed URL redirects (301/302), rpodder follows the redirect and updates the canonical URL.

### Multiple URLs

Each podcast can have multiple URLs (e.g., after a redirect). The `podcast_urls` table maps URLs to podcasts, so subscribing with either URL resolves to the same podcast.

## FTS5 repair (SQLite)

If the full-text search index becomes corrupted (rare, usually after a crash):

```bash
rpodder repair
```

This drops and rebuilds the FTS5 index and recalculates subscriber counts.
