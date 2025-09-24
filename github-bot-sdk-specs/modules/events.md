# Events Module

The events module defines the normalized event schema and processing utilities for GitHub webhook events. It provides type-safe event parsing, validation, and correlation with the Queue-Keeper event format.

## Overview

This module bridges the gap between raw GitHub webhook payloads and the bot processing system. It defines the canonical event format used throughout the system and provides utilities for event validation, normalization, and correlation.

## Core Types

### EventEnvelope

The primary event container that wraps all GitHub events in a normalized format.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub event_type: String,
    pub repository: Repository,
    pub entity_type: EntityType,
    pub entity_id: Option<String>,
    pub session_id: Option<String>,
    pub payload: EventPayload,
    pub metadata: EventMetadata,
    pub trace_context: Option<TraceContext>,
}

impl EventEnvelope {
    pub fn new(
        event_type: String,
        repository: Repository,
        payload: EventPayload,
    ) -> Self { ... }

    pub fn with_session_id(mut self, session_id: String) -> Self { ... }

    pub fn with_trace_context(mut self, context: TraceContext) -> Self { ... }

    pub fn entity_key(&self) -> String { ... }

    pub fn correlation_id(&self) -> &str { ... }
}
```

### EventId

Unique identifier for events, ensuring idempotency and deduplication.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(String);

impl EventId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_github_delivery(delivery_id: &str) -> Self {
        Self(format!("gh-{}", delivery_id))
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

impl Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

### EntityType

Classifies the primary entity involved in the event for session correlation.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    Repository,
    PullRequest,
    Issue,
    Branch,
    Release,
    User,
    Organization,
    CheckRun,
    CheckSuite,
    Deployment,
    Unknown,
}

impl EntityType {
    pub fn from_event_type(event_type: &str) -> Self { ... }

    pub fn supports_ordering(&self) -> bool {
        matches!(self, Self::PullRequest | Self::Issue | Self::Branch)
    }
}
```

### EventPayload

Container for the actual GitHub webhook payload data.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    inner: serde_json::Value,
}

impl EventPayload {
    pub fn new(value: serde_json::Value) -> Self {
        Self { inner: value }
    }

    pub fn raw(&self) -> &serde_json::Value { &self.inner }

    pub fn parse_pull_request(&self) -> Result<PullRequestEvent, EventError> { ... }

    pub fn parse_issue(&self) -> Result<IssueEvent, EventError> { ... }

    pub fn parse_push(&self) -> Result<PushEvent, EventError> { ... }

    pub fn parse_check_run(&self) -> Result<CheckRunEvent, EventError> { ... }

    pub fn parse_check_suite(&self) -> Result<CheckSuiteEvent, EventError> { ... }

    pub fn parse_release(&self) -> Result<ReleaseEvent, EventError> { ... }
}
```

### EventMetadata

Additional metadata about event processing and routing.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub received_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub source: EventSource,
    pub delivery_id: Option<String>,
    pub signature_valid: bool,
    pub retry_count: u32,
    pub routing_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSource {
    GitHub,
    Replay,
    Test,
}
```

## Typed Event Structures

The module provides strongly-typed structures for common GitHub events.

### Pull Request Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestEvent {
    pub action: PullRequestAction,
    pub number: u32,
    pub pull_request: PullRequest,
    pub repository: RepositoryInfo,
    pub sender: User,
    pub changes: Option<PullRequestChanges>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PullRequestAction {
    Opened,
    Closed,
    Reopened,
    Synchronize,
    Edited,
    Assigned,
    Unassigned,
    ReviewRequested,
    ReviewRequestRemoved,
    Labeled,
    Unlabeled,
    ReadyForReview,
    ConvertedToDraft,
}
```

### Issue Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueEvent {
    pub action: IssueAction,
    pub issue: Issue,
    pub repository: RepositoryInfo,
    pub sender: User,
    pub changes: Option<IssueChanges>,
    pub assignee: Option<User>,
    pub label: Option<Label>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueAction {
    Opened,
    Closed,
    Reopened,
    Edited,
    Assigned,
    Unassigned,
    Labeled,
    Unlabeled,
    Transferred,
    Pinned,
    Unpinned,
}
```

