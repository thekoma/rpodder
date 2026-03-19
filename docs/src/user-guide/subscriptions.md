# Subscriptions

## Managing subscriptions

### Via the web UI

- **Subscribe**: search for a podcast in Discover, click it, then click **+ Subscribe**
- **Unsubscribe**: go to Subscriptions, hover over a podcast card, click **Unsubscribe**

### Via a podcast app

Your podcast app syncs subscriptions automatically when you add or remove podcasts. See [Client Setup](clients.md) for configuration.

### Via the API

```bash
# Subscribe to a feed
curl -X POST "http://localhost:3005/api/2/subscriptions/myuser/phone.json" \
  -u myuser:mypass \
  -H "Content-Type: application/json" \
  -d '{"add": ["https://example.com/feed.xml"], "remove": []}'

# List all subscriptions
curl "http://localhost:3005/subscriptions/myuser.json" -u myuser:mypass
```

## HTTPS upgrade suggestions

rpodder monitors your subscriptions for feeds that use `http://` when an `https://` version exists in the database. When upgrades are available, a yellow banner appears at the top of the Subscriptions page:

- **Upgrade individual**: click **Upgrade** next to each subscription
- **Upgrade all**: click **Upgrade all (N)** to switch all at once

The upgrade is an atomic operation — rpodder unsubscribes from the HTTP URL and subscribes to the HTTPS URL in a single API call, so your podcast app picks up the change on next sync.

!!! info "Why this matters"
    HTTP feed URLs transmit your requests (including any authentication tokens for private feeds) in plain text. HTTPS protects your privacy and prevents man-in-the-middle attacks. rpodder never upgrades your subscriptions automatically — it only suggests.

## Device sync

If you have multiple devices in a sync group, subscriptions are automatically propagated across all devices in the group. Subscribe on your phone, and it appears on your laptop too.

See [Devices](../api/devices.md) for more on sync groups.

## Formats

The subscription API supports multiple formats:

| Format | Endpoint suffix | Description |
|--------|----------------|-------------|
| JSON | `.json` | Default, structured data |
| OPML | `.opml` | Standard podcast import/export format |
| TXT | `.txt` | One URL per line |

Example:

```bash
# Export as OPML
curl "http://localhost:3005/subscriptions/myuser/phone.opml" -u myuser:mypass > subs.opml

# Import from OPML (replaces all subscriptions on this device)
curl -X PUT "http://localhost:3005/subscriptions/myuser/phone.opml" \
  -u myuser:mypass \
  -H "Content-Type: text/xml" \
  --data-binary @subs.opml
```
