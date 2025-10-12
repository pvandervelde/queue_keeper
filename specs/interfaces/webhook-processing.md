# Webhook Processing Specification

**Module Path**: `crates/queue-keeper-core/src/webhook/mod.rs`

**Architectural Layer**: Core Domain (Business Logic)

**Responsibilities**: Validates GitHub webhook authenticity, normalizes webhook payloads into standard event format, extracts entity information for session management

## Dependencies

- Shared Types: `EventId`, `SessionId`, `Repository`, `Timestamp`, `ValidationError`
- External Traits: `SignatureValidator`, `PayloadStorer`, `EventNormalizer`
- Errors: `WebhookError`, `ValidationError`, `StorageError`

## Core Types

### WebhookRequest

Raw HTTP request data from GitHub webhooks.

```rust
#[derive(Debug, Clone)]
pub struct WebhookRequest {
    pub headers: WebhookHeaders,
    pub body: bytes::Bytes,
    pub received_at: Timestamp,
}

impl WebhookRequest {
    pub fn new(headers: WebhookHeaders, body: bytes::Bytes) -> Self;
    pub fn event_type(&self) -> &str;
    pub fn delivery_id(&self) -> &str;
    pub fn signature(&self) -> Option<&str>;
}
```

### WebhookHeaders

GitHub-specific HTTP headers required for processing.

```rust
#[derive(Debug, Clone)]
pub struct WebhookHeaders {
    pub event_type: String,           // X-GitHub-Event
    pub delivery_id: String,          // X-GitHub-Delivery
    pub signature: Option<String>,    // X-Hub-Signature-256
    pub user_agent: Option<String>,   // User-Agent
    pub content_type: String,         // Content-Type
}

impl WebhookHeaders {
    pub fn from_http_headers(headers: &http::HeaderMap) -> Result<Self, ValidationError>;
    pub fn validate(&self) -> Result<(), ValidationError>;
}
```

**Validation Rules**:

- `event_type` must be non-empty and match GitHub event types
- `delivery_id` must be valid UUID format
- `signature` required for non-ping events
- `content_type` must be "application/json"

### EventEnvelope

Normalized event structure after webhook processing.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub event_type: String,
    pub action: Option<String>,
    pub repository: Repository,
    pub entity: EventEntity,
    pub session_id: SessionId,
    pub correlation_id: CorrelationId,
    pub occurred_at: Timestamp,
    pub processed_at: Timestamp,
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    pub fn new(
        event_type: String,
        action: Option<String>,
        repository: Repository,
        entity: EventEntity,
        payload: serde_json::Value,
    ) -> Self;
}
```

### EventEntity

The primary GitHub object affected by the event (for session grouping).

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventEntity {
    PullRequest { number: u32 },
    Issue { number: u32 },
    Branch { name: String },
    Release { tag: String },
    Repository,
    Unknown,
}

impl EventEntity {
    pub fn from_payload(event_type: &str, payload: &serde_json::Value) -> Self;
    pub fn entity_type(&self) -> &'static str;
    pub fn entity_id(&self) -> String;
}
```

**Entity Extraction Rules**:

- Pull Request events → `PullRequest { number }`
- Issue events → `Issue { number }`
- Push events → `Branch { name }`
- Release events → `Release { tag }`
- Repository events → `Repository`
- Unknown events → `Unknown`

## Core Operations

### WebhookProcessor

Main interface for webhook processing pipeline.

```rust
#[async_trait]
pub trait WebhookProcessor: Send + Sync {
    async fn process_webhook(
        &self,
        request: WebhookRequest,
    ) -> Result<EventEnvelope, WebhookError>;

    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        event_type: &str,
    ) -> Result<(), ValidationError>;

    async fn store_raw_payload(
        &self,
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError>;

    async fn normalize_event(
        &self,
        request: &WebhookRequest,
    ) -> Result<EventEnvelope, NormalizationError>;
}
```

### SignatureValidator (External Trait)

Interface for GitHub webhook signature validation.

```rust
#[async_trait]
pub trait SignatureValidator: Send + Sync {
    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        secret_key: &str,
    ) -> Result<(), ValidationError>;

    async fn get_webhook_secret(&self, event_type: &str) -> Result<String, SecretError>;

    fn supports_constant_time_comparison(&self) -> bool;
}
```

