# Queue Message Format

This document describes the structure of messages that Queue-Keeper places on queues for downstream bot consumption. Queue-Keeper supports multiple queue backends (Azure Service Bus, AWS SQS, and others) through the `queue-runtime` abstraction layer. The message body format is identical across all backends; provider-specific concepts such as session IDs, message attributes, and correlation properties are surfaced according to each backend's conventions.

Queue-Keeper supports two processing modes that produce different message formats:

| Mode | Producers | Message body | Use when |
|---|---|---|---|
| **Wrap** | GitHub provider; generic providers with `processing_mode: wrap` | JSON-serialized `WrappedEvent` | You want provider-agnostic routing and session ordering |
| **Direct** | Generic providers with `processing_mode: direct` | Raw webhook body (bytes) | You need the native payload format with no transformation |

---

## Wrapped Mode Messages

### Message Structure

Wrapped mode messages carry a JSON body plus queue message attributes.

**Body** (JSON): A serialized `WrappedEvent` envelope.

**Queue message attributes** (surfaced according to your backend's conventions):

| Attribute | Value | Notes |
|---|---|---|
| `CorrelationId` | Same as `WrappedEvent.correlation_id` | Used by the queue provider for correlation tracking |
| `SessionId` | Same as `WrappedEvent.session_id` (when ordered) | Present only when `ordered: true` in bot config and the event has a session |
| User attribute `event_type` | Same as `WrappedEvent.event_type` | Available for queue filter rules where supported |
| User attribute `bot_name` | The name of the target bot subscription | Identifies the targeted bot |

### `WrappedEvent` JSON Schema

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "provider": "github",
  "event_type": "pull_request",
  "action": "opened",
  "session_id": "myorg/myrepo/pull_request/42",
  "correlation_id": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
  "received_at": "2026-04-18T10:00:00.000Z",
  "processed_at": "2026-04-18T10:00:00.123Z",
  "payload": { }
}
```

### Field Reference

#### `event_id` (string, required)

Unique identifier for this event. Format: [ULID](https://github.com/ulid/spec) — a 26-character sortable, globally-unique string (e.g. `01JQZM7XK4B3VYFNHD0G2T8P1X`). Lexicographic sort order matches chronological order.

Use this field to deduplicate redeliveries: if your bot receives the same `event_id` twice, it is a replay or redelivery of the same event.

#### `provider` (string, required)

The provider that generated this event. Examples:

| Value | Source |
|---|---|
| `"github"` | GitHub built-in provider |
| `"gitlab"` | Generic provider configured with `provider_id: "gitlab"` |
| `"jira"` | Generic provider configured with `provider_id: "jira"` |

#### `event_type` (string, required)

The webhook event type. For GitHub this matches the `X-GitHub-Event` header value (e.g. `"push"`, `"pull_request"`, `"issues"`, `"workflow_run"`). For generic providers this is extracted using the provider's `event_type_source` configuration.

#### `action` (string or null)

The action within the event type. For GitHub events this matches the `action` field in the payload (e.g. `"opened"`, `"closed"`, `"synchronize"`). `null` when no action concept applies to the event type (e.g. `"push"`).

#### `session_id` (string or null)

The session identifier used for ordered delivery. When non-null, Queue-Keeper sets the session identifier on the outgoing message (the exact attribute name depends on the queue backend — e.g. `SessionId` in Azure Service Bus), causing messages for the same session to be delivered in FIFO order to session-aware receivers.

Format: `{owner}/{repo}/{entity_type}/{entity_id}`

Examples:

| `session_id` | Meaning |
|---|---|
| `"myorg/myrepo/pull_request/42"` | Events for PR #42 in myorg/myrepo |
| `"myorg/myrepo/issue/17"` | Events for issue #17 |
| `"myorg/myrepo/branch/main"` | Push events to the `main` branch |
| `"myorg/myrepo/release/v1.2.0"` | Release events for tag `v1.2.0` |
| `"myorg/myrepo/repository/repository"` | Repository-level events (no specific entity) |
| `null` | Event has no ordering requirement |

`session_id` is `null` for:

- Events where `ordered: false` in the bot subscription
- Events from providers that do not produce sessions (entity type resolves to `Unknown`)

#### `correlation_id` (string, required)

The distributed trace identifier for this event. Used to correlate logs and traces across Queue-Keeper, the queue, and your bot.

The value is preserved from the incoming webhook request when a trace header is present (see [Trace Context](#trace-context)). When no trace header was present, Queue-Keeper generates a UUID v4.

#### `received_at` (string, required)

ISO 8601 UTC timestamp when the webhook HTTP request was first received by Queue-Keeper's HTTP layer. Format: `"2026-04-18T10:00:00.000Z"`.

Use this field to measure end-to-end latency from GitHub's delivery to your bot.

#### `processed_at` (string, required)

ISO 8601 UTC timestamp when Queue-Keeper finished normalising the event. Always greater than or equal to `received_at`.

#### `payload` (object, required)

The original webhook body, parsed and preserved verbatim. All provider-specific fields are available here.

For **GitHub events**, this is the complete GitHub webhook payload as documented in the [GitHub Webhook Events reference](https://docs.github.com/en/webhooks/webhook-events-and-payloads). Common fields:

```json
{
  "payload": {
    "action": "opened",
    "pull_request": {
      "number": 42,
      "title": "Add feature X",
      "state": "open",
      "head": { "sha": "abc123", "ref": "feature/x" },
      "base": { "ref": "main" }
    },
    "repository": {
      "id": 123456,
      "name": "myrepo",
      "full_name": "myorg/myrepo",
      "private": false,
      "owner": { "login": "myorg" }
    },
    "sender": { "login": "alice", "type": "User" }
  }
}
```

For **generic providers** in wrap mode, this is the raw request body as supplied by the provider.

### Complete Wrapped Mode Example

```json
{
  "event_id": "01JQZM7XK4B3VYFNHD0G2T8P1X",
  "provider": "github",
  "event_type": "pull_request",
  "action": "opened",
  "session_id": "myorg/myrepo/pull_request/42",
  "correlation_id": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
  "received_at": "2026-04-18T10:00:00.000Z",
  "processed_at": "2026-04-18T10:00:00.123Z",
  "payload": {
    "action": "opened",
    "number": 42,
    "pull_request": {
      "number": 42,
      "title": "Add new feature",
      "state": "open",
      "draft": false,
      "head": {
        "sha": "abc123def456",
        "ref": "feature/new-feature",
        "repo": { "full_name": "myorg/myrepo" }
      },
      "base": {
        "ref": "main",
        "repo": { "full_name": "myorg/myrepo" }
      }
    },
    "repository": {
      "id": 123456789,
      "name": "myrepo",
      "full_name": "myorg/myrepo",
      "private": false,
      "owner": { "login": "myorg", "type": "Organization" }
    },
    "sender": { "login": "alice", "type": "User" }
  }
}
```

---

## Direct Mode Messages

### Message Structure

In direct mode the raw webhook body is forwarded unmodified as the message body, with tracking metadata encoded as queue message attributes.

**Body**: Raw bytes from the webhook request body (typically JSON or form-encoded, as supplied by the provider).

**Queue message attributes** (surfaced according to your backend's conventions):

| Attribute | Value |
|---|---|
| `CorrelationId` | Propagated trace identifier (see [Trace Context](#trace-context)) |
| `MessageId` | Auto-generated ULID (`event_id`) |
| User attribute `qk_provider_id` | The provider ID (e.g. `"jira"`) |
| User attribute `qk_event_id` | The event's ULID |
| User attribute `qk_received_at` | ISO 8601 UTC timestamp of receipt |
| User attribute `qk_content_type` | The `Content-Type` of the original request |

The raw body contains exactly what the upstream provider sent — no transformation is applied. Your bot is responsible for parsing and validating it according to the provider's schema.

### Direct Mode Example (Jira)

Message body (raw bytes, no transformation):

```json
{
  "webhookEvent": "jira:issue_created",
  "issue": {
    "id": "10001",
    "key": "PROJ-1",
    "fields": {
      "summary": "Example issue",
      "status": { "name": "To Do" }
    }
  },
  "user": { "name": "alice" }
}
```

---

## Session Ordering

### How Sessions Work

When a bot subscription is configured with `ordered: true` and an event carries a non-null `session_id`, Queue-Keeper sets the session identifier on the outgoing message (the exact mechanism depends on the queue backend — e.g. `SessionId` in Azure Service Bus, a message group attribute in other systems).

The queue backend delivers messages within a session in strict FIFO order. Your bot **must** use a session-aware receiver to consume ordered messages:

```
// Use a session-aware receiver, not a plain receiver.
// A plain receiver cannot read session-locked messages.
```

When `ordered: false`, no session is set and messages can be consumed by any concurrent receiver without ordering guarantees.

### Session Concurrency

Different sessions are independent. Multiple pull requests (or issues) can be processed concurrently even when all are in ordered mode — each PR has a different `session_id` and therefore a different session lock. Only events within the same session (same PR/issue) are strictly ordered.

### Session Abandonment and Rebalancing

If your bot crashes mid-session, the session lock expires (lock duration is configurable on the queue) and the session becomes available for another receiver. Implement idempotency in your bot so that replaying an event (same `event_id`) produces the same outcome.

---

## Trace Context

The `correlation_id` field in all messages (and the queue's `CorrelationId` message attribute) carries a distributed trace identifier that allows you to correlate:

- GitHub delivery logs (via `X-GitHub-Delivery` header)
- Queue-Keeper processing logs
- Your bot's processing logs

Queue-Keeper extracts trace headers in the following priority order:

| Priority | Header | Standard |
|---|---|---|
| 1 | `traceparent` | W3C Trace Context — recommended |
| 2 | `X-Correlation-ID` | Queue-Keeper convention |
| 3 | `X-Request-ID` | Common de-facto standard |

When none of these headers are present, Queue-Keeper generates a fresh UUID v4.

### Using the Correlation ID in Your Bot

Include the `correlation_id` in every log line your bot emits for a given event:

```python
import json, logging
from azure.servicebus import ServiceBusClient

