# Bot Integration Guide

This guide explains how to build a bot that consumes events from Queue-Keeper's queues.

Queue-Keeper sits between GitHub (or other webhook providers) and your bot. It validates incoming webhooks, normalises them into a consistent format, and delivers them to your queue with guaranteed ordering.

```
GitHub ──webhook──▶ Queue-Keeper ──WrappedEvent──▶ Queue ──▶ Your Bot
              (validate, normalise, route)           (process, act)
```

Queue-Keeper supports multiple queue backends (Azure Service Bus, AWS SQS, and others) through the `queue-runtime` abstraction layer. The message format is the same regardless of backend; the code examples in this guide use **Azure Service Bus** unless otherwise noted. Adapt the SDK calls for your chosen backend.

---

## Prerequisites

- A queue configured for use with Queue-Keeper's `queue-runtime` (e.g. Azure Service Bus namespace, AWS SQS queue)
- Queue-Keeper deployed and configured (see [Configuration Guide](configuration.md))
- Your bot registered in `bot-config.yaml` with a matching queue name

---

## Step 1: Register Your Bot Subscription

Add your bot to `bot-config.yaml` to tell Queue-Keeper which events to route to your queue:

```yaml
bots:
  - name: "my-bot"
    queue: "queue-keeper-my-bot"
    events:
      - "pull_request.opened"
      - "pull_request.closed"
      - "pull_request.synchronize"
    ordered: true   # Receive events for the same PR in order
```

**Key decisions:**

| Field | Guidance |
|---|---|
| `ordered: true` | Required when your bot maintains state per pull request or issue. The queue delivers events for the same entity in arrival order. |
| `ordered: false` | Use for stateless bots (notifications, metrics collectors). Messages arrive without ordering guarantees but with higher throughput. |
| `events` | Narrow this list as much as possible to reduce Queue-Keeper's per-message cost and your bot's processing load. Use wildcards (`"pull_request.*"`) only when you need all sub-actions. |