### Push Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEvent {
    pub ref_name: String,
    pub before: String,
    pub after: String,
    pub created: bool,
    pub deleted: bool,
    pub forced: bool,
    pub commits: Vec<Commit>,
    pub head_commit: Option<Commit>,
    pub repository: RepositoryInfo,
    pub pusher: User,
    pub sender: User,
}
```

### Check Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRunEvent {
    pub action: CheckRunAction,
    pub check_run: CheckRun,
    pub repository: RepositoryInfo,
    pub sender: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSuiteEvent {
    pub action: CheckSuiteAction,
    pub check_suite: CheckSuite,
    pub repository: RepositoryInfo,
    pub sender: User,
}
```

## Event Processing

### EventProcessor

Handles conversion from raw GitHub webhooks to normalized events.

```rust
pub struct EventProcessor {
    config: ProcessorConfig,
}

impl EventProcessor {
    pub fn new(config: ProcessorConfig) -> Self { ... }

    pub async fn process_webhook(
        &self,
        event_type: &str,
        payload: &[u8],
        delivery_id: Option<&str>,
    ) -> Result<EventEnvelope, EventError> { ... }

    pub fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        secret: &str,
    ) -> Result<(), EventError> { ... }

    pub fn extract_entity_info(
        &self,
        event_type: &str,
        payload: &serde_json::Value,
    ) -> Result<(EntityType, Option<String>), EventError> { ... }

    pub fn generate_session_id(
        &self,
        entity_type: &EntityType,
        entity_id: &Option<String>,
        repository: &Repository,
    ) -> Option<String> { ... }
}
```

### ProcessorConfig

Configuration for event processing behavior.

```rust
pub struct ProcessorConfig {
    pub enable_signature_validation: bool,
    pub enable_session_correlation: bool,
    pub session_id_strategy: SessionIdStrategy,
    pub max_payload_size: usize,
    pub trace_sampling_rate: f64,
}

#[derive(Debug, Clone)]
pub enum SessionIdStrategy {
    None,
    Entity,        // PR#123, Issue#456
    Repository,    // repo:owner/name
    Custom(Box<dyn Fn(&EventEnvelope) -> Option<String> + Send + Sync>),
}
```

## Webhook Validation

### SignatureValidator

Handles HMAC-SHA256 signature validation for GitHub webhooks.

```rust
pub struct SignatureValidator {
    secrets: Arc<dyn SecretProvider>,
}

impl SignatureValidator {
    pub fn new(secrets: impl SecretProvider + 'static) -> Self { ... }

    pub async fn validate(
        &self,
        payload: &[u8],
        signature: &str,
        repository: &Repository,
    ) -> Result<(), ValidationError> { ... }

    pub fn validate_with_secret(
        payload: &[u8],
        signature: &str,
        secret: &str,
    ) -> Result<(), ValidationError> { ... }
}

#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get_webhook_secret(&self, repository: &Repository) -> Result<String, SecretError>;
}
```

## Event Correlation

### SessionManager

Manages session IDs for ordered event processing.

```rust
pub struct SessionManager {
    strategy: SessionIdStrategy,
}

impl SessionManager {
    pub fn new(strategy: SessionIdStrategy) -> Self { ... }

    pub fn generate_session_id(&self, envelope: &EventEnvelope) -> Option<String> { ... }

    pub fn extract_ordering_key(&self, envelope: &EventEnvelope) -> Option<String> { ... }
}
```

### Built-in Session Strategies

```rust
impl SessionManager {
    pub fn entity_session_strategy() -> SessionIdStrategy {
        SessionIdStrategy::Custom(Box::new(|envelope| {
            match (&envelope.entity_type, &envelope.entity_id) {
                (EntityType::PullRequest, Some(id)) => Some(format!("pr-{}-{}", envelope.repository.full_name, id)),
                (EntityType::Issue, Some(id)) => Some(format!("issue-{}-{}", envelope.repository.full_name, id)),
                (EntityType::Branch, Some(id)) => Some(format!("branch-{}-{}", envelope.repository.full_name, id)),
                _ => None,
            }
        }))
    }

    pub fn repository_session_strategy() -> SessionIdStrategy {
        SessionIdStrategy::Custom(Box::new(|envelope| {
            Some(format!("repo-{}", envelope.repository.full_name))
        }))
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Invalid event payload: {message}")]
    InvalidPayload { message: String },

    #[error("Unsupported event type: {event_type}")]
    UnsupportedEventType { event_type: String },

    #[error("Signature validation failed")]
    InvalidSignature,

    #[error("Missing required field: {field}")]
    MissingField { field: String },

    #[error("Payload too large: {size} bytes (max: {max})")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("JSON parsing error: {source}")]
    JsonParsing { source: serde_json::Error },

    #[error("Secret provider error: {source}")]
    SecretProvider { source: Box<dyn std::error::Error + Send + Sync> },
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid signature format")]
    InvalidSignatureFormat,

    #[error("HMAC validation failed")]
    HmacValidationFailed,

    #[error("Missing signature header")]
    MissingSignature,

    #[error("Secret not found for repository: {repository}")]
    SecretNotFound { repository: String },
}
```

