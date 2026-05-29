# rpodder Helm chart

Deploys [rpodder](https://github.com/thekoma/rpodder) on Kubernetes. Built on the
[bjw-s common library](https://bjw-s-labs.github.io/helm-charts/docs/common-library/),
so every [app-template](https://bjw-s-labs.github.io/helm-charts/docs/app-template/)
value is available for customization.

## Install

```bash
helm dependency build deploy/helm/rpodder
helm install rpodder deploy/helm/rpodder \
  --set ingress.main.enabled=true \
  --set ingress.main.hosts[0].host=podcasts.example.com
```

Create the first (admin) user:

```bash
kubectl exec deploy/rpodder -- rpodder user create <name> <password> --admin
```

## Defaults

- SQLite on a 2 Gi `PersistentVolumeClaim` mounted at `/data`.
- Service on port `3005`; ingress disabled by default.
- Liveness/readiness/startup probes against `/health`.

## PostgreSQL

Point the database URL at an external PostgreSQL and drop the PVC:

```yaml
controllers:
  rpodder:
    containers:
      main:
        env:
          RPODDER_DATABASE_URL: postgres://user:pass@my-postgres:5432/rpodder
persistence:
  data:
    enabled: false
```

## SSO / extra config

Any `RPODDER_*` setting works as a container env var — see the
[project README](https://github.com/thekoma/rpodder#configuration).
