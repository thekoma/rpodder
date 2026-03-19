# Discovery & Search

rpodder helps you find new podcasts through local search, external databases, and trending charts.

## Local search

The local search queries podcasts that are already indexed in your rpodder database — podcasts that you or other users on your instance have subscribed to.

- **PostgreSQL**: uses `tsvector` full-text search
- **SQLite**: uses FTS5 (Full-Text Search) with prefix matching

Search is fuzzy — typing "pil" will find "Pillole di Bit".

## Podcast Index integration

When configured with API keys (see [Configuration](../getting-started/configuration.md#podcast-index)), rpodder also searches [Podcast Index](https://podcastindex.org) — an open, free database of over 4 million podcasts.

Search results are split into two sections:

- **Local results** — podcasts already in your database, with subscriber counts
- **Podcast Index** — external results with a **+ Add** button

When you subscribe to a Podcast Index result, rpodder:

1. Creates a podcast entry in your database
2. Subscribes you to it
3. Fetches the feed in the background (within 30 minutes, or immediately if you visit the podcast detail page)

!!! tip "No API keys needed for basic use"
    Podcast Index integration is optional. Without it, search works with your local database only. The web UI gracefully degrades — the "Podcast Index" section simply doesn't appear.

## Trending

The **Trending** tab shows podcasts that are gaining popularity, powered by Podcast Index. You can filter by language:

- All, English, Italiano, Deutsch, Español, Français

Trending data comes directly from Podcast Index's `/api/1.0/podcasts/trending` endpoint.

## Categories and tags

rpodder extracts categories (tags) from podcast RSS feeds during indexing. The Browse page shows:

- **Popular** — top podcasts by subscriber count
- **Category sections** — podcasts grouped by their most common tags
- **Tag cloud** — clickable category pills with usage counts

Click a tag to see all podcasts in that category.

## HTTP/HTTPS dedup

If the same podcast exists with both `http://` and `https://` URLs, rpodder:

- Shows only the HTTPS version in directory listings
- Sums the subscriber counts from both versions
- Suggests HTTPS upgrades in the Subscriptions page (see [Subscriptions](subscriptions.md))