**Contract Requirements**:

- Must use HMAC-SHA256 algorithm
- Must use constant-time comparison to prevent timing attacks
- Must retrieve secrets from secure storage (Key Vault)
- Must cache secrets for performance (5-minute TTL max)

### PayloadStorer (External Trait)

Interface for persisting raw webhook payloads for audit and replay.

```rust
#[async_trait]
pub trait PayloadStorer: Send + Sync {
    async fn store_payload(
        &self,
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError>;

    async fn retrieve_payload(
        &self,
        storage_ref: &StorageReference,
    ) -> Result<WebhookRequest, StorageError>;

    async fn list_payloads(
        &self,
        filters: PayloadFilters,
    ) -> Result<Vec<StorageReference>, StorageError>;
}

#[derive(Debug, Clone)]
pub struct StorageReference {
    pub blob_path: String,
    pub stored_at: Timestamp,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub enum ValidationStatus {
    Valid,
    InvalidSignature,
    MalformedPayload,
    UnknownEvent,
}
```

**Storage Requirements**:

- Immutable storage (never modify after creation)
- Organized by date hierarchy: `{year}/{month}/{day}/{event_id}.json`
- Include metadata: event type, repository, validation status
- Retention policy: configurable (default 90 days)

### EventNormalizer (External Trait)

Interface for transforming GitHub payloads to standard event format.

```rust
#[async_trait]
pub trait EventNormalizer: Send + Sync {
    async fn normalize_event(
        &self,
        request: &WebhookRequest,
    ) -> Result<EventEnvelope, NormalizationError>;

    fn extract_repository(&self, payload: &serde_json::Value) -> Result<Repository, ExtractionError>;

    fn extract_entity(&self, event_type: &str, payload: &serde_json::Value) -> EventEntity;

    fn generate_session_id(
        &self,
        repository: &Repository,
        entity: &EventEntity,
    ) -> SessionId;
}
```

**Normalization Rules**:

- Always generate new `EventId` (ULID)
- Extract `Repository` from payload (required for all events)
- Determine `EventEntity` based on event type and payload structure
- Generate `SessionId` from repository and entity for ordering
- Preserve original payload in `payload` field
- Set `processed_at` to current timestamp

## Error Types

### WebhookError

Top-level error for webhook processing failures.

```rust
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("Webhook validation failed: {0}")]
    Validation(#[from] ValidationError),

    #[error("Signature validation failed: {0}")]
    InvalidSignature(String),

    #[error("Payload storage failed: {0}")]
    Storage(#[from] StorageError),

    #[error("Event normalization failed: {0}")]
    Normalization(#[from] NormalizationError),

    #[error("Unknown event type: {event_type}")]
    UnknownEventType { event_type: String },

    #[error("Malformed payload: {message}")]
    MalformedPayload { message: String },
}

impl WebhookError {
    pub fn is_transient(&self) -> bool;
    pub fn error_category(&self) -> ErrorCategory;
    pub fn should_retry(&self) -> bool;
}
```

### NormalizationError

Errors during event normalization process.

```rust
#[derive(Debug, thiserror::Error)]
pub enum NormalizationError {
    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },

    #[error("Invalid field format: {field} - {message}")]
    InvalidFieldFormat { field: String, message: String },

    #[error("Repository extraction failed: {0}")]
    RepositoryExtraction(#[from] ExtractionError),

    #[error("JSON parsing failed: {0}")]
    JsonParsing(#[from] serde_json::Error),
}
```

## Processing Pipeline

### Webhook Processing Flow

```rust
impl WebhookProcessor for DefaultWebhookProcessor {
    async fn process_webhook(
        &self,
        request: WebhookRequest,
    ) -> Result<EventEnvelope, WebhookError> {
        // 1. Validate headers and basic structure
        request.headers.validate()?;

        // 2. Validate webhook signature (if present)
        if let Some(signature) = &request.headers.signature {
            self.validate_signature(
                &request.body,
                signature,
                &request.headers.event_type,
            ).await?;
        }

        // 3. Store raw payload for audit/replay
        let validation_status = ValidationStatus::Valid;
        self.store_raw_payload(&request, validation_status).await?;

        // 4. Normalize to standard event format
        let event_envelope = self.normalize_event(&request).await?;

        Ok(event_envelope)
    }
}
```

### Error Handling Strategy

