# Devices API

## List devices

```
GET /api/2/devices/{username}.json
```

**Response**:

```json
[
  {
    "id": "phone",
    "caption": "My Phone",
    "type": "mobile",
    "subscriptions": 12
  },
  {
    "id": "laptop",
    "caption": "Work Laptop",
    "type": "laptop",
    "subscriptions": 8
  }
]
```

## Create / update device

```
POST /api/2/devices/{username}/{deviceid}.json
```

Creates a device if it doesn't exist, or updates its caption and type.

**Body**:

```json
{
  "caption": "My Phone",
  "type": "mobile"
}
```

### Device types

`mobile`, `laptop`, `desktop`, `server`, `other`

## Delete device

```
DELETE /api/2/devices/{username}/{deviceid}.json
```

## Auto-creation

Devices are auto-created when a client syncs subscriptions with a device ID that doesn't exist yet. The device gets a default caption and type of `other`.

## Sync groups

Sync groups allow multiple devices to share the same subscriptions. Subscribe on one device, and the subscription appears on all devices in the group.

### Get sync status

```
GET /api/2/sync-devices/{username}.json
```

**Response**:

```json
{
  "synchronized": [["phone", "tablet"]],
  "not-synchronized": ["laptop"]
}
```

### Set sync groups

```
POST /api/2/sync-devices/{username}.json
```

**Body**:

```json
{
  "synchronize": [["phone", "tablet"]]
}
```

Devices in the same inner array are synced together. Subscriptions are propagated across all devices in a group when changes are uploaded.
