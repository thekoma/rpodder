# gpodder API Reference

Complete list of API endpoints that rpodder must implement for full compatibility with gpodder clients (AntennaPod, gPodder, KDE Kasts, etc.).

API specification: https://gpoddernet.readthedocs.io/en/latest/api/

## Phase 1 — Core Sync (MVP)

### Authentication
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| POST | `/api/2/auth/{username}/login.json` | Login with HTTP Basic Auth, returns session | TODO |
| POST | `/api/2/auth/{username}/logout.json` | Destroy session | TODO |

### Subscriptions — Simple API
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/subscriptions/{username}/{deviceid}.{format}` | Get subscriptions for device (txt/opml/json) | TODO |
| PUT | `/subscriptions/{username}/{deviceid}.{format}` | Upload full subscription list for device | TODO |
| GET | `/subscriptions/{username}.{format}` | Get all subscriptions for user | TODO |

### Subscriptions — Advanced API
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/api/2/subscriptions/{username}/{deviceid}.json?since=T` | Get subscription changes since T | TODO |
| POST | `/api/2/subscriptions/{username}/{deviceid}.json` | Upload subscription changes (add/remove) | TODO |

### Episode Actions
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/api/2/episodes/{username}.json` | Get episode actions (optional: since, podcast, device, aggregated) | TODO |
| POST | `/api/2/episodes/{username}.json` | Upload episode actions | TODO |

### Devices
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| POST | `/api/2/devices/{username}/{deviceid}.json` | Update device data (caption, type) | TODO |
| GET | `/api/2/devices/{username}.json` | Get list of devices | TODO |

## Phase 2 — Directory

### Search & Discovery
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/search.{format}` | Search podcasts | TODO |
| GET | `/toplist/{count}.{format}` | Top podcasts | TODO |
| GET | `/api/2/tags/{count}.json` | Top tags | TODO |
| GET | `/api/2/tag/{tag}/{count}.json` | Podcasts for a tag | TODO |
| GET | `/api/2/data/podcast.json` | Podcast metadata | TODO |
| GET | `/api/2/data/episode.json` | Episode metadata | TODO |
| GET | `/suggestions/{count}.{format}` | Podcast suggestions for user | TODO |

## Phase 3 — Advanced Features

### Device Synchronization
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/api/2/sync-devices/{username}.json` | Get sync status | TODO |
| POST | `/api/2/sync-devices/{username}.json` | Update sync status | TODO |

### Device Updates (combined endpoint)
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/api/2/updates/{username}/{deviceid}.json` | Get combined subscription + episode updates | TODO |

### Settings
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/api/2/settings/{username}/{scope}.json` | Get settings (scope: account/device/podcast/episode) | TODO |
| POST | `/api/2/settings/{username}/{scope}.json` | Update settings (set/remove keys) | TODO |

### Podcast Lists
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| POST | `/api/2/lists/{username}/create.{format}` | Create podcast list | TODO |
| GET | `/api/2/lists/{username}.json` | Get all lists for user | TODO |
| GET | `/api/2/lists/{username}/list/{slug}.{format}` | Get podcast list | TODO |
| PUT | `/api/2/lists/{username}/list/{slug}.{format}` | Update podcast list | TODO |
| DELETE | `/api/2/lists/{username}/list/{slug}.{format}` | Delete podcast list | TODO |

### Chapters
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/api/2/chapters/{username}.json` | Get chapters for episode | TODO |
| POST | `/api/2/chapters/{username}.json` | Add/remove chapters | TODO |

### Favorites
| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| GET | `/api/2/favorites/{username}.json` | Get favorite episodes | TODO |

## Response Format Notes

### Subscription changes response
```json
{"add": ["http://feed1.xml"], "remove": ["http://feed2.xml"], "timestamp": 1234567890}
```

### Episode actions response
```json
{
  "actions": [
    {
      "podcast": "http://example.com/feed.rss",
      "episode": "http://example.com/ep1.mp3",
      "guid": "ep1-guid",
      "action": "play",
      "device": "my-phone",
      "timestamp": "2024-01-01T12:00:00",
      "started": 0,
      "position": 120,
      "total": 3600
    }
  ],
  "timestamp": 1234567890
}
```

### Device list response
```json
[{"id": "my-phone", "caption": "My Phone", "type": "mobile", "subscriptions": 42}]
```

### Sync status response
```json
{"synchronized": [["dev1", "dev2"]], "not-synchronized": ["dev3"]}
```

### Settings response
```json
{"setting_key": "value", "another_key": "value"}
```

### Supported formats
- **json** — Primary format, always supported
- **opml** — For subscription import/export
- **txt** — Plain text, one URL per line (subscriptions only)
