# Event Schema Design

## Overview

Queue-Keeper normalizes GitHub webhook payloads into a standardized event schema that provides consistency for downstream bots while preserving the complete original payload for flexibility.

## Normalized Event Schema

### Core Event Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    /// Unique identifier for this event (ULID format for sortability)
    pub event_id: String,

    /// ISO 8601 timestamp when the event was processed by Queue-Keeper
    pub processed_at: String,

    /// GitHub webhook delivery ID (X-GitHub-Delivery header)
    pub delivery_id: String,

    /// Repository information
    pub repository: RepositoryInfo,

    /// Entity this event relates to (PR, issue, etc.)
    pub entity: EntityInfo,

    /// Session identifier for ordered processing
    pub session_id: String,

    /// GitHub event type and action
    pub event_type: EventType,

    /// Complete original GitHub webhook payload
    pub payload: serde_json::Value,

    /// Event metadata and processing information
    pub metadata: EventMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryInfo {
    /// Repository owner (organization or user)
    pub owner: String,

    /// Repository name
    pub name: String,

    /// Full repository name (owner/name)
    pub full_name: String,

    /// Repository ID (GitHub's internal ID)
    pub id: u64,

    /// Whether the repository is private
    pub private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInfo {
    /// Type of entity this event relates to
    pub entity_type: EntityType,

    /// Entity identifier (PR number, issue number, etc.)
    pub entity_id: String,

    /// Human-readable entity reference (e.g., "PR #123", "Issue #456")
    pub entity_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    PullRequest,
    Issue,
    Repository,      // For push, release events
    Discussion,
    CheckRun,
    CheckSuite,
    CodeScanningAlert,
    DependabotAlert,
    Release,
    Other(String),   // Extensible for future GitHub event types
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventType {
    /// GitHub webhook event type (e.g., "pull_request", "issues")
    pub event: String,

    /// GitHub webhook action (e.g., "opened", "closed", "synchronize")
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Event format version for backward compatibility
    pub schema_version: String,

    /// Which bots this event was routed to
    pub routed_to: Vec<String>,

    /// Processing latency in milliseconds
    pub processing_time_ms: u64,

    /// Blob storage location of raw payload
    pub blob_url: String,

    /// Whether this is a replayed event
    pub is_replay: bool,

    /// Original event timestamp from GitHub
    pub github_timestamp: Option<String>,
}
```

## Session ID Strategy

### Session ID Format

Session IDs follow the pattern: `{repo_owner}/{repo_name}/{entity_type}/{entity_id}`

Examples:

- `microsoft/vscode/pull_request/1234`
- `octocat/hello-world/issue/567`
- `github/docs/repository/push`

### Session ID Generation Rules

1. **Pull Request Events**: `{owner}/{repo}/pull_request/{pr_number}`
2. **Issue Events**: `{owner}/{repo}/issue/{issue_number}`
3. **Push Events**: `{owner}/{repo}/repository/push`
4. **Release Events**: `{owner}/{repo}/repository/release`
5. **Repository Events**: `{owner}/{repo}/repository/{event_type}`

### Ordering Implications

- Events with the same session ID are processed sequentially
- Events with different session IDs can be processed in parallel
- Push events to the same repository are ordered
- PR and Issue events are ordered per individual PR/Issue

## Event Type Mapping

### GitHub Event to Entity Type Mapping

| GitHub Event | Entity Type | Session ID Pattern |
|--------------|-------------|-------------------|
| `pull_request` | `PullRequest` | `{owner}/{repo}/pull_request/{number}` |
| `pull_request_review` | `PullRequest` | `{owner}/{repo}/pull_request/{number}` |
| `pull_request_review_comment` | `PullRequest` | `{owner}/{repo}/pull_request/{number}` |
| `issues` | `Issue` | `{owner}/{repo}/issue/{number}` |
| `issue_comment` | `Issue` | `{owner}/{repo}/issue/{number}` |
| `push` | `Repository` | `{owner}/{repo}/repository/push` |
| `release` | `Repository` | `{owner}/{repo}/repository/release` |
| `create` | `Repository` | `{owner}/{repo}/repository/create` |
| `delete` | `Repository` | `{owner}/{repo}/repository/delete` |
| `check_run` | `CheckRun` | `{owner}/{repo}/check_run/{id}` |
| `check_suite` | `CheckSuite` | `{owner}/{repo}/check_suite/{id}` |

## Schema Evolution

### Versioning Strategy

- Schema version follows semantic versioning (e.g., "1.0.0")
- Minor version increments for backward-compatible additions
- Major version increments for breaking changes
- Bots MUST handle unknown fields gracefully (forward compatibility)

### Backward Compatibility Rules

1. Never remove required fields
2. Never change field types (breaking change)
3. New optional fields can be added freely
4. Enum variants can be extended (use `Other(String)` pattern)
5. Field renames require deprecation period with dual support

### Migration Strategy

```rust
impl NormalizedEvent {
    /// Migrate event from older schema version to current
    pub fn migrate_from_version(mut self, from_version: &str) -> Result<Self, MigrationError> {
        match from_version {
            "1.0.0" => {
                // Current version, no migration needed
                Ok(self)
            }
            version => Err(MigrationError::UnsupportedVersion(version.to_string()))
        }
    }
}
```

## Example Events

### Pull Request Opened Event

```json
{
  "event_id": "01H8X2K3M4N5P6Q7R8S9T0V1W2",
  "processed_at": "2025-09-18T10:30:45.123Z",
  "delivery_id": "12345678-1234-1234-1234-123456789abc",
  "repository": {
    "owner": "microsoft",
    "name": "vscode",
    "full_name": "microsoft/vscode",
    "id": 41881900,
    "private": false
  },
  "entity": {
    "entity_type": "PullRequest",
    "entity_id": "1234",
    "entity_ref": "PR #1234"
  },
  "session_id": "microsoft/vscode/pull_request/1234",
  "event_type": {
    "event": "pull_request",
    "action": "opened"
  },
  "payload": {
    // Complete original GitHub webhook payload
  },
  "metadata": {
    "schema_version": "1.0.0",
    "routed_to": ["task-tactician", "merge-warden"],
    "processing_time_ms": 45,
    "blob_url": "https://storage.blob.core.windows.net/webhooks/2025/09/18/01H8X2K3M4N5P6Q7R8S9T0V1W2.json",
    "is_replay": false,
    "github_timestamp": "2025-09-18T10:30:44.000Z"
  }
}
```

### Issue Comment Event

```json
{
  "event_id": "01H8X2K3M4N5P6Q7R8S9T0V1W3",
  "processed_at": "2025-09-18T10:31:12.456Z",
  "delivery_id": "87654321-4321-4321-4321-abcdef123456",
  "repository": {
    "owner": "octocat",
    "name": "hello-world",
    "full_name": "octocat/hello-world",
    "id": 583231,
    "private": false
  },
  "entity": {
    "entity_type": "Issue",
    "entity_id": "567",
    "entity_ref": "Issue #567"
  },
  "session_id": "octocat/hello-world/issue/567",
  "event_type": {
    "event": "issue_comment",
    "action": "created"
  },
  "payload": {
    // Complete original GitHub webhook payload
  },
  "metadata": {
    "schema_version": "1.0.0",
    "routed_to": ["task-tactician"],
    "processing_time_ms": 32,
    "blob_url": "https://storage.blob.core.windows.net/webhooks/2025/09/18/01H8X2K3M4N5P6Q7R8S9T0V1W3.json",
    "is_replay": false,
    "github_timestamp": "2025-09-18T10:31:11.500Z"
  }
}
```

### Push Event

```json
{
  "event_id": "01H8X2K3M4N5P6Q7R8S9T0V1W4",
  "processed_at": "2025-09-18T10:32:01.789Z",
  "delivery_id": "abcdef12-3456-7890-abcd-ef1234567890",
  "repository": {
    "owner": "github",
    "name": "docs",
    "full_name": "github/docs",
    "id": 9919,
    "private": false
  },
  "entity": {
    "entity_type": "Repository",
    "entity_id": "push",
    "entity_ref": "Repository Push"
  },
  "session_id": "github/docs/repository/push",
  "event_type": {
    "event": "push",
    "action": null
  },
  "payload": {
    // Complete original GitHub webhook payload
  },
  "metadata": {
    "schema_version": "1.0.0",
    "routed_to": ["spec-sentinel"],
    "processing_time_ms": 28,
    "blob_url": "https://storage.blob.core.windows.net/webhooks/2025/09/18/01H8X2K3M4N5P6Q7R8S9T0V1W4.json",
    "is_replay": false,
    "github_timestamp": "2025-09-18T10:32:00.000Z"
  }
}
```

## Service Bus Message Format

### Message Properties

Queue-Keeper sets the following Service Bus message properties:

```rust
// Standard Service Bus properties
message.session_id = normalized_event.session_id;
message.message_id = normalized_event.event_id;
message.content_type = "application/json";

// Custom properties for routing and filtering
message.properties.insert("event_type", normalized_event.event_type.event);
message.properties.insert("repository", normalized_event.repository.full_name);
message.properties.insert("entity_type", normalized_event.entity.entity_type.to_string());
message.properties.insert("processed_at", normalized_event.processed_at);
message.properties.insert("is_replay", normalized_event.metadata.is_replay.to_string());
```

### Message Body

The complete `NormalizedEvent` struct serialized as JSON.

## Error Handling Schema

### Processing Errors

When event processing fails, Queue-Keeper creates error events:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    /// Original event ID that failed
    pub original_event_id: String,

    /// Error classification
    pub error_type: ErrorType,

    /// Human-readable error message
    pub error_message: String,

    /// Detailed error context
    pub error_details: serde_json::Value,

    /// Number of retry attempts made
    pub retry_count: u32,

    /// Whether this event can be retried
    pub retryable: bool,

    /// Timestamp of the error
    pub error_timestamp: String,

    /// Original event data (if available)
    pub original_event: Option<NormalizedEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorType {
    SignatureValidation,
    PayloadParsing,
    BlobStorageFailure,
    QueueDeliveryFailure,
    ConfigurationError,
    UnknownEventType,
    InternalError,
}
```

## Validation Rules

### Event ID Validation

- Must be a valid ULID format
- Must be unique across all events
- Must be sortable chronologically

### Session ID Validation

- Must match pattern: `^[a-zA-Z0-9\-_.]+/[a-zA-Z0-9\-_.]+/(pull_request|issue|repository|check_run|check_suite)/[a-zA-Z0-9\-_.]+$`
- Maximum length: 256 characters
- Must not contain special characters that interfere with Service Bus sessions

### Repository Validation

- Owner and name must match GitHub naming conventions
- Full name must equal `{owner}/{name}`
- Repository ID must be positive integer

### Payload Validation

- Original payload must be valid JSON
- Must contain minimum required GitHub webhook fields
- Size limit: 1MB per payload (GitHub's limit)
