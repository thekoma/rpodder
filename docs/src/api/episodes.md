# Episodes API

## Upload episode actions

```
POST /api/2/episodes/{username}.json
```

Upload episode actions (play, download, delete, new).

**Body**:

```json
{
  "actions": [
    {
      "podcast": "https://example.com/feed.xml",
      "episode": "https://example.com/episode1.mp3",
      "action": "play",
      "timestamp": "2024-01-15T10:30:00",
      "started": 0,
      "position": 120,
      "total": 3600,
      "device": "phone"
    }
  ]
}
```

!!! note "Kasts compatibility"
    rpodder also accepts a bare array `[...]` without the `{"actions": ...}` wrapper, as Kasts sends actions in this format.

### Action types

| Action | Description |
|--------|-------------|
| `play` | Episode was played. Include `started`, `position`, `total` |
| `download` | Episode was downloaded |
| `delete` | Episode was deleted |
| `new` | Episode was marked as new |

### Fields

| Field | Required | Description |
|-------|:--------:|-------------|
| `podcast` | Yes | Feed URL |
| `episode` | Yes | Episode media URL |
| `action` | Yes | Action type |
| `timestamp` | No | When the action occurred (string or integer epoch) |
| `device` | No | Device ID |
| `started` | No | Play start position (seconds) |
| `position` | No | Current play position (seconds) |
| `total` | No | Total episode duration (seconds) |

**Response**:

```json
{
  "timestamp": 1679000000,
  "update_urls": []
}
```

## Download episode actions

```
GET /api/2/episodes/{username}.json?since={timestamp}
```

Download episode actions since the given timestamp.

**Query parameters**:

| Parameter | Description |
|-----------|-------------|
| `since` | Timestamp (epoch). Only return actions after this time |
| `podcast` | Filter by feed URL |
| `device` | Filter by device ID |
| `aggregated` | If true, only return the latest action per episode |

**Response**:

```json
{
  "actions": [
    {
      "podcast": "https://example.com/feed.xml",
      "episode": "https://example.com/episode1.mp3",
      "action": "play",
      "timestamp": "2024-01-15T10:30:00",
      "position": 120,
      "total": 3600,
      "device": "phone"
    }
  ],
  "timestamp": 1679001000
}
```

## Episode history (enriched)

```
GET /api/2/history/{username}.json?page=0
```

Returns episode actions with enriched podcast and episode metadata (titles instead of just URLs). Used by the web UI's History page.

**Response**:

```json
[
  {
    "podcast_title": "Pillole di Bit",
    "podcast_url": "https://www.pilloledib.it/feed/podcast",
    "episode_title": "Episode 350 - Example",
    "action": "play",
    "timestamp": "2024-01-15T10:30:00",
    "position": 120,
    "total": 3600
  }
]
```

## Deduplication

rpodder deduplicates episode actions. If a client uploads the same action multiple times (same user + episode + action type + timestamp), only one copy is stored.
