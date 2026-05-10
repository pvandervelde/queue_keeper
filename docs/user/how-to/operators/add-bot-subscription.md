# Add a Bot Subscription

This guide adds a new downstream bot to Queue-Keeper's routing configuration. After following these steps the bot's queue will start receiving matching events from the next service restart.

## Prerequisites

- The bot's Azure Service Bus queue already exists — see [Configure Azure Services](configure-azure.md)
- You have access to Queue-Keeper's `bot-config.yaml`

---

## Step 1: Determine what events the bot needs

Before editing configuration, identify:

1. **Which event types** the bot handles (e.g. `issues.*`, `pull_request.opened`)
2. **Whether ordering matters** — does the bot maintain per-entity state? If yes, use `ordered: true`
3. **Which repositories** to include, if not all

Refer to [Event Types and Session IDs](../../reference/event-types.md) for the full list of available event patterns.

---

## Step 2: Edit `bot-config.yaml`

Add a new entry to the `bots` list:

=== "Ordered bot (state-tracking)"

    ```yaml
    bots:
      # ... existing entries ...

      - name: "my-new-bot"
        queue: "queue-keeper-my-new-bot"
        events:
          - "pull_request.opened"
          - "pull_request.synchronize"
          - "pull_request.closed"
        ordered: true
    ```

=== "Unordered bot (stateless)"

    ```yaml
    bots:
      # ... existing entries ...

      - name: "my-notifier"
        queue: "queue-keeper-my-notifier"
        events: ["*"]
        ordered: false
    ```

=== "Repository-scoped bot"

    ```yaml
    bots:
      # ... existing entries ...

      - name: "prod-watcher"
        queue: "queue-keeper-prod-watcher"
        events: ["push"]
        ordered: true
        repository_filter:
          !exact
          owner: "myorg"
          name: "production-app"
    ```

---

## Step 3: Validate the configuration

Use the CLI to validate before restarting:

```bash
queue-keeper config --file /path/to/bot-config.yaml --show
```

Fix any errors reported. Common mistakes:

| Error | Cause |
|---|---|
| `Invalid queue name` | Queue name doesn't start with `queue-keeper-` |
| `Duplicate bot name` | Another bot already uses that name |
| `Bot name contains invalid characters` | Use only letters, numbers, and hyphens |

---

## Step 4: Restart Queue-Keeper

Configuration is loaded at startup only — there is no hot-reload.

=== "Docker"

    ```bash
    docker restart queue-keeper
    ```

=== "Docker Compose"

    ```bash
    docker compose restart queue-keeper
    ```

=== "Kubernetes"

    ```bash
    kubectl -n automation rollout restart deployment/queue-keeper
    kubectl -n automation rollout status deployment/queue-keeper
    ```

---

## Step 5: Verify routing

Send a test event and confirm it reaches the bot's queue:

```bash
# Send a test webhook (development service with signature disabled)
curl -s -X POST http://localhost:8080/webhook/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: pull_request" \
  -H "X-GitHub-Delivery: test-delivery-001" \
  -d '{
    "action": "opened",
    "number": 1,
    "pull_request": { "number": 1, "title": "Test PR" },
    "repository": { "full_name": "myorg/myrepo", "owner": { "login": "myorg" }, "name": "myrepo" }
  }'
```

Check the queue depth in Azure:

```bash
az servicebus queue show \
  --resource-group queue-keeper-rg \
  --namespace-name my-namespace \
  --name queue-keeper-my-new-bot \
  --query "countDetails.activeMessageCount"
```

The count should increase by one.

---

## Removing a bot

Remove the entry from `bot-config.yaml` and restart the service. Queue-Keeper will stop routing events to that queue. The queue itself and any unprocessed messages are not affected — manage those through the Azure portal or CLI.
