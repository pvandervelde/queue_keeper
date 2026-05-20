# Get Started

This tutorial gets Queue-Keeper running on your local machine and shows you a real GitHub webhook being received and processed. By the end you will have confirmed that the service starts, accepts webhooks, and routes events correctly.

**Time:** ~15 minutes
**Prerequisites:** Docker, a GitHub repository where you can configure webhooks

---

## Step 1: Create a minimal configuration

Create a working directory and a bot configuration file:

```bash
mkdir queue-keeper-demo && cd queue-keeper-demo
```

Create `bot-config.yaml`:

```yaml
bots:
  - name: "demo-bot"
    queue: "queue-keeper-demo-bot"
    events: ["*"]
    ordered: false
```

This registers one bot named `demo-bot` that subscribes to every event. The `ordered: false` setting means events are processed in parallel without FIFO guarantees, which is fine for this demo.

Create `service.yaml`:

```yaml
server:
  port: 8080
  host: "0.0.0.0"

logging:
  level: "debug"
  json_format: false

providers:
  - id: "github"
    require_signature: false

queue:
  provider: in_memory
```

!!! warning
    `require_signature: false` and the `in_memory` queue are **development-only** settings. For a real deployment you must validate signatures and use a real queue backend. See [Deploy with Docker](../how-to/operators/deploy-docker.md) and [Configure Cloud Services](../how-to/operators/configure-azure.md).

!!! info "GitHub webhook URL points to Queue-Keeper"
    In production, GitHub sends webhooks to `https://your-queue-keeper.example.com/webhook/github`. Individual bots are not registered in GitHub — they only subscribe to the queue. See [Configure Webhook Providers](../how-to/operators/configure-providers.md) for the full setup.

---

## Step 2: Start the service

```bash
docker run --rm -p 8080:8080 \
  -v "$(pwd)/bot-config.yaml:/config/bot-config.yaml:ro" \
  -v "$(pwd)/service.yaml:/config/service.yaml:ro" \
  -e QUEUE_KEEPER_CONFIG=/config/service.yaml \
  ghcr.io/pvandervelde/queue-keeper:latest \
  start --foreground
```

Wait until you see a log line similar to:

```
INFO queue_keeper_service: Listening on 0.0.0.0:8080
```

---

## Step 3: Verify the service is healthy

Open a second terminal and run:

```bash
curl -s http://localhost:8080/health | python3 -m json.tool
```

You should see:

```json
{
  "status": "healthy",
  "version": "...",
  "timestamp": "...",
  "checks": {
    "service": { "healthy": true, "message": "Service is running", "duration_ms": 0 },
    "providers": { "healthy": true, "message": "1 webhook provider(s) registered", "duration_ms": 0 }
  }
}
```

---

## Step 4: Send a test webhook

Send a simulated GitHub `push` event. Because we disabled signature validation for this demo, no HMAC header is required:

```bash
curl -s -X POST http://localhost:8080/webhook/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: push" \
  -H "X-GitHub-Delivery: $(uuidgen || echo 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')" \
  -d '{
    "ref": "refs/heads/main",
    "repository": {
      "id": 1296269,
      "full_name": "myorg/myrepo",
      "owner": { "login": "myorg" },
      "name": "myrepo"
    }
  }' | python3 -m json.tool
```

The response should confirm processing:

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "session_id": "myorg/myrepo/branch/main",
  "status": "processed",
  "message": "Webhook processed successfully"
}
```

In the service's log output you will see lines showing signature skipped (development mode), event normalization, and routing to the `queue-keeper-demo-bot` queue.

---

## Step 5: Check event details with the CLI

The CLI can query the service:

```bash
docker run --rm --network host \
  ghcr.io/pvandervelde/queue-keeper:latest \
  events list --format table
```

You should see the `push` event you just sent.

---

## What you learned

- Queue-Keeper starts from a small YAML configuration and a container image
- The `/health` endpoint confirms the service is ready
- Validated webhook payloads are normalised and routed to bot queues
- The CLI can inspect processed events

## Next steps

- [Build Your First Bot](first-bot.md) — write a Python consumer that reads from the bot queue
- [Deploy with Docker](../how-to/operators/deploy-docker.md) — production Docker configuration with real credentials
- [Configure Azure Services](../how-to/operators/configure-azure.md) — provision the Azure infrastructure