```rust
pub fn classify_webhook_error(error: &WebhookError) -> (ErrorCategory, bool) {
    match error {
        WebhookError::InvalidSignature(_) => (ErrorCategory::Security, false),
        WebhookError::UnknownEventType { .. } => (ErrorCategory::Permanent, false),
        WebhookError::MalformedPayload { .. } => (ErrorCategory::Permanent, false),
        WebhookError::Storage(storage_error) => {
            if storage_error.is_transient() {
                (ErrorCategory::Transient, true)
            } else {
                (ErrorCategory::Permanent, false)
            }
        }
        WebhookError::Validation(_) => (ErrorCategory::Permanent, false),
        WebhookError::Normalization(_) => (ErrorCategory::Permanent, false),
    }
}
```

## Usage Examples

### Basic Webhook Processing

```rust
use queue_keeper_core::webhook::{WebhookProcessor, WebhookRequest, WebhookHeaders};

async fn handle_webhook(
    processor: &dyn WebhookProcessor,
    headers: http::HeaderMap,
    body: bytes::Bytes,
) -> Result<EventEnvelope, WebhookError> {
    // Parse webhook headers
    let webhook_headers = WebhookHeaders::from_http_headers(&headers)?;

    // Create webhook request
    let request = WebhookRequest::new(webhook_headers, body);

    // Process through pipeline
    let event = processor.process_webhook(request).await?;

    tracing::info!(
        event_id = %event.event_id,
        event_type = %event.event_type,
        repository = %event.repository.full_name,
        session_id = %event.session_id,
        "Successfully processed webhook"
    );

    Ok(event)
}
```

### Entity Extraction

```rust
use queue_keeper_core::webhook::EventEntity;

fn example_entity_extraction() {
    // Pull request event
    let pr_payload = json!({
        "action": "opened",
        "pull_request": {
            "number": 123
        }
    });
    let entity = EventEntity::from_payload("pull_request", &pr_payload);
    assert_eq!(entity, EventEntity::PullRequest { number: 123 });

    // Push event
    let push_payload = json!({
        "ref": "refs/heads/main",
        "commits": [...]
    });
    let entity = EventEntity::from_payload("push", &push_payload);
    assert_eq!(entity, EventEntity::Branch { name: "main".to_string() });
}
```

### Session ID Generation

```rust
use queue_keeper_core::webhook::EventEntity;
use queue_keeper_core::{SessionId, Repository};

fn generate_session_id(repository: &Repository, entity: &EventEntity) -> SessionId {
    let entity_type = entity.entity_type();
    let entity_id = entity.entity_id();

    SessionId::from_parts(
        repository.owner_name(),
        repository.repo_name(),
        entity_type,
        &entity_id,
    )
}

// Results in session IDs like:
// - "microsoft/vscode/pull_request/123"
// - "github/docs/issue/456"
// - "owner/repo/branch/main"
```

## Performance Characteristics

### Processing Latency

- Header validation: < 1ms
- Signature validation: < 10ms (including secret retrieval)
- Payload storage: < 50ms (Azure Blob Storage)
- Event normalization: < 5ms
- **Total target**: < 100ms (95th percentile)

### Memory Usage

- Webhook request: ~1KB baseline + payload size
- Event envelope: ~2KB baseline + payload size
- Maximum payload size: 1MB (GitHub limit)
- Memory cleanup: Automatic on request completion

### Error Recovery

- Transient storage failures: Retry with exponential backoff
- Secret retrieval failures: Use cached secret if available
- Unknown event types: Continue processing but log warning
- Malformed payloads: Fail fast with detailed error context

## Security Considerations

### Signature Validation

- Always validate signatures for non-ping events
- Use constant-time comparison to prevent timing attacks
- Reject requests with invalid or missing signatures
- Cache secrets securely with limited TTL

### Input Sanitization

- Validate all header values for proper format
- Limit payload size to prevent DoS attacks
- Sanitize string inputs to prevent injection
- Parse JSON with depth limits for security

### Audit Requirements

- Log all webhook processing attempts
- Store raw payloads for compliance and debugging
- Record validation failures for security monitoring
- Include correlation IDs for end-to-end tracing

This webhook processing specification ensures secure, reliable, and performant processing of GitHub webhooks while maintaining clear separation between business logic and infrastructure concerns.
