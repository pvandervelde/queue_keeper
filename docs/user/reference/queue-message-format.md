# Queue Message Format

Queue-Keeper places messages on the configured queue backend for downstream bot consumption. Two formats are produced depending on the provider's processing mode.

!!! note "Azure Service Bus and AWS SQS"
    This page uses Azure Service Bus terminology (`CorrelationId`, `SessionId`, user properties). See the [AWS SQS attribute mapping](#aws-sqs-attribute-mapping) section at the bottom for the equivalent SQS `MessageAttributes`.

| Mode | Output | Producers |
|---|---|---|
| **Wrap** | JSON-serialised `WrappedEvent` | GitHub provider; generic providers with `processing_mode: wrap` |
| **Direct** | Raw webhook body bytes | Generic providers with `processing_mode: direct` |

---

## Wrapped Mode

### Message body

A JSON-serialised `WrappedEvent` object.

### Queue message attributes

| Attribute | Value | Notes |
|---|---|---|
| `CorrelationId` | Same as `WrappedEvent.correlation_id` | Used by Service Bus for correlation tracking |
| `SessionId` | Same as `WrappedEvent.session_id` | Set only when `ordered: true` and session is non-null |
| `event_type` (user property) | Same as `WrappedEvent.event_type` | Available for Service Bus filter rules |
| `bot_name` (user property) | Target bot subscription name | Identifies the bot this message is for |

### `WrappedEvent` JSON schema

```json
{
  "event_id":       "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "provider":       "github",
  "event_type":     "pull_request",
  "action":         "opened",
  "session_id":     "myorg/myrepo/pull_request/42",
  "correlation_id": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
  "received_at":    "2026-05-07T10:00:00.000Z",
  "processed_at":   "2026-05-07T10:00:00.123Z",
  "payload":        { }
}
```

### Field reference

#### `event_id` (string, required)

Globally unique event identifier. Format: [ULID](https://github.com/ulid/spec) — 26-character, lexicographically sortable, globally unique. Lexicographic order matches chronological order.

Use `event_id` to deduplicate redeliveries. See [Deduplicate Replayed Events](../how-to/bot-developers/deduplicate-events.md).

#### `provider` (string, required)

The provider that generated this event.

| Value | Source |
|---|---|
| `"github"` | GitHub built-in provider |
| `"gitlab"` | Generic provider with `provider_id: "gitlab"` |
| `"jira"` | Generic provider with `provider_id: "jira"` |

#### `event_type` (string, required)

GitHub event type (matches the `X-GitHub-Event` header value). Examples: `push`, `pull_request`, `issues`, `workflow_run`, `release`.

For generic providers in wrap mode, the value is extracted using `event_type_source` configuration.

#### `action` (string or null)

The action within the event type. Matches the `action` field in the GitHub payload. Null for event types that have no action sub-type (e.g. `push`).

Examples: `opened`, `closed`, `synchronize`, `labeled`.

#### `session_id` (string or null)

Session identifier for ordered delivery. Format: `{owner}/{repo}/{entity_type}/{entity_id}`.

| `session_id` | Meaning |
|---|---|
| `"myorg/myrepo/pull_request/42"` | PR #42 in myorg/myrepo |
| `"myorg/myrepo/issue/17"` | Issue #17 |
| `"myorg/myrepo/branch/main"` | Push events to `main` |
| `"myorg/myrepo/release/v1.2.0"` | Release events for tag `v1.2.0` |
| `"myorg/myrepo/repository/repository"` | Repository-level events |
| `"myorg/myrepo/workflow_run/9999"` | Workflow run events |
| `"myorg/myrepo/discussion/42"` | Discussion thread events |
| `"myorg/myrepo/team/backend"` | Team membership events |
| `"myorg/myrepo/unknown/unknown"` | Unrecognised event type |
| `null` | No ordering requirement (generic providers in wrap mode) |

The `SessionId` queue attribute is set only when the bot subscription has `ordered: true` **and** `session_id` is non-null.

#### `correlation_id` (string, required)

Distributed trace identifier. W3C `traceparent` format when an upstream trace was supplied; UUID v4 otherwise. Use this to correlate Queue-Keeper logs with your bot's logs and spans. See [Correlate Distributed Traces](../how-to/bot-developers/trace-correlation.md).

#### `received_at` (string, required)

RFC 3339 UTC timestamp of when Queue-Keeper received the webhook from the provider.

#### `processed_at` (string, required)

RFC 3339 UTC timestamp of when Queue-Keeper placed the message on the queue.

#### `payload` (object, required)

The original webhook payload as received from the provider. For GitHub events this is the complete GitHub webhook JSON body. The structure varies by event type — refer to the [GitHub Webhook Events documentation](https://docs.github.com/en/webhooks/webhook-events-and-payloads).

---

## Direct Mode

In direct mode, the message body is the raw webhook bytes — no JSON wrapping or field extraction.

### Queue message attributes for direct mode

| Attribute | Value |
|---|---|
| `CorrelationId` | Extracted or generated correlation ID |
| `content_type` (user property) | `application/json` |
| `provider_id` (user property) | The provider's `provider_id` |
| `event_type` (user property) | Extracted event type (if `event_type_source` is configured) |

### Reading direct-mode messages

```python
import json

# The body is the raw webhook bytes
raw_body = str(msg)
payload = json.loads(raw_body)

# Metadata is in message application properties
correlation_id = msg.application_properties.get(b"correlation_id", b"").decode()
event_type = msg.application_properties.get(b"event_type", b"").decode()
```

---

## AWS SQS attribute mapping

When Queue-Keeper is configured with the `aws_sqs` backend, Azure Service Bus message attributes map to SQS `MessageAttributes` as follows:

### Wrapped mode

| Azure Service Bus | AWS SQS `MessageAttributes` key | Type |
|---|---|---|
| `CorrelationId` | `CorrelationId` | `String` |
| `SessionId` | `SessionId` | `String` (omitted when null) |
| `event_type` (user property) | `event_type` | `String` |
| `bot_name` (user property) | `bot_name` | `String` |

**Reading wrapped-mode SQS messages (Python):**

```python
import json
import boto3

sqs = boto3.client("sqs")
response = sqs.receive_message(
    QueueUrl="https://sqs.us-east-1.amazonaws.com/123456789012/queue-keeper-my-bot",
    MessageAttributeNames=["All"],
)

for msg in response.get("Messages", []):
    body = json.loads(msg["Body"])                     # WrappedEvent JSON
    attrs = msg.get("MessageAttributes", {})
    event_type = attrs.get("event_type", {}).get("StringValue", "")
    correlation_id = attrs.get("CorrelationId", {}).get("StringValue", "")
```

### Direct mode

| Azure Service Bus | AWS SQS `MessageAttributes` key | Type |
|---|---|---|
| `CorrelationId` | `CorrelationId` | `String` |
| `content_type` (user property) | `content_type` | `String` |
| `provider_id` (user property) | `provider_id` | `String` |
| `event_type` (user property) | `event_type` | `String` |
