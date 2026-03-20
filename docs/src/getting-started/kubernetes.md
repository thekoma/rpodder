# Kubernetes (Helm)

This guide shows how to deploy rpodder on Kubernetes using the [bjw-s app-template](https://bjw-s-labs.github.io/helm-charts/docs/app-template/) Helm chart. The example includes PostgreSQL as a sidecar controller — no external database operator required.

## Prerequisites

- Kubernetes cluster (1.27+)
- Helm 3
- An Ingress controller or Gateway API implementation (the example uses [Gateway API](https://gateway-api.sigs.k8s.io/))

## Add the Helm repository

```bash
helm repo add bjw-s https://bjw-s-labs.github.io/helm-charts
helm repo update
```

## Values file

Create a `values.yaml`:

```yaml
controllers:
  # ---------------------------------------------------------------------------
  # rpodder application
  # ---------------------------------------------------------------------------
  main:
    replicas: 1
    strategy: RollingUpdate

    pod:
      securityContext:
        runAsUser: 1000
        runAsGroup: 1000
        fsGroup: 1000
        runAsNonRoot: true

    containers:
      main:
        image:
          repository: ghcr.io/thekoma/rpodder
          tag: latest
          pullPolicy: Always

        env:
          RPODDER_DATABASE_URL: postgres://rpodder:rpodder@rpodder-postgres:5432/rpodder
          RPODDER_BASE_URL: https://rpodder.example.com
          # RPODDER_OAUTH_ISSUER_URL: https://sso.example.com/application/o/rpodder/
          # RPODDER_OAUTH_ADMIN_GROUP: admins
          # RPODDER_OAUTH_PROVIDER_NAME: Authentik

        # Uncomment to load secrets from a Kubernetes Secret
        # envFrom:
        #   - secretRef:
        #       name: rpodder-secrets

        securityContext:
          runAsUser: 1000
          runAsGroup: 1000
          runAsNonRoot: true
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop: ["ALL"]

        resources:
          requests:
            cpu: 100m
            memory: 128Mi
          limits:
            cpu: 500m
            memory: 512Mi

        probes:
          liveness:
            enabled: true
            custom: true
            spec:
              httpGet:
                path: /health
                port: &port 3005
              initialDelaySeconds: 5
              periodSeconds: 30
          readiness:
            enabled: true
            custom: true
            spec:
              httpGet:
                path: /health
                port: *port
              initialDelaySeconds: 5
              periodSeconds: 10

  # ---------------------------------------------------------------------------
  # PostgreSQL
  # ---------------------------------------------------------------------------
  postgres:
    type: statefulset
    replicas: 1
    strategy: RollingUpdate

    statefulset:
      volumeClaimTemplates:
        - name: data
          accessMode: ReadWriteOnce
          size: 5Gi
          globalMounts:
            - path: /var/lib/postgresql/data
              subPath: pgdata

    pod:
      securityContext:
        runAsUser: 999
        runAsGroup: 999
        fsGroup: 999
        runAsNonRoot: true

    containers:
      main:
        image:
          repository: postgres
          tag: "17-alpine"

        env:
          POSTGRES_USER: rpodder
          POSTGRES_PASSWORD: rpodder    # Use a Secret in production!
          POSTGRES_DB: rpodder
          PGDATA: /var/lib/postgresql/data/pgdata

        securityContext:
          runAsUser: 999
          runAsGroup: 999
          runAsNonRoot: true
          allowPrivilegeEscalation: false
          capabilities:
            drop: ["ALL"]

        resources:
          requests:
            cpu: 100m
            memory: 256Mi
          limits:
            cpu: 500m
            memory: 512Mi

        probes:
          liveness:
            enabled: true
            custom: true
            spec:
              exec:
                command: ["pg_isready", "-U", "rpodder"]
              initialDelaySeconds: 10
              periodSeconds: 30
          readiness:
            enabled: true
            custom: true
            spec:
              exec:
                command: ["pg_isready", "-U", "rpodder"]
              initialDelaySeconds: 5
              periodSeconds: 10

# ---------------------------------------------------------------------------
# Services
# ---------------------------------------------------------------------------
service:
  main:
    controller: main
    ports:
      http:
        port: 3005

  postgres:
    controller: postgres
    ports:
      postgres:
        port: 5432

# ---------------------------------------------------------------------------
# Ingress / Gateway API
# ---------------------------------------------------------------------------

# Option A: Gateway API (HTTPRoute)
route:
  main:
    hostnames:
      - rpodder.example.com
    parentRefs:
      - name: my-gateway
        namespace: gateway-system
    rules:
      - backendRefs:
          - kind: Service
            port: 3005
        matches:
          - path:
              type: PathPrefix
              value: /

# Option B: Traditional Ingress (uncomment and remove the route section above)
# ingress:
#   main:
#     className: nginx
#     hosts:
#       - host: rpodder.example.com
#         paths:
#           - path: /
#             pathType: Prefix
#             service:
#               identifier: main
#               port: http
#     tls:
#       - hosts:
#           - rpodder.example.com
#         secretName: rpodder-tls
```

## Install

```bash
helm install rpodder bjw-s/app-template \
  --version 4.6.2 \
  --namespace rpodder --create-namespace \
  -f values.yaml
```

## Create the first user

```bash
kubectl exec -n rpodder deploy/rpodder -- rpodder user create admin yourpassword --admin
```

## Secrets management

For production, avoid putting credentials in `values.yaml`. Create a Secret and reference it with `envFrom`:

```bash
kubectl create secret generic rpodder-secrets -n rpodder \
  --from-literal=RPODDER_OAUTH_CLIENT_ID=your-client-id \
  --from-literal=RPODDER_OAUTH_CLIENT_SECRET=your-client-secret \
  --from-literal=RPODDER_PODCASTINDEX_KEY=your-key \
  --from-literal=RPODDER_PODCASTINDEX_SECRET=your-secret
```

Then uncomment the `envFrom` section in the values file.

!!! tip "Database password"
    The PostgreSQL password in this example is hardcoded for simplicity. In production, use a Secret for `POSTGRES_PASSWORD` and reference the same value in `RPODDER_DATABASE_URL`.

## ArgoCD

To manage this deployment with ArgoCD, create an Application:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: rpodder
  namespace: argocd
spec:
  project: default
  destination:
    namespace: rpodder
    server: https://kubernetes.default.svc
  source:
    chart: app-template
    repoURL: https://bjw-s-labs.github.io/helm-charts
    targetRevision: 4.6.2
    helm:
      valuesObject:
        # ... paste the values from above ...
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
```

## Upgrading

The release workflow publishes new images to `ghcr.io/thekoma/rpodder:latest` on every push to `main`. With `pullPolicy: Always`, a rollout restart picks up the latest image:

```bash
kubectl rollout restart deployment/rpodder -n rpodder
```

Database migrations run automatically on startup.

## SQLite mode

For a simpler setup without PostgreSQL, remove the `postgres` controller and service, and change the database URL:

```yaml
controllers:
  main:
    containers:
      main:
        env:
          RPODDER_DATABASE_URL: sqlite:///data/rpodder.db

persistence:
  data:
    type: persistentVolumeClaim
    accessMode: ReadWriteOnce
    size: 1Gi
    globalMounts:
      - path: /data
```

!!! note "readOnlyRootFilesystem"
    When using SQLite with `readOnlyRootFilesystem: true`, make sure the database path points to the mounted volume, not the read-only root filesystem.