## Usage Examples

### Basic Event Processing

```rust
use github_bot_sdk::events::{EventProcessor, ProcessorConfig, SessionIdStrategy};

let config = ProcessorConfig {
    enable_signature_validation: true,
    enable_session_correlation: true,
    session_id_strategy: SessionIdStrategy::Entity,
    max_payload_size: 1024 * 1024, // 1MB
    trace_sampling_rate: 0.1,
};

let processor = EventProcessor::new(config);

// Process incoming webhook
let envelope = processor.process_webhook(
    "pull_request",
    payload_bytes,
    Some("12345-67890-abcdef"),
).await?;

println!("Event ID: {}", envelope.event_id);
println!("Repository: {}", envelope.repository.full_name);
println!("Entity: {:?} ({})", envelope.entity_type, envelope.entity_id.unwrap_or_default());
```

### Typed Event Handling

```rust
match envelope.event_type.as_str() {
    "pull_request" => {
        let pr_event = envelope.payload.parse_pull_request()?;

        match pr_event.action {
            PullRequestAction::Opened => {
                println!("New PR opened: #{}", pr_event.number);
            }
            PullRequestAction::Synchronize => {
                println!("PR #{} updated with new commits", pr_event.number);
            }
            _ => {}
        }
    }
    "issues" => {
        let issue_event = envelope.payload.parse_issue()?;

        if issue_event.action == IssueAction::Opened {
            println!("New issue opened: #{}", issue_event.issue.number);
        }
    }
    _ => {
        println!("Unhandled event type: {}", envelope.event_type);
    }
}
```

### Custom Session Strategy

```rust
let custom_strategy = SessionIdStrategy::Custom(Box::new(|envelope| {
    // Group all events for a repository by day
    let date = envelope.metadata.received_at.format("%Y-%m-%d");
    Some(format!("daily-{}-{}", envelope.repository.full_name, date))
}));

let config = ProcessorConfig {
    session_id_strategy: custom_strategy,
    ..Default::default()
};
```

### Signature Validation

```rust
use github_bot_sdk::events::SignatureValidator;

struct MySecretProvider {
    secrets: HashMap<String, String>,
}

#[async_trait]
impl SecretProvider for MySecretProvider {
    async fn get_webhook_secret(&self, repository: &Repository) -> Result<String, SecretError> {
        self.secrets
            .get(&repository.full_name)
            .cloned()
            .ok_or_else(|| SecretError::NotFound)
    }
}

let validator = SignatureValidator::new(MySecretProvider { secrets });

validator.validate(
    payload_bytes,
    "sha256=abc123...",
    &envelope.repository,
).await?;
```

## Testing Support

```rust
#[cfg(test)]
pub mod testing {
    use super::*;

    pub struct EventBuilder {
        event_type: String,
        repository: Repository,
        payload: serde_json::Value,
    }

    impl EventBuilder {
        pub fn pull_request() -> Self { ... }
        pub fn issue() -> Self { ... }
        pub fn push() -> Self { ... }

        pub fn with_action(mut self, action: &str) -> Self { ... }
        pub fn with_number(mut self, number: u32) -> Self { ... }
        pub fn with_repository(mut self, owner: &str, name: &str) -> Self { ... }

        pub fn build(self) -> EventEnvelope { ... }
    }

    // Test utilities
    pub fn sample_pull_request_event() -> EventEnvelope { ... }
    pub fn sample_issue_event() -> EventEnvelope { ... }
    pub fn sample_push_event() -> EventEnvelope { ... }
}
```

## Performance Characteristics

- **Event Processing**: ~1-3ms per event (parsing + validation)
- **Signature Validation**: ~0.5ms per webhook
- **Session ID Generation**: ~0.1ms per event
- **Memory Usage**: ~2KB per event envelope
- **Throughput**: 1000+ events/second per processor instance
