# Feed Management

rpodder automatically fetches and indexes podcast feeds in the background.

## Background updater

The feed updater runs every **30 minutes** as a background task. It:

1. Lists the podcasts whose next check is due
2. Sorts by subscriber count (popular podcasts first)
3. Skips private feeds (see below)
4. Fetches each feed with conditional GET (ETag, If-Modified-Since)
5. Updates podcast metadata (title, description, author, logo)
6. Creates/updates episodes from feed entries
7. Extracts tags from feed categories

### When a feed is checked

Feeds follow their own publishing rhythm. After each read, rpodder looks at the release dates of the newest episodes and schedules the next check at a quarter of the typical gap between them, between 1 and 24 hours:

| Feed | Typical gap | Checked every |
|------|-------------|---------------|
| News show, three editions a day | ~7 hours | 1 hour |
| Daily show | 24 hours | 6 hours |
| Weekly show | 7 days | 24 hours |
| Show that stopped publishing | (time since the last episode) | 24 hours |
| Nobody on this server subscribes | | 7 days |

The gap is never taken shorter than the time since the newest episode, so a show that goes quiet drifts back to daily checks on its own. Popular feeds are checked at least every 6 hours (10+ subscribers) or every hour (100+ subscribers).

Frequent checks are cheap: rpodder sends the feed's ETag and Last-Modified back, and an unchanged feed answers `304 Not Modified` without being downloaded again.

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

This also works from the podcast detail page: **Refresh feed** (administrators only) downloads the feed in full, ignoring ETag and Last-Modified, waits for the update (up to 25 seconds) and tells you how many new episodes it found. The response is `{"status": "updated", "new_episodes": N}`, `"failed"` with an `error`, or `"running"` when the feed takes longer and finishes in the background.

## On-demand fetch

When a user visits a podcast detail page and the podcast has no metadata yet (title equals URL, empty description), rpodder fetches the feed immediately. This provides instant results for newly added podcasts.

Opening the page of a podcast whose feed hasn't been checked for an hour also re-reads it, so new episodes show up without waiting for the next scheduled check. The page waits up to 4 seconds; for a slower feed it shows *Checking the feed for new episodes…* and reloads the list once the update has had time to finish. A podcast is re-read this way at most once every 10 minutes.

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
