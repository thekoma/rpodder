# External Search Providers

rpodder can integrate with external podcast directories to enhance search and discovery. When configured, external providers power the **Trending** tab and supplement search results alongside the local database.

Without an external provider, search only returns podcasts already indexed locally (i.e., podcasts that at least one user has subscribed to).

## Podcast Index

[Podcast Index](https://podcastindex.org) is a free, open podcast directory. It provides search and trending APIs at no cost.

### Getting API credentials

1. Go to [podcastindex.org](https://podcastindex.org) and create a free account
2. Navigate to **Developer** → **API Keys**
3. Generate a new API key pair (key + secret)

### Configuration

Set the following environment variables:

```bash
RPODDER_PODCASTINDEX_KEY=your-api-key
RPODDER_PODCASTINDEX_SECRET=your-api-secret
```

Or in `rpodder.toml`:

```toml
podcastindex_key = "your-api-key"
podcastindex_secret = "your-api-secret"
```

### What it enables

| Feature | Endpoint | Description |
|---------|----------|-------------|
| **Trending** | `GET /api/2/trending` | Trending podcasts with optional language filter |
| **External search** | `GET /api/2/search/external` | Search Podcast Index directly |
| **Combined search** | `GET /api/2/search/all` | Local + Podcast Index results in parallel, deduplicated |

### Kubernetes / Docker

For Docker Compose, add the variables to `.env.oauth` (gitignored):

```bash
RPODDER_PODCASTINDEX_KEY=your-api-key
RPODDER_PODCASTINDEX_SECRET=your-api-secret
```

For Kubernetes, store them in a Secret:

```bash
kubectl create secret generic rpodder-secrets -n rpodder \
  --from-literal=RPODDER_PODCASTINDEX_KEY=your-api-key \
  --from-literal=RPODDER_PODCASTINDEX_SECRET=your-api-secret
```

Then reference the secret with `envFrom` in your deployment (see [Kubernetes guide](../getting-started/kubernetes.md)).

### Troubleshooting

If trending or search returns empty results, check the logs:

```
# Authentication error (wrong key/secret):
WARN Podcast Index trending returned error status=401 body="..."

# Network/DNS error:
WARN Podcast Index trending failed error=...

# Unexpected response format:
WARN Podcast Index trending response parse failed error=...
```

Common issues:

- **401 Unauthorized**: Double-check your API key and secret. Make sure there are no extra spaces or newlines.
- **DNS resolution failure**: The container must be able to resolve `api.podcastindex.org`. Check your cluster DNS configuration.
- **Empty results with no errors**: The provider is working but returned no results for your query/language filter.
