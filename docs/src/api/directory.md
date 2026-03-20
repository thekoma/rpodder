# Directory & Search API

These endpoints are public (no authentication required).

## Search (local)

```
GET /search.json?q={query}
```

Searches the local podcast database. Uses PostgreSQL `tsvector` or SQLite FTS5 with fuzzy prefix matching.

**Response**: array of podcast objects

```json
[
  {
    "url": "https://example.com/feed.xml",
    "title": "My Podcast",
    "description": "A great podcast",
    "website": "https://example.com",
    "subscribers": 5,
    "logo_url": "https://example.com/logo.jpg",
    "author": "John Doe",
    "language": "en"
  }
]
```

## Combined search (local + Podcast Index)

```
GET /api/2/search/all?q={query}
```

Searches both the local database and Podcast Index in parallel. Deduplicates results (external podcasts that already exist locally are filtered out).

**Response**:

```json
{
  "local": [
    {"url": "...", "title": "...", "subscribers": 5, ...}
  ],
  "external": [
    {"url": "...", "title": "...", "source": "podcastindex", ...}
  ]
}
```

## External search only

```
GET /api/2/search/external?q={query}
```

Searches [Podcast Index](https://podcastindex.org/) only. Requires `RPODDER_PODCASTINDEX_KEY` and `RPODDER_PODCASTINDEX_SECRET` to be configured. Returns an empty array if not configured.

**Response**: array of external podcast objects

```json
[
  {
    "title": "My Podcast",
    "url": "https://example.com/feed.xml",
    "description": "A great show",
    "author": "Jane Doe",
    "logo_url": "https://example.com/cover.jpg",
    "language": "en",
    "source": "podcastindex"
  }
]
```

## Trending

```
GET /api/2/trending?lang={lang}&max={max}
```

Returns trending podcasts from [Podcast Index](https://podcastindex.org/). Requires Podcast Index API keys to be configured.

| Parameter | Default | Description |
|-----------|---------|-------------|
| `lang` | (all) | Language filter: `en`, `it`, `de`, `es`, `fr`, etc. |
| `max` | `20` | Maximum results (1-50) |

**Response**: array of external podcast objects (same format as external search).

## Toplist

```
GET /toplist/{count}.json
```

Top podcasts by subscriber count on this instance.

## Podcast data

```
GET /api/2/data/podcast.json?url={feed_url}
```

Get metadata for a specific podcast. If the podcast has no metadata yet, rpodder fetches the feed on-demand.

## Episode data

```
GET /api/2/data/episode.json?podcast={feed_url}&url={episode_url}
```

Get metadata for a specific episode.

## Podcast episodes

```
GET /api/2/data/podcast/episodes.json?url={feed_url}&page=0&per_page=30
```

Paginated episode list for a podcast.

## Tags

```
GET /api/2/tags/{count}.json
```

Top tags by usage count.

```
GET /api/2/tag/{tag}/{count}.json
```

Podcasts for a specific tag.

## Suggestions

```
GET /suggestions/{count}.json
```

Personalized podcast suggestions based on the authenticated user's subscriptions. Finds podcasts with similar tags that the user hasn't subscribed to yet. Requires authentication.
