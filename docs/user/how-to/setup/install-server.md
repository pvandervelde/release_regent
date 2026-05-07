---
title: Deploy the server
description: How to run the rr-server webhook server with Docker, Docker Compose, or Kubernetes
---

# Deploy the server

`rr-server` is the production binary. It runs an HTTP server that receives GitHub webhook events,
validates their signatures, and drives the release workflow.

## Prerequisites

Before deploying the server you need:

- A GitHub App with App ID and private key — see [Set up the GitHub App](github-app-setup.md)
- A webhook secret string that matches what you configured in the GitHub App

## Required environment variables

| Variable | Description |
| :--- | :--- |
| `GITHUB_APP_ID` | Numeric GitHub App ID |
| `GITHUB_PRIVATE_KEY` | PEM-encoded private key (the entire `.pem` file contents) |
| `GITHUB_WEBHOOK_SECRET` | HMAC-SHA256 secret shared with GitHub |

See the full list in the [environment variables reference](../../reference/environment-variables.md).

## Option 1: Docker

### Run with `docker run`

```bash
docker run \
  --name release-regent \
  --restart unless-stopped \
  -p 8080:8080 \
  -e GITHUB_APP_ID=123456 \
  -e GITHUB_PRIVATE_KEY="$(cat /path/to/private-key.pem)" \
  -e GITHUB_WEBHOOK_SECRET=your-webhook-secret \
  ghcr.io/pvandervelde/release_regent:latest
```

### Store credentials in a file

Avoid passing the private key on the command line by using `--env-file`:

```bash
# secrets.env (keep this file out of version control)
GITHUB_APP_ID=123456
GITHUB_PRIVATE_KEY=-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA...
-----END RSA PRIVATE KEY-----
GITHUB_WEBHOOK_SECRET=your-webhook-secret
```

```bash
docker run \
  --name release-regent \
  --restart unless-stopped \
  -p 8080:8080 \
  --env-file secrets.env \
  ghcr.io/pvandervelde/release_regent:latest
```

### Health check

```bash
curl http://localhost:8080/
# {"status":"healthy"}
```

## Option 2: Docker Compose

Create a `docker-compose.yml`:

```yaml
services:
  release-regent:
    image: ghcr.io/pvandervelde/release_regent:latest
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      GITHUB_APP_ID: "${GITHUB_APP_ID}"
      GITHUB_PRIVATE_KEY: "${GITHUB_PRIVATE_KEY}"
      GITHUB_WEBHOOK_SECRET: "${GITHUB_WEBHOOK_SECRET}"
      # Optional: restrict to specific repositories
      ALLOWED_REPOS: "myorg/repo-a,myorg/repo-b"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/"]
      interval: 30s
      timeout: 5s
      retries: 3
```

Store credentials in a `.env` file in the same directory (Docker Compose picks it up
automatically):

```bash
# .env
GITHUB_APP_ID=123456
GITHUB_PRIVATE_KEY=-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----
GITHUB_WEBHOOK_SECRET=your-webhook-secret
```

Start the service:

```bash
docker compose up -d
```

## Option 3: Kubernetes

Create a `Secret` for sensitive values:

```bash
kubectl create secret generic release-regent-secrets \
  --from-literal=GITHUB_APP_ID=123456 \
  --from-file=GITHUB_PRIVATE_KEY=/path/to/private-key.pem \
  --from-literal=GITHUB_WEBHOOK_SECRET=your-webhook-secret
```

Deploy with a `Deployment` and `Service`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: release-regent
  labels:
    app: release-regent
spec:
  replicas: 1
  selector:
    matchLabels:
      app: release-regent
  template:
    metadata:
      labels:
        app: release-regent
    spec:
      containers:
        - name: release-regent
          image: ghcr.io/pvandervelde/release_regent:latest
          ports:
            - containerPort: 8080
          envFrom:
            - secretRef:
                name: release-regent-secrets
          env:
            - name: ALLOWED_REPOS
              value: "myorg/repo-a,myorg/repo-b"
          livenessProbe:
            httpGet:
              path: /
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 30
          readinessProbe:
            httpGet:
              path: /
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          resources:
            requests:
              cpu: 50m
              memory: 64Mi
            limits:
              cpu: 500m
              memory: 256Mi
---
apiVersion: v1
kind: Service
metadata:
  name: release-regent
spec:
  selector:
    app: release-regent
  ports:
    - port: 80
      targetPort: 8080
  type: ClusterIP
```

Expose the service externally (for example via an ingress) so that GitHub can reach
`/webhook`. Update the webhook URL in your GitHub App settings to match.

!!! warning "Run only one replica"
    Release Regent processes webhook events from a single in-memory channel. Running more than
    one replica will cause duplicate release PRs. If you need high availability, use a load
    balancer with sticky sessions or implement an external queue (see
    [architecture](../../explanation/architecture.md)).

## Passing the private key securely

The PEM-encoded private key can span multiple lines. Several standard approaches handle this:

**Docker secret (Swarm)**:

```bash
docker secret create rr_private_key /path/to/private-key.pem
```

**Kubernetes secret from file** (shown above) — the `--from-file` flag preserves line breaks.

**Environment variable with newlines** — embed literal `\n` characters and set
`GITHUB_PRIVATE_KEY_FORMAT=escaped` (supported by the server) so it unescapes them on startup.

!!! danger "Never commit private keys"
    Do not store the `.pem` file in version control. Use a secrets manager (AWS Secrets
    Manager, Azure Key Vault, HashiCorp Vault) in production.

## Updating the webhook URL in GitHub

After deployment, update the GitHub App with the server's public URL:

1. Go to **GitHub → Settings → Developer settings → GitHub Apps → \<your app\>**
2. Under **Webhook URL**, enter `https://your-domain.com/webhook`
3. Click **Save changes**

---

## Next steps

- [Configure multiple repositories](configure-multiple-repos.md)
- [Environment variables reference](../../reference/environment-variables.md) — full list with
  defaults
- [Server API reference](../../reference/api.md)
