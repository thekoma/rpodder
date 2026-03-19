# Subscriptions API

## Simple API

### Get device subscriptions

```
GET /subscriptions/{username}/{deviceid}.{json|opml|txt}
```

Returns the current list of subscriptions for a device.

**JSON response**: array of feed URLs

```json
["https://example.com/feed1.xml", "https://example.com/feed2.xml"]
```

### Replace device subscriptions

```
PUT /subscriptions/{username}/{deviceid}.{json|opml|txt}
```

Replaces all subscriptions for a device.

**JSON body**: array of feed URLs

```json
["https://example.com/feed1.xml", "https://example.com/feed2.xml"]
```

### Get all user subscriptions

```
GET /subscriptions/{username}.{json|opml|txt}
```

Returns all subscriptions across all devices (deduplicated).

## Advanced API (delta sync)

### Upload subscription changes

```
POST /api/2/subscriptions/{username}/{deviceid}.json
```

Upload add/remove changes. This is what podcast clients typically use.

**Body**:

```json
{
  "add": ["https://example.com/new-feed.xml"],
  "remove": ["https://example.com/old-feed.xml"]
}
```

**Response**:

```json
{
  "timestamp": 1679000000,
  "update_urls": []
}
```

The `timestamp` should be stored and sent back as the `since` parameter in subsequent download requests.

### Download subscription changes

```
GET /api/2/subscriptions/{username}/{deviceid}.json?since={timestamp}
```

Download changes since the given timestamp.

**Response**:

```json
{
  "add": ["https://example.com/new-feed.xml"],
  "remove": ["https://example.com/old-feed.xml"],
  "timestamp": 1679001000,
  "update_urls": []
}
```

## Combined updates

```
GET /api/2/updates/{username}/{deviceid}.json?since={timestamp}
```

Returns both subscription changes and new episode actions in a single request. Useful for clients that want to minimize API calls.

**Response**:

```json
{
  "add": ["https://example.com/feed.xml"],
  "remove": [],
  "updates": [],
  "timestamp": 1679001000
}
```

## Device sync propagation

If devices are in a sync group, subscription changes are automatically propagated. Subscribe on device A, and device B will see the new subscription on next sync.