with ServiceBusClient.from_connection_string(conn_str) as client:
    with client.get_queue_session_receiver(queue_name, session_id="*") as receiver:
        for msg in receiver:
            event = json.loads(str(msg))
            correlation_id = event["correlation_id"]

            logger.info("Processing event", extra={
                "correlation_id": correlation_id,
                "event_id": event["event_id"],
                "event_type": event["event_type"],
            })

            # ... bot logic ...

            receiver.complete_message(msg)
```

This ensures your logs can be correlated with Queue-Keeper's logs and GitHub's delivery logs using the same identifier.

---

## Delivery Guarantees and Failure Handling

### At-Least-Once Delivery

Queue-Keeper delivers each event at least once. Your bot must be prepared to receive the same `event_id` more than once (e.g. after a Queue-Keeper retry or an event replay). Use `event_id` as an idempotency key.

### Dead-Letter Queue

When Queue-Keeper exhausts all retry attempts for a message, it writes the event to a dead-letter queue (DLQ) in blob storage for investigation and manual replay. Contact your Queue-Keeper operator if you observe delivery gaps.

### Event Replay

Queue-Keeper supports replaying stored events via the admin API. Replayed events carry the same `event_id` as the original (enabling idempotency) but have a new `correlation_id`. Check the queue message attributes for a `qk_is_replay` attribute (value `"true"`) to distinguish replayed events from originals.

---

## GitHub-Specific Notes

### Session ID Mapping

The following table shows how GitHub event types map to `session_id` format:

| GitHub event | `event_type` | Example `session_id` |
|---|---|---|
| `pull_request` | `pull_request` | `myorg/myrepo/pull_request/42` |
| `pull_request_review` | `pull_request_review` | `myorg/myrepo/pull_request/42` |
| `issues` | `issues` | `myorg/myrepo/issue/17` |
| `issue_comment` | `issue_comment` | `myorg/myrepo/issue/17` |
| `push` (branch) | `push` | `myorg/myrepo/branch/main` |
| `release` | `release` | `myorg/myrepo/release/v1.2.0` |
| `workflow_run` | `workflow_run` | `myorg/myrepo/workflow_run/987654321` |
| `repository`, `push` (tag) | varies | `myorg/myrepo/repository/repository` |

### Accessing GitHub Payload Fields

All GitHub-specific data lives inside `payload`. To get the repository or PR details, access them directly:

```python
event = json.loads(message_body)
repo_full_name = event["payload"]["repository"]["full_name"]
pr_number = event["payload"]["pull_request"]["number"]  # for pull_request events
issue_number = event["payload"]["issue"]["number"]       # for issues events
```

See the [GitHub Webhook Events reference](https://docs.github.com/en/webhooks/webhook-events-and-payloads) for the full payload schema for each event type.
