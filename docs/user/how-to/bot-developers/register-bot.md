# Register a Bot

This guide adds your bot as a subscriber in Queue-Keeper's configuration so it starts receiving matching webhook events. It covers the two required tasks: editing `bot-config.yaml` and creating the corresponding message queue.

For the full configuration schema see [Configuration reference](../../reference/configuration.md). For a step-by-step walkthrough of the complete end-to-end setup see [Build Your First Bot](../../tutorials/first-bot.md).

---

## Step 1: Add the subscription to `bot-config.yaml`

Open `bot-config.yaml` and add an entry to the `bots` list. The minimum required fields are `name`, `queue`, `events`, and `ordered`.

```yaml
bots:
  - name: "my-bot"                          # Unique identifier, 1–50 chars, letters/numbers/hyphens
    queue: "queue-keeper-my-bot"            # Must start with "queue-keeper-"
    events:
      - "pull_request.opened"
      - "pull_request.synchronize"
      - "pull_request.closed"
    ordered: true                           # true = FIFO per PR/issue; false = parallel
```

**Choosing events:**

- Use exact patterns (`issues.opened`) rather than broad wildcards to reduce unnecessary message volume
- Use `pull_request.*` only if you truly need all sub-actions
- Use `*` (all events) only for auditing or metrics bots

**Choosing `ordered`:**

| `ordered: true` | `ordered: false` |
|---|---|
| Bot tracks state per entity (PR, issue) | Bot is stateless |
| Processing order affects correctness | Events are independent |
| Uses FIFO/session queue (Azure Service Bus sessions, AWS SQS FIFO) | Standard queue, higher throughput |

---

## Step 2: Create the queue

The queue must exist before Queue-Keeper will route events to it.

!!! important "GitHub webhook target"
    GitHub webhooks are pointed at **Queue-Keeper**, not at your bot. Configure a single webhook in your GitHub repository or organisation settings with the payload URL `https://your-queue-keeper.example.com/webhook/github`. Queue-Keeper then routes events to each registered bot's queue. Your bot has no public HTTP endpoint for receiving webhooks directly.

For an **ordered** bot, the queue must have FIFO / session support enabled.

=== "Azure Service Bus — ordered bot"

    ```bash
    az servicebus queue create \
      --resource-group queue-keeper-rg \
      --namespace-name my-namespace \
      --name queue-keeper-my-bot \
      --requires-session true \
      --lock-duration PT5M \
      --default-message-time-to-live P14D \
      --max-delivery-count 10 \
      --enable-dead-lettering-on-message-expiration true
    ```

    !!! warning
        If `ordered: true` but the queue was created without `--requires-session true`, Azure Service Bus silently ignores the session ID and delivers messages without ordering. Always create the queue with sessions enabled before registering an ordered bot.

=== "Azure Service Bus — unordered bot"

    ```bash
    az servicebus queue create \
      --resource-group queue-keeper-rg \
      --namespace-name my-namespace \
      --name queue-keeper-my-bot \
      --lock-duration PT5M \
      --default-message-time-to-live P14D \
      --max-delivery-count 10 \
      --enable-dead-lettering-on-message-expiration true
    ```

=== "AWS SQS — ordered bot"

    Ordered delivery on AWS requires a **FIFO queue** (suffix `.fifo`) with content-based deduplication enabled:

    ```bash
    aws sqs create-queue \
      --queue-name queue-keeper-my-bot.fifo \
      --attributes FifoQueue=true,ContentBasedDeduplication=true,\
VisibilityTimeout=300,MessageRetentionPeriod=1209600
    ```

    The IAM role attached to Queue-Keeper's workload must have `sqs:SendMessage` permission on this queue.

=== "AWS SQS — unordered bot"

    ```bash
    aws sqs create-queue \
      --queue-name queue-keeper-my-bot \
      --attributes VisibilityTimeout=300,MessageRetentionPeriod=1209600
    ```

Grant Queue-Keeper's workload identity send access on the queue (Azure: `Azure Service Bus Data Sender` role; AWS: `sqs:SendMessage` IAM permission).

---

## Step 3: Reload Queue-Keeper

Configuration is loaded at startup. Restart the service to apply the new subscription:

=== "Docker"

    ```bash
    docker restart queue-keeper
    ```

=== "Kubernetes"

    ```bash
    kubectl -n automation rollout restart deployment/queue-keeper
    ```

---

## Step 4: Test the subscription

Send a test event and verify the message arrives on your queue:

```bash
# (development service with require_signature: false)
curl -s -X POST http://localhost:8080/webhook/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: pull_request" \
  -H "X-GitHub-Delivery: test-001" \
  -d '{
    "action": "opened",
    "number": 1,
    "pull_request": { "number": 1, "title": "Test" },
    "repository": {
      "full_name": "myorg/myrepo",
      "owner": { "login": "myorg" },
      "name": "myrepo"
    }
  }'

# Check queue depth
az servicebus queue show \
  --resource-group queue-keeper-rg \
  --namespace-name my-namespace \
  --name queue-keeper-my-bot \
  --query "countDetails.activeMessageCount"
```

The count should be 1.
