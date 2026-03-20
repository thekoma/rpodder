# Discovery & Search

rpodder helps you find new podcasts through local search, external databases, and trending charts.

![Browse podcasts](../assets/screenshots/discover-browse.png)

![Trending podcasts](../assets/screenshots/discover-trending.png)

## Local search

The local search queries podcasts that are already indexed in your rpodder database — podcasts that you or other users on your instance have subscribed to.

- **PostgreSQL**: uses `tsvector` full-text search
- **SQLite**: uses FTS5 (Full-Text Search) with prefix matching

Search is fuzzy — typing "pil" will find "Pillole di Bit".

## Podcast Index integration

[Podcast Index](https://podcastindex.org/) is an open, community-driven podcast database created by Adam Curry and Dave Jones as a free, open alternative to Apple Podcasts. It indexes over **4 million podcasts** and provides a completely free API — no rate limits, no paid tiers, no strings attached.

rpodder integrates with Podcast Index to give your users access to the entire podcast ecosystem, not just what's already been subscribed to on your instance.

### Setup

1. Go to [api.podcastindex.org](https://api.podcastindex.org/) and create a free account (30 seconds)
2. You'll receive an **API Key** and **API Secret** by email
3. Add them to your environment:

```bash
RPODDER_PODCASTINDEX_KEY=your-key
RPODDER_PODCASTINDEX_SECRET=your-secret
```

That's it. rpodder will start showing external results immediately.

### Search

When you search from the Browse tab, results are split into two sections:

- **Local results** — podcasts already in your database, with subscriber counts and metadata
- **Podcast Index** — external results from the global database, with a **+ Add** button

Searches run in parallel — local and Podcast Index are queried simultaneously, so there's no added latency. Results that exist in both are deduplicated (the local version takes priority).

### On-demand feed fetch

When you click on an external podcast (from search or trending), rpodder automatically:

1. Creates a podcast entry in the local database
2. Fetches and parses the RSS/Atom feed
3. Indexes all episodes with titles, descriptions, durations, and release dates
4. Displays the full podcast detail page — all without requiring a subscription

This means users can **browse any podcast in the world** before deciding to subscribe.

### Trending

The **Trending** tab shows podcasts that are gaining popularity globally, powered by Podcast Index's trending algorithm. Filter by language with the pills at the top:

- **All** — global trending across all languages
- **English**, **Italiano**, **Deutsch**, **Español**, **Français** — filtered by language

This is a great way to discover popular podcasts in your language that you might not have heard of.

!!! tip "No API keys needed for basic use"
    Podcast Index integration is optional. Without it, search works with your local database only and the Trending tab won't appear. The web UI gracefully degrades — everything else works exactly the same.

!!! info "About Podcast Index"
    [Podcast Index](https://podcastindex.org/) is a 501(c)(3) non-profit that believes podcast data should be open and free. Their API has no rate limits, no paid tiers, and is funded entirely by the community via [Podcasting 2.0 Value4Value](https://podcastindex.org/donate). If you find it useful, consider supporting them.

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
