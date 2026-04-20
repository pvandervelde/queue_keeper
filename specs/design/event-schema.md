# Event Schema Design

## Overview

Queue-Keeper normalizes webhook payloads into a standardized event envelope called `WrappedEvent` that provides consistency for downstream bots while preserving the complete original payload. This design serves both GitHub and generic webhook providers (GitLab, Jira, Slack, etc.).

For the user-facing queue message format specification, see [Queue Message Format](../../docs/queue-message-format.md).

## `WrappedEvent` — Normalized Event Envelope

`WrappedEvent` is the queue message body produced by any provider running in **wrap mode**. It is the sole normalized event type in the system.

### Rust Type Definition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedEvent {
    /// Unique event identifier (ULID — sortable and globally unique).
    pub event_id: EventId,

    /// The provider that generated this event (e.g. `"github"`, `"jira"`).
    pub provider: String,

    /// The event type (e.g. `"push"`, `"pull_request"`, `"issue_updated"`).
    pub event_type: String,

    /// Optional action within the event type (e.g. `"opened"`, `"closed"`).
    pub action: Option<String>,

    /// Session identifier for ordered processing.
    ///
    /// `Some` when the provider or event warrants ordered processing.
    /// For GitHub, encodes the repository and affected entity:
    /// `"{owner}/{repo}/{entity_type}/{entity_id}"`.
    /// `None` for providers or events without an ordering requirement.
    pub session_id: Option<SessionId>,

    /// Correlation identifier for distributed tracing.
    /// Propagated from the incoming `traceparent` / `X-Correlation-ID` /
    /// `X-Request-ID` header, or generated as a UUID v4 when absent.
    pub correlation_id: CorrelationId,

    /// UTC time when the webhook was received by Queue-Keeper's HTTP layer.
    pub received_at: Timestamp,

    /// UTC time when normalization of this event completed.
    pub processed_at: Timestamp,

    /// The original webhook payload, preserved verbatim.
    ///
    /// All provider-specific structured data lives here. Consumers
    /// extract what they need using the fields appropriate for their provider.
    pub payload: serde_json::Value,
}
```

### `DirectQueueMetadata` — Direct Mode Tracking

When a provider operates in **direct mode**, the raw bytes are forwarded unmodified. The following metadata type is attached as Service Bus message properties (not as the message body):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectQueueMetadata {
    /// Unique event identifier for tracking and deduplication.
    event_id: EventId,

    /// Correlation identifier for distributed tracing.
    correlation_id: CorrelationId,

    /// UTC timestamp when the payload was received by Queue-Keeper.
    received_at: Timestamp,

    /// The provider ID that produced this output (e.g. `"jira"`, `"gitlab"`).
    provider_id: String,

    /// The `Content-Type` of the original request body.
    content_type: String,
}
```

## Session ID Strategy

### Session ID Format

Session IDs follow the pattern: `{repo_owner}/{repo_name}/{entity_type}/{entity_id}`

Examples:

- `microsoft/vscode/pull_request/1234`
- `octocat/hello-world/issue/567`
- `github/docs/branch/main`
- `github/docs/release/v1.0.0`

### Session ID Generation Rules (GitHub Provider)

| GitHub event | `entity_type` | `entity_id` source |
|---|---|---|
| `pull_request`, `pull_request_review`, `pull_request_review_comment` | `pull_request` | `payload["pull_request"]["number"]` |
| `issues`, `issue_comment` | `issue` | `payload["issue"]["number"]` |
| `push`, `create`, `delete` (branch ref) | `branch` | `payload["ref"]` stripped of `refs/heads/` |
| `release` | `release` | `payload["release"]["tag_name"]` |
| `discussion`, `discussion_comment` | `discussion` | `payload["discussion"]["number"]` |
| `workflow_run` | `workflow_run` | `payload["workflow_run"]["id"]` |
| Repository-level events | `repository` | `"repository"` |
| Unrecognised events | `unknown` | `"unknown"` |

### Ordering Implications

- Events with the same `session_id` are delivered to Azure Service Bus with the same `SessionId` property, guaranteeing FIFO order within that session
- Events with different `session_id` values can be processed concurrently by separate session receivers
- `session_id` is `None` (null in JSON) when no ordering concept applies, in which case no Service Bus `SessionId` is set

## Event Type Mapping

See the session ID generation rules table above for the full GitHub event → `session_id` mapping. The spec for `EventEntity` extraction logic (which GitHub payload fields are read for each event type) is maintained in `specs/interfaces/webhook-processing.md`.

## Schema Evolution

### Backward Compatibility Rules

1. Never remove fields from `WrappedEvent` without a major version bump
2. New optional fields (`Option<T>`) can be added at any time
3. `action` and `session_id` are already `Option` — bots must handle `null` for both
4. Bots must ignore unknown JSON fields (forward compatibility)

## Example Events

### Pull Request Opened

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
      "head": { "sha": "abc123", "ref": "feature/new-feature" },
      "base": { "ref": "main" }
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

### Push Event

```json
{
  "event_id": "01JQZM8YL5C4WZGOHD1H3U9Q2Y",
  "provider": "github",
  "event_type": "push",
  "action": null,
  "session_id": "myorg/myrepo/branch/main",
  "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
  "received_at": "2026-04-18T10:05:00.000Z",
  "processed_at": "2026-04-18T10:05:00.087Z",
  "payload": {
    "ref": "refs/heads/main",
    "head_commit": { "id": "def456", "message": "Fix bug" },
    "repository": {
      "id": 123456789,
      "name": "myrepo",
      "full_name": "myorg/myrepo",
      "private": false,
      "owner": { "login": "myorg", "type": "Organization" }
    },
    "sender": { "login": "bob", "type": "User" }
  }
}
```

## Service Bus Message Properties

For **wrap mode** messages:

| Property | Value |
|---|---|
| `CorrelationId` | `WrappedEvent.correlation_id` |
| `SessionId` | `WrappedEvent.session_id` (when ordered, may be absent) |
| User property `event_type` | `WrappedEvent.event_type` |
| User property `bot_name` | Target bot name |

For **direct mode** messages the raw body is forwarded with metadata as user properties prefixed `qk_`. See [Queue Message Format — Direct Mode](../../docs/queue-message-format.md#direct-mode-messages).

## Validation Rules

### Event ID

- Must be a valid ULID (26 uppercase alphanumeric characters)
- Globally unique across all events
- Monotonically sortable: later events always sort after earlier ones

### Session ID

- Format: `{owner}/{repo}/{entity_type}/{entity_id}`
- Maximum length: 128 characters
- Characters: ASCII graphic characters excluding whitespace; no leading, trailing, or consecutive slashes

### Repository Validation

- Owner and name must match GitHub naming conventions
- Full name must equal `{owner}/{name}`
- Repository ID must be positive integer

### Payload Validation

- Original payload must be valid JSON
- Must contain minimum required GitHub webhook fields
- Size limit: 1MB per payload (GitHub's limit)
