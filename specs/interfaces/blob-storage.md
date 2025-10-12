# Blob Storage Interface

**Architectural Layer**: Infrastructure Interface
**Module Path**: `src/storage.rs`
**Responsibilities** (from RDD):

- Knows: Blob naming conventions, metadata formats, Azure storage APIs
- Does: Persists webhook payloads, retrieves payloads for replay, manages blob metadata

## Dependencies

- Types: `EventId`, `EventEnvelope`, `WebhookRequest` (shared-types.md)
- Interfaces: None (infrastructure boundary)
- Shared: `Result<T, E>` (shared-types.md)

## Primary Traits

### BlobStorage

#### Purpose

Abstracts blob storage operations for webhook payload persistence and replay capabilities.

#### Interface Definition

```rust
#[async_trait]
pub trait BlobStorage: Send + Sync {
    /// Store webhook payload with metadata
    async fn store_payload(
        &self,
        event_id: &EventId,
        payload: &WebhookPayload,
    ) -> Result<BlobMetadata, BlobStorageError>;

    /// Retrieve stored payload by event ID
    async fn get_payload(
        &self,
        event_id: &EventId,
    ) -> Result<Option<StoredWebhook>, BlobStorageError>;

    /// List payloads by date range for replay
    async fn list_payloads(
        &self,
        filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError>;

    /// Delete payload (for retention policy)
    async fn delete_payload(
        &self,
        event_id: &EventId,
    ) -> Result<(), BlobStorageError>;

    /// Check blob storage health
    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError>;
}
```

## Supporting Types

### WebhookPayload

```rust
/// Webhook payload with metadata for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    /// Raw webhook payload bytes
    pub body: Bytes,

    /// HTTP headers from webhook request
    pub headers: HashMap<String, String>,

    /// Event metadata extracted during processing
    pub metadata: PayloadMetadata,
}

/// Metadata extracted during webhook processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadMetadata {
    /// Event ID (ULID)
    pub event_id: EventId,

    /// GitHub event type
    pub event_type: String,

    /// Repository information
    pub repository: Repository,

    /// Signature validation status
    pub signature_valid: bool,

    /// Processing timestamp
    pub received_at: Timestamp,

    /// GitHub delivery ID
    pub delivery_id: Option<String>,
}
```

### BlobMetadata

```rust
/// Metadata about stored blob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMetadata {
    /// Event ID used as blob identifier
    pub event_id: EventId,

    /// Blob path in storage
    pub blob_path: String,

    /// Size of stored payload in bytes
    pub size_bytes: u64,

    /// Content type (always application/json)
    pub content_type: String,

    /// When blob was created
    pub created_at: Timestamp,

    /// Payload metadata
    pub metadata: PayloadMetadata,
}
```

### StoredWebhook

```rust
/// Complete webhook data retrieved from storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredWebhook {
    /// Blob metadata
    pub metadata: BlobMetadata,

    /// Original webhook payload
    pub payload: WebhookPayload,
}
```

### PayloadFilter

```rust
/// Filter criteria for listing stored payloads
#[derive(Debug, Clone, Default)]
pub struct PayloadFilter {
    /// Date range for filtering
    pub date_range: Option<DateRange>,

    /// Repository filter
    pub repository: Option<String>,

    /// Event type filter
    pub event_type: Option<String>,

    /// Maximum number of results
    pub limit: Option<usize>,

    /// Skip this many results (for pagination)
    pub offset: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct DateRange {
    pub start: Timestamp,
    pub end: Timestamp,
}
```

### StorageHealthStatus

```rust
/// Health status of blob storage
#[derive(Debug, Clone)]
pub struct StorageHealthStatus {
    /// Overall health status
    pub healthy: bool,

    /// Connection status
    pub connected: bool,

    /// Last successful operation
    pub last_success: Option<Timestamp>,

    /// Error message if unhealthy
    pub error_message: Option<String>,

    /// Performance metrics
    pub metrics: StorageMetrics,
}

#[derive(Debug, Clone)]
pub struct StorageMetrics {
    /// Average write latency (ms)
    pub avg_write_latency_ms: f64,

    /// Average read latency (ms)
    pub avg_read_latency_ms: f64,

    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
}
```

