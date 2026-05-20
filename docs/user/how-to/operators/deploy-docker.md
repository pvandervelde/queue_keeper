# Deploy with Docker

This guide shows how to run Queue-Keeper using Docker, covering single-container, Docker Compose, and environment variable configuration patterns.

## Prerequisites

- Docker 20.10 or later
- Bot configuration file (`bot-config.yaml`) — see [Add a Bot Subscription](add-bot-subscription.md)
- Service configuration file (`service.yaml`) — see [Configuration reference](../../reference/configuration.md)
- Azure credentials provisioned — see [Configure Azure Services](configure-azure.md)

---

## Single container

The minimal production invocation mounts both configuration files and sets credentials via environment:

```bash
docker run -d \
  --name queue-keeper \
  --restart unless-stopped \
  -p 8080:8080 \
  -v /etc/queue-keeper/service.yaml:/config/service.yaml:ro \
  -v /etc/queue-keeper/bot-config.yaml:/config/bot-config.yaml:ro \
  -e QUEUE_KEEPER_CONFIG=/config/service.yaml \
  ghcr.io/pvandervelde/queue-keeper:latest \
  start --foreground
```

Verify it is running:

```bash
curl -sf http://localhost:8080/health && echo "OK"
```

---

## Docker Compose

For local development or small-scale deployments, Docker Compose is more convenient:

```yaml
# docker-compose.yml
services:
  queue-keeper:
    image: ghcr.io/pvandervelde/queue-keeper:latest
    command: start --foreground
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      - ./config/service.yaml:/config/service.yaml:ro
      - ./config/bot-config.yaml:/config/bot-config.yaml:ro
    environment:
      QUEUE_KEEPER_CONFIG: /config/service.yaml
      QK__LOGGING__LEVEL: info
    healthcheck:
      test: ["CMD", "curl", "-sf", "http://localhost:8080/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
```

Run:

```bash
docker compose up -d
docker compose logs -f queue-keeper
```

---

## Configuration files

### `service.yaml` — production template

```yaml
server:
  port: 8080
  host: "0.0.0.0"

logging:
  level: "info"
  json_format: true          # Use false in development for human-readable output

providers:
  - id: "github"
    require_signature: true
    secret:
      type: key_vault
      secret_name: "github-webhook-secret"

key_vault:
  vault_url: "https://my-vault.vault.azure.net"

queue:
  provider: azure_service_bus
  namespace: my-namespace.servicebus.windows.net
```

### `bot-config.yaml` — minimal starter

```yaml
bots:
  - name: "my-bot"
    queue: "queue-keeper-my-bot"
    events:
      - "issues.*"
      - "pull_request.*"
    ordered: true
```

---

## Passing secrets safely

Never put connection strings or webhook secrets in `service.yaml` when running in production. Use one of these approaches:

**Azure Key Vault (recommended in Azure)**

Set `key_vault.vault_url` in `service.yaml` and grant the container's managed identity `Key Vault Secrets User` role. Secrets are fetched at startup and refreshed automatically.

**Environment variable override (CI / local dev)**

Webhook secrets are injected via a user-defined environment variable wired into the provider's `secret` config. First, add the `environment_variable` secret source to `service.yaml`:

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: environment_variable
      env_var_name: "GITHUB_WEBHOOK_SECRET"
```

Then pass the secret at runtime:

```bash
docker run ... \
  -e GITHUB_WEBHOOK_SECRET=my-dev-secret \
  ...
```

The env var name (`GITHUB_WEBHOOK_SECRET` above) is chosen by you — it must match `env_var_name` in `service.yaml`.

!!! warning
    Environment variable secrets are a convenience for local development. Do not use them in production — they expose secrets in the process environment and `docker inspect` output.

---

## Updating the image

```bash
docker pull ghcr.io/pvandervelde/queue-keeper:latest
docker compose up -d --no-deps queue-keeper
```

Queue-Keeper performs a graceful shutdown (default 30 s) before the old container exits. Existing requests are drained before the process terminates.

---

## Health and readiness

| Endpoint | Usage |
|---|---|
| `GET /health` | Liveness — basic service is running |
| `GET /ready` | Readiness — external dependencies (queue, key vault) verified |

Use `/health` for Docker's `HEALTHCHECK` and load balancer liveness probes. Use `/ready` to delay traffic until the service has fully initialised.
