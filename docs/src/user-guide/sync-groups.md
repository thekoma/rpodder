# Sync Groups

Sync groups allow you to keep podcast subscriptions synchronized across multiple devices. When devices are in the same sync group, adding or removing a subscription on one device automatically applies the change to all other devices in the group.

## How it works

By default, each device has its own independent set of subscriptions. This is how the gpodder.net protocol works — each client registers with a unique device ID.

To sync subscriptions across devices, you group them into a **sync group**. When you subscribe to a podcast on any device in the group, rpodder automatically propagates the subscription to all other devices in the same group.

## Managing sync groups (Web UI)

1. Go to the **Devices** page
2. The **Sync Groups** section shows your current groups and unsynced devices
3. Click the **+** button next to an unsynced device
4. Select the device (or existing group) to sync with
5. Click **Sync** to create or extend the group

To remove a device from a group, click the **×** button next to its name in the group.

## Managing sync groups (API)

### Get current sync status

```bash
curl -u user:pass https://rpodder.example.com/api/2/sync-devices/USERNAME.json
```

Response:

```json
{
  "synchronized": [["phone", "laptop"]],
  "not-synchronized": ["tablet"]
}
```

### Create a sync group

```bash
curl -u user:pass -X POST \
  https://rpodder.example.com/api/2/sync-devices/USERNAME.json \
  -H 'Content-Type: application/json' \
  -d '{"synchronize": [["phone", "laptop", "tablet"]]}'
```

You can create multiple groups at once:

```json
{"synchronize": [["phone", "laptop"], ["tablet", "desktop"]]}
```

### Remove a device from its group

```bash
curl -u user:pass -X POST \
  https://rpodder.example.com/api/2/sync-devices/USERNAME.json \
  -H 'Content-Type: application/json' \
  -d '{"synchronize": [], "stop-synchronize": ["tablet"]}'
```

## Important notes

- A sync group requires at least **2 devices**. Attempting to create a group with a single device is a no-op.
- Removing a device from a 2-device group effectively dissolves the group.
- Sync groups propagate **subscription changes** (add/remove). Existing subscriptions are **not** retroactively merged when creating a group.
- Episode actions (play position, etc.) are synced independently via the episode actions API — they are not affected by sync groups.