## Error Types

### BlobStorageError

```rust
/// Errors that can occur during blob storage operations
#[derive(Debug, thiserror::Error)]
pub enum BlobStorageError {
    #[error("Connection failed: {message}")]
    ConnectionFailed { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Blob not found: {event_id}")]
    BlobNotFound { event_id: EventId },

    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },

    #[error("Storage quota exceeded")]
    QuotaExceeded,

    #[error("Invalid blob path: {path}")]
    InvalidPath { path: String },

    #[error("Serialization failed: {message}")]
    SerializationFailed { message: String },

    #[error("Network timeout: {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Internal storage error: {message}")]
    InternalError { message: String },
}

impl BlobStorageError {
    /// Check if error is transient and worth retrying
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed { .. }
                | Self::Timeout { .. }
                | Self::InternalError { .. }
        )
    }
}
```

## Blob Path Conventions

### Naming Convention

Blobs MUST be stored using the immutable naming convention as specified in REQ-002:

```
{container}/webhook-payloads/year={year}/month={month}/day={day}/hour={hour}/{event_id}.json
```

### Example Paths

```
webhook-payloads/year=2025/month=10/day=11/hour=14/01HKQZXJ9M2K8PQRST7VWX4Y3Z.json
webhook-payloads/year=2025/month=10/day=11/hour=14/01HKQZXJ9M2K8PQRST7VWX4Y40.json
```

### Path Generation

```rust
impl EventId {
    /// Generate blob path for storage
    pub fn to_blob_path(&self) -> String {
        let timestamp = self.timestamp();
        format!(
            "webhook-payloads/year={}/month={:02}/day={:02}/hour={:02}/{}.json",
            timestamp.year(),
            timestamp.month(),
            timestamp.day(),
            timestamp.hour(),
            self
        )
    }
}
```

## Usage Examples

### Store Webhook Payload

```rust
let storage = AzureBlobStorage::new(config).await?;

let payload = WebhookPayload {
    body: webhook_body,
    headers: webhook_headers,
    metadata: PayloadMetadata {
        event_id: event_id.clone(),
        event_type: "pull_request".to_string(),
        repository: repo.clone(),
        signature_valid: true,
        received_at: Timestamp::now(),
        delivery_id: Some("12345-67890".to_string()),
    },
};

let metadata = storage.store_payload(&event_id, &payload).await?;
println!("Stored payload at: {}", metadata.blob_path);
```

### Retrieve for Replay

```rust
let storage = AzureBlobStorage::new(config).await?;

match storage.get_payload(&event_id).await? {
    Some(stored_webhook) => {
        println!("Found payload: {} bytes", stored_webhook.metadata.size_bytes);
        // Process for replay
    }
    None => {
        println!("Payload not found for event: {}", event_id);
    }
}
```

### List for Batch Replay

```rust
let filter = PayloadFilter {
    date_range: Some(DateRange {
        start: Timestamp::from_date(2025, 10, 11)?,
        end: Timestamp::now(),
    }),
    repository: Some("owner/repo".to_string()),
    event_type: Some("pull_request".to_string()),
    limit: Some(100),
    offset: None,
};

let payloads = storage.list_payloads(&filter).await?;
for metadata in payloads {
    println!("Event: {} at {}", metadata.event_id, metadata.created_at);
}
```

## Implementation Notes

### REQ-002 Compliance

- All webhook payloads MUST be persisted immediately upon receipt
- Metadata MUST include all specified fields
- Naming convention MUST be immutable for audit compliance
- Storage MUST support replay scenarios

### Performance Requirements

- Storage operations MUST complete within 200ms (P95)
- Concurrent storage operations MUST be supported
- Failed storage MUST NOT block webhook processing
- Circuit breaker protection for storage failures

### Security Considerations

- Payloads may contain sensitive repository information
- Access control via Azure RBAC and Managed Identity
- Encryption at rest via Azure Storage Service Encryption
- Audit all storage access operations

### Retention and Lifecycle

- Hot tier: 90 days for active replay scenarios
- Cool tier: 91-365 days for compliance
- Archive tier: >365 days for long-term compliance
- Automatic lifecycle management via Azure policies