See [Configuration Guide — Event Pattern Syntax](configuration.md#event-pattern-syntax) for the full pattern reference, including exclusion patterns and wildcard rules.

---

## Step 2: Create the Queue

Create a queue matching your `queue` field value. If you use `ordered: true`, the queue **must** have session-based ordering enabled (the exact setting name depends on your queue backend).

> **Azure Service Bus example**
>
> ```bash
> az servicebus queue create \
>   --resource-group my-rg \
>   --namespace-name my-namespace \
>   --name queue-keeper-my-bot \
>   --requires-session true \
>   --lock-duration PT5M \
>   --default-message-time-to-live P14D
> ```
>
> For unordered bots (`ordered: false`), omit `--requires-session`.

Recommended queue settings (names are illustrative; map to your backend's terminology):

| Concept | Recommended value |
|---|---|
| Message lock / visibility timeout | 5 minutes — extend if your bot needs more processing time |
| Message time-to-live | 14 days |
| Max delivery count | 10 — messages dead-lettered after this many failed deliveries |
| Dead-letter on expiration | Enabled |

---

## Step 3: Receive Messages

> The examples below use **Azure Service Bus**. For other backends, use the equivalent session-aware receiver from your queue provider's SDK.

### Ordered Bot (Session Receiver)

When `ordered: true`, you **must** use a session-aware receiver. A plain receiver cannot read session-locked messages.

**Python (azure-servicebus — Azure Service Bus)**

```python
import json
import logging
from azure.servicebus import ServiceBusClient, NEXT_AVAILABLE_SESSION
from azure.servicebus.exceptions import OperationTimeoutError

logger = logging.getLogger(__name__)

CONN_STR = "Endpoint=sb://..."  # from environment/secret store
QUEUE_NAME = "queue-keeper-my-bot"

def process_event(event: dict) -> None:
    """Your bot logic here."""
    event_type = event["event_type"]
    action = event.get("action")
    correlation_id = event["correlation_id"]
    event_id = event["event_id"]

    logger.info(
        "Processing event",
        extra={"correlation_id": correlation_id, "event_id": event_id,
               "event_type": event_type, "action": action}
    )

    if event_type == "pull_request" and action == "opened":
        pr = event["payload"]["pull_request"]
        repo = event["payload"]["repository"]["full_name"]
        logger.info("PR opened: %s#%s", repo, pr["number"],
                    extra={"correlation_id": correlation_id})

def main() -> None:
    with ServiceBusClient.from_connection_string(CONN_STR) as client:
        while True:
            try:
                # Accept the next available session (blocks until one is ready)
                with client.get_queue_session_receiver(
                    QUEUE_NAME,
                    session_id=NEXT_AVAILABLE_SESSION,
                    max_wait_time=30,
                ) as receiver:
                    for msg in receiver:
                        event_id = msg.application_properties.get(b"qk_event_id", b"").decode()
                        try:
                            event = json.loads(str(msg))
                            process_event(event)
                            receiver.complete_message(msg)
                        except Exception as exc:
                            logger.error("Failed to process event %s: %s", event_id, exc)
                            # abandon so it can be retried or dead-lettered
                            receiver.abandon_message(msg)
            except OperationTimeoutError:
                # No sessions available — loop and wait
                continue

if __name__ == "__main__":
    main()
```

**C# (Azure.Messaging.ServiceBus — Azure Service Bus)**

```csharp
using Azure.Messaging.ServiceBus;
using System.Text.Json;

var client = new ServiceBusClient(connectionString);
var processor = client.CreateSessionProcessor(queueName, new ServiceBusSessionProcessorOptions
{
    MaxConcurrentSessions = 4,         // process up to 4 sessions in parallel
    MaxConcurrentCallsPerSession = 1,  // within a session, process one message at a time
});

processor.ProcessMessageAsync += async args =>
{
    var body = args.Message.Body.ToString();
    var evt = JsonDocument.Parse(body).RootElement;

    var correlationId = evt.GetProperty("correlation_id").GetString();
    var eventType = evt.GetProperty("event_type").GetString();

    Console.WriteLine($"[{correlationId}] Processing {eventType}");

    // ... your bot logic ...

    await args.CompleteMessageAsync(args.Message);
};

processor.ProcessErrorAsync += args =>
{
    Console.Error.WriteLine($"Error: {args.Exception}");
    return Task.CompletedTask;
};

await processor.StartProcessingAsync();
Console.ReadKey();
await processor.StopProcessingAsync();
```

### Unordered Bot (Standard Receiver)

When `ordered: false`, use a standard (non-session) receiver:

**Python (Azure Service Bus)**

```python
with ServiceBusClient.from_connection_string(CONN_STR) as client:
    with client.get_queue_receiver(QUEUE_NAME) as receiver:
        for msg in receiver:
            event = json.loads(str(msg))
            try:
                process_event(event)
                receiver.complete_message(msg)
            except Exception as exc:
                logger.error("Failed to process: %s", exc)
                receiver.abandon_message(msg)
```

---

## Step 4: Parse the Event

All GitHub events (and generic providers in wrap mode) arrive as a JSON-serialized `WrappedEvent`. See [Queue Message Format](queue-message-format.md) for the full field reference.

**Quick field summary:**

```python
event = json.loads(message_body)

event["event_id"]       # "01JQZM7XK4B3VYFNHD0G2T8P1X" — ULID, use for deduplication
event["provider"]       # "github"
event["event_type"]     # "pull_request"
event["action"]         # "opened" (may be None/null)
event["session_id"]     # "myorg/myrepo/pull_request/42" (may be None/null)
event["correlation_id"] # trace ID — include in all your log lines
event["received_at"]    # "2026-04-18T10:00:00.000Z"
event["payload"]        # complete original GitHub webhook body
```

**Accessing GitHub-specific fields:**

```python
payload = event["payload"]

# Common fields available for all events
repo_name = payload["repository"]["full_name"]   # "myorg/myrepo"
sender = payload["sender"]["login"]              # "alice"

# pull_request events
pr = payload["pull_request"]
pr_number = pr["number"]
pr_title = pr["title"]
pr_state = pr["state"]            # "open" or "closed"
is_draft = pr["draft"]            # True/False
head_sha = pr["head"]["sha"]
base_ref = pr["base"]["ref"]      # target branch, e.g. "main"

# issues events
issue = payload["issue"]
issue_number = issue["number"]
issue_title = issue["title"]
labels = [l["name"] for l in issue.get("labels", [])]

# push events
ref = payload["ref"]              # "refs/heads/main"
head_commit = payload["head_commit"]["id"]
```

---

## Step 5: Implement Idempotency

Queue-Keeper guarantees _at-least-once_ delivery. Your bot may receive the same `event_id` more than once during:

- Queue redeliveries (bot crash, lock expiry, message abandon)
- Event replays triggered by an operator

Use `event_id` as an idempotency key:

```python
def is_already_processed(event_id: str) -> bool:
    # Check a database, cache, or blob store
    return db.exists("processed_events", event_id)

def mark_processed(event_id: str) -> None:
    db.insert("processed_events", event_id)

# In your message handler:
if is_already_processed(event["event_id"]):
    logger.info("Skipping duplicate event", extra={"event_id": event["event_id"]})
    receiver.complete_message(msg)
    return

# ... process ...
mark_processed(event["event_id"])
receiver.complete_message(msg)
```

---

## Trace Context and Logging

Every message carries a `correlation_id` that spans the GitHub delivery → Queue-Keeper processing → your bot pipeline. Include it in every log line to enable end-to-end correlation across all three systems.

```python
import structlog

log = structlog.get_logger()

event = json.loads(message_body)
bound_log = log.bind(
    correlation_id=event["correlation_id"],
    event_id=event["event_id"],
    event_type=event["event_type"],
)

bound_log.info("processing_started")
# ... do work ...
bound_log.info("processing_complete", duration_ms=elapsed)
```

To correlate with GitHub's delivery logs, note that Queue-Keeper emits a structured log line pairing the GitHub `X-GitHub-Delivery` ID with the `correlation_id`:

```
INFO delivery_correlated delivery_id=12345678-... correlation_id=00-4bf92f...
```

Search for either the `delivery_id` or `correlation_id` to find related log entries across all systems.

---

## Filtering Events

You can filter events before processing by inspecting `event_type` and `action`:

```python
def should_process(event: dict) -> bool:
    event_type = event["event_type"]
    action = event.get("action")

    if event_type == "pull_request" and action in ("opened", "synchronize", "reopened"):
        return True
    if event_type == "issues" and action in ("opened", "labeled"):
        return True
    return False

# In your message handler:
if not should_process(event):
    # still complete the message — this is expected, not an error
    receiver.complete_message(msg)
    return
```

Prefer narrow `events` patterns in `bot-config.yaml` over filtering in code to reduce unnecessary message delivery.

---

## Error Handling

### Transient Errors

For recoverable errors (network failure, downstream API rate limit), abandon the message to return it to the queue for redelivery:

```python
except TransientError as exc:
    logger.warning("Transient error, will retry: %s", exc,
                   extra={"correlation_id": event["correlation_id"]})
    receiver.abandon_message(msg)
```

The queue provider will redeliver the message after the lock/visibility timeout, up to the configured maximum delivery count. After that, the message is moved to the dead-letter queue.

### Permanent Errors

For unrecoverable errors (bad payload, programming error), dead-letter the message explicitly so it is not retried endlessly:

```python
except PermanentError as exc:
    logger.error("Permanent error, dead-lettering: %s", exc,
                 extra={"correlation_id": event["correlation_id"]})
    receiver.dead_letter_message(
        msg,
        reason="ProcessingFailed",
        error_description=str(exc),
    )
```

### Dead-Letter Queue Monitoring

Monitor the dead-letter queue for your subscription and alert when messages accumulate. Messages in the DLQ can be replayed via the Queue-Keeper admin API after the underlying issue is resolved.

---

## Direct Mode Consumers

If your subscription uses a generic provider configured with `processing_mode: direct`, the message body is the raw webhook payload (not a `WrappedEvent`). Tracking metadata is available as queue message attributes prefixed with `qk_`:

| Attribute key | Description |
|---|---|
| `qk_event_id` | Unique event ULID |
| `qk_provider_id` | Provider that received the webhook (e.g. `"jira"`) |
| `qk_received_at` | ISO 8601 UTC receipt timestamp |
| `qk_content_type` | Content-Type of the original request body |
| `CorrelationId` | Trace correlation ID (same semantics as wrapped mode) |

Parse the body according to the provider's native schema. The example below uses Azure Service Bus:

```python
# Direct mode — body is the raw webhook bytes
payload = json.loads(message_body)  # or xml.parse, etc.
event_id = msg.application_properties.get(b"qk_event_id", b"unknown").decode()
correlation_id = msg.correlation_id or "unknown"

logger.info("Received direct payload", extra={
    "correlation_id": correlation_id,
    "event_id": event_id,
})
```

See [Queue Message Format — Direct Mode](queue-message-format.md#direct-mode-messages) for the full property list.

---

## Testing Your Bot Locally

You can send test messages directly to your queue using your queue provider's SDK or management console (e.g. Azure portal Service Bus Explorer, AWS SQS console). For end-to-end testing, send a webhook to a local Queue-Keeper instance:

```bash
# Simulate a GitHub pull_request event (development — literal secret)
SECRET="dev-secret"
PAYLOAD='{"action":"opened","number":1,"pull_request":{"number":1,"title":"Test PR","state":"open","draft":false,"head":{"sha":"abc123","ref":"feature/test","repo":{"full_name":"myorg/myrepo"}},"base":{"ref":"main","repo":{"full_name":"myorg/myrepo"}}},"repository":{"id":1,"name":"myrepo","full_name":"myorg/myrepo","private":false,"owner":{"login":"myorg","type":"Organization"}},"sender":{"login":"alice","type":"User"}}'
SIG="sha256=$(printf '%s' "$PAYLOAD" | openssl dgst -sha256 -hmac "$SECRET" | awk '{print $2}')"

curl -X POST http://localhost:8080/webhook/github \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Event: pull_request" \
  -H "X-GitHub-Delivery: $(uuidgen)" \
  -H "X-Hub-Signature-256: $SIG" \
  -d "$PAYLOAD"
```

Check Queue-Keeper's structured logs for the `correlation_id` and verify the message appears in your queue.

---

## Further Reading

- [Queue Message Format](queue-message-format.md) — Full schema reference for `WrappedEvent` and direct mode messages
- [Configuration Guide](configuration.md) — Bot subscription configuration including event patterns and repository filters
- [API Reference](api.md) — HTTP API including trace context headers
- [Provider Integration Examples](provider-examples.md) — Configuration examples for GitHub, GitLab, Jira, Slack
