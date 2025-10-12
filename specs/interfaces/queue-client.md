# Queue Client Specification

**Module Path**: `crates/queue-runtime/src/lib.rs`

**Architectural Layer**: Core Domain (Queue Abstraction)

**Responsibilities**: Provides provider-agnostic queue operations with consistent behavior across Azure Service Bus, AWS SQS, and testing implementations

## Dependencies

- Shared Types: `Result`, `Timestamp`, `ValidationError`
- External Traits: `QueueProvider`, `SessionProvider`, `MessageSerializer`
- Async: `async_trait`, `tokio`
- Serialization: `serde`, `bytes`

## Core Types

### QueueName

Validated queue name that follows provider naming conventions.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueueName(String);

impl QueueName {
    pub fn new(name: String) -> Result<Self, ValidationError>;
    pub fn as_str(&self) -> &str;
    pub fn with_prefix(prefix: &str, base_name: &str) -> Result<Self, ValidationError>;
}
```

**Validation Rules**:

- Length: 1-260 characters
- Characters: ASCII alphanumeric, hyphens, underscores
- No consecutive hyphens or leading/trailing hyphens
- Compatible with Azure Service Bus and AWS SQS naming

### MessageId

Unique identifier for messages within the queue system.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl MessageId {
    pub fn new() -> Self;
    pub fn from_str(s: &str) -> Result<Self, ValidationError>;
    pub fn as_str(&self) -> &str;
}
```

### Message

A message to be sent through the queue system.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub body: bytes::Bytes,
    pub attributes: HashMap<String, String>,
    pub session_id: Option<SessionId>,
    pub correlation_id: Option<String>,
    pub time_to_live: Option<Duration>,
}

impl Message {
    pub fn new(body: bytes::Bytes) -> Self;
    pub fn with_session_id(mut self, session_id: SessionId) -> Self;
    pub fn with_attribute(mut self, key: String, value: String) -> Self;
    pub fn with_correlation_id(mut self, correlation_id: String) -> Self;
    pub fn with_ttl(mut self, ttl: Duration) -> Self;
}
```

### ReceivedMessage

A message received from the queue with processing metadata.

```rust
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub message_id: MessageId,
    pub body: bytes::Bytes,
    pub attributes: HashMap<String, String>,
    pub session_id: Option<SessionId>,
    pub correlation_id: Option<String>,
    pub receipt_handle: ReceiptHandle,
    pub delivery_count: u32,
    pub first_delivered_at: Timestamp,
    pub delivered_at: Timestamp,
}

impl ReceivedMessage {
    pub fn message(&self) -> Message;
    pub fn has_exceeded_max_delivery_count(&self, max_count: u32) -> bool;
}
```

### ReceiptHandle

Opaque token for acknowledging or rejecting received messages.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptHandle {
    handle: String,
    expires_at: Timestamp,
    provider_type: ProviderType,
}

impl ReceiptHandle {
    pub fn new(handle: String, expires_at: Timestamp, provider_type: ProviderType) -> Self;
    pub fn is_expired(&self) -> bool;
    pub fn time_until_expiry(&self) -> Duration;
}
```

### SessionId

Identifier for grouping related messages for ordered processing.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: String) -> Result<Self, ValidationError>;
    pub fn as_str(&self) -> &str;
}
```

**Session ID Requirements**:

- Maximum 128 characters
- ASCII printable characters only
- Used for FIFO ordering within session
- Maps to Azure Service Bus sessions or AWS SQS message groups

## Core Operations

### QueueClient

Main interface for queue operations across all providers.

```rust
#[async_trait]
pub trait QueueClient: Send + Sync {
    async fn send_message(
        &self,
        queue: &QueueName,
        message: Message,
    ) -> Result<MessageId, QueueError>;

    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError>;

    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError>;

    async fn complete_message(
        &self,
        receipt: ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn abandon_message(
        &self,
        receipt: ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn dead_letter_message(
        &self,
        receipt: ReceiptHandle,
        reason: String,
    ) -> Result<(), QueueError>;

    async fn accept_session(
        &self,
        queue: &QueueName,
        session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionClient>, QueueError>;

    fn provider_type(&self) -> ProviderType;
    fn supports_sessions(&self) -> bool;
    fn supports_batching(&self) -> bool;
}
```

### SessionClient

Interface for session-based ordered message processing.

```rust
#[async_trait]
pub trait SessionClient: Send + Sync {
    async fn receive_message(
        &self,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    async fn complete_message(
        &self,
        receipt: ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn abandon_message(
        &self,
        receipt: ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn dead_letter_message(
        &self,
        receipt: ReceiptHandle,
        reason: String,
    ) -> Result<(), QueueError>;

    async fn renew_session_lock(&self) -> Result<(), QueueError>;

    async fn close_session(&self) -> Result<(), QueueError>;

    fn session_id(&self) -> &SessionId;
    fn session_expires_at(&self) -> Timestamp;
}
```

### QueueProvider (External Trait)

Interface implemented by specific queue providers (Azure, AWS, etc.).

```rust
#[async_trait]
pub trait QueueProvider: Send + Sync {
    async fn send_message(
        &self,
        queue: &QueueName,
        message: &Message,
    ) -> Result<MessageId, QueueError>;

    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError>;

    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError>;

    async fn complete_message(
        &self,
        receipt: &ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn abandon_message(
        &self,
        receipt: &ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn dead_letter_message(
        &self,
        receipt: &ReceiptHandle,
        reason: &str,
    ) -> Result<(), QueueError>;

    async fn create_session_client(
        &self,
        queue: &QueueName,
        session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError>;

    fn provider_type(&self) -> ProviderType;
    fn supports_sessions(&self) -> SessionSupport;
    fn supports_batching(&self) -> bool;
    fn max_batch_size(&self) -> u32;
}
```

### SessionProvider (External Trait)

Interface implemented by provider-specific session implementations.

```rust
#[async_trait]
pub trait SessionProvider: Send + Sync {
    async fn receive_message(
        &self,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    async fn complete_message(
        &self,
        receipt: &ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn abandon_message(
        &self,
        receipt: &ReceiptHandle,
    ) -> Result<(), QueueError>;

    async fn dead_letter_message(
        &self,
        receipt: &ReceiptHandle,
        reason: &str,
    ) -> Result<(), QueueError>;

    async fn renew_session_lock(&self) -> Result<(), QueueError>;

    async fn close_session(&self) -> Result<(), QueueError>;

    fn session_id(&self) -> &SessionId;
    fn session_expires_at(&self) -> Timestamp;
}
```

## Provider Types and Capabilities

### ProviderType

Enumeration of supported queue providers.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderType {
    AzureServiceBus,
    AwsSqs,
    InMemory,
}

impl ProviderType {
    pub fn supports_sessions(&self) -> SessionSupport;
    pub fn supports_batching(&self) -> bool;
    pub fn max_message_size(&self) -> usize;
}
```

### SessionSupport

Level of session support provided by different providers.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSupport {
    Native,      // Provider has built-in session support (Azure Service Bus)
    Emulated,    // Provider emulates sessions via other mechanisms (AWS SQS FIFO)
    Unsupported, // Provider cannot support session ordering
}
```

## Error Types

### QueueError

Comprehensive error type for all queue operations.

```rust
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue not found: {queue_name}")]
    QueueNotFound { queue_name: String },

    #[error("Message not found or receipt expired: {receipt}")]
    MessageNotFound { receipt: String },

    #[error("Session '{session_id}' is locked until {locked_until}")]
    SessionLocked { session_id: String, locked_until: Timestamp },

    #[error("Session '{session_id}' not found or expired")]
    SessionNotFound { session_id: String },

    #[error("Operation timed out after {duration:?}")]
    Timeout { duration: Duration },

    #[error("Connection failed: {message}")]
    ConnectionFailed { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Permission denied for operation: {operation}")]
    PermissionDenied { operation: String },

    #[error("Message too large: {size} bytes (max: {max_size})")]
    MessageTooLarge { size: usize, max_size: usize },

    #[error("Batch size {size} exceeds maximum {max_size}")]
    BatchTooLarge { size: usize, max_size: usize },

    #[error("Provider error ({provider}): {code} - {message}")]
    ProviderError {
        provider: String,
        code: String,
        message: String,
    },

    #[error("Serialization failed: {0}")]
    SerializationError(#[from] SerializationError),

    #[error("Configuration error: {0}")]
    ConfigurationError(#[from] ConfigurationError),
}

impl QueueError {
    pub fn is_transient(&self) -> bool;
    pub fn should_retry(&self) -> bool;
    pub fn retry_after(&self) -> Option<Duration>;
}
```

### SerializationError

Errors during message serialization/deserialization.

```rust
#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    #[error("JSON serialization failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Message body is not valid UTF-8")]
    InvalidUtf8,

    #[error("Message attribute '{key}' has invalid value")]
    InvalidAttribute { key: String },

    #[error("Message exceeds size limit: {size} bytes")]
    MessageTooLarge { size: usize },
}
```

## Configuration

### QueueConfig

Configuration for queue client initialization.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub provider: ProviderConfig,
    pub default_timeout: Duration,
    pub max_retry_attempts: u32,
    pub retry_base_delay: Duration,
    pub enable_dead_letter: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderConfig {
    AzureServiceBus(AzureServiceBusConfig),
    AwsSqs(AwsSqsConfig),
    InMemory(InMemoryConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureServiceBusConfig {
    pub connection_string: String,
    pub namespace: String,
    pub use_sessions: bool,
    pub session_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsSqsConfig {
    pub region: String,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub use_fifo_queues: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryConfig {
    pub max_queue_size: usize,
    pub enable_persistence: bool,
}
```

## Client Factory

### QueueClientFactory

Factory for creating queue clients with appropriate providers.

```rust
pub struct QueueClientFactory;

impl QueueClientFactory {
    pub async fn create_client(
        config: QueueConfig,
    ) -> Result<Box<dyn QueueClient>, QueueError> {
        match config.provider {
            ProviderConfig::AzureServiceBus(azure_config) => {
                let provider = AzureServiceBusProvider::new(azure_config).await?;
                Ok(Box::new(StandardQueueClient::new(provider)))
            }
            ProviderConfig::AwsSqs(aws_config) => {
                let provider = AwsSqsProvider::new(aws_config).await?;
                Ok(Box::new(StandardQueueClient::new(provider)))
            }
            ProviderConfig::InMemory(memory_config) => {
                let provider = InMemoryProvider::new(memory_config);
                Ok(Box::new(StandardQueueClient::new(provider)))
            }
        }
    }

    pub fn create_test_client() -> Box<dyn QueueClient> {
        let provider = InMemoryProvider::default();
        Box::new(StandardQueueClient::new(provider))
    }
}
```

## Usage Examples

### Basic Message Operations

```rust
use queue_runtime::{QueueClient, QueueName, Message, QueueClientFactory, QueueConfig};

async fn basic_queue_operations() -> Result<(), QueueError> {
    // Create queue client
    let config = QueueConfig::default_azure_service_bus();
    let client = QueueClientFactory::create_client(config).await?;

    let queue_name = QueueName::new("test-queue".to_string())?;

    // Send a message
    let message = Message::new("Hello, World!".into())
        .with_attribute("source".to_string(), "example".to_string())
        .with_correlation_id("correlation-123".to_string());

    let message_id = client.send_message(&queue_name, message).await?;
    println!("Sent message: {}", message_id.as_str());

    // Receive and process message
    let timeout = Duration::from_secs(30);
    if let Some(received) = client.receive_message(&queue_name, timeout).await? {
        println!("Received: {}", String::from_utf8_lossy(&received.body));

        // Complete the message
        client.complete_message(received.receipt_handle).await?;
    }

    Ok(())
}
```

### Session-Based Processing

```rust
async fn session_based_processing(
    client: &dyn QueueClient,
    queue_name: &QueueName,
) -> Result<(), QueueError> {
    // Accept a session (any available session)
    let session_client = client.accept_session(queue_name, None).await?;

    println!("Accepted session: {}", session_client.session_id().as_str());

    // Process messages in order within the session
    loop {
        let timeout = Duration::from_secs(10);
        match session_client.receive_message(timeout).await? {
            Some(message) => {
                // Process message
                let body = String::from_utf8_lossy(&message.body);
                println!("Processing: {}", body);

                // Complete message to advance session
                session_client.complete_message(message.receipt_handle).await?;
            }
            None => {
                // No more messages in session
                break;
            }
        }
    }

    // Close session when done
    session_client.close_session().await?;
    Ok(())
}
```

### Batch Operations

```rust
async fn batch_operations(
    client: &dyn QueueClient,
    queue_name: &QueueName,
) -> Result<(), QueueError> {
    if !client.supports_batching() {
        return Err(QueueError::ConfigurationError(
            "Provider doesn't support batching".into()
        ));
    }

    // Send multiple messages in a single batch
    let messages = vec![
        Message::new("Message 1".into()),
        Message::new("Message 2".into()),
        Message::new("Message 3".into()),
    ];

    let message_ids = client.send_messages(queue_name, messages).await?;
    println!("Sent {} messages in batch", message_ids.len());

    // Receive multiple messages
    let max_messages = 10;
    let timeout = Duration::from_secs(30);
    let received = client.receive_messages(queue_name, max_messages, timeout).await?;

    for message in received {
        // Process each message
        println!("Received: {}", String::from_utf8_lossy(&message.body));
        client.complete_message(message.receipt_handle).await?;
    }

    Ok(())
}
```

### Error Handling and Retry

```rust
async fn robust_message_processing(
    client: &dyn QueueClient,
    queue_name: &QueueName,
) -> Result<(), QueueError> {
    let timeout = Duration::from_secs(30);

    match client.receive_message(queue_name, timeout).await? {
        Some(message) => {
            // Attempt to process message
            match process_message(&message.body).await {
                Ok(_) => {
                    // Success - complete the message
                    client.complete_message(message.receipt_handle).await?;
                }
                Err(e) if is_transient_error(&e) => {
                    // Transient error - abandon for retry
                    client.abandon_message(message.receipt_handle).await?;
                }
                Err(e) => {
                    // Permanent error - send to dead letter queue
                    let reason = format!("Processing failed: {}", e);
                    client.dead_letter_message(message.receipt_handle, reason).await?;
                }
            }
        }
        None => {
            // No messages available
        }
    }

    Ok(())
}

async fn process_message(body: &[u8]) -> Result<(), ProcessingError> {
    // Your message processing logic here
    unimplemented!()
}

fn is_transient_error(error: &ProcessingError) -> bool {
    // Your error classification logic here
    unimplemented!()
}
```

## Performance Characteristics

### Throughput Targets

- Single message operations: 1000 ops/second
- Batch operations: 10000 messages/second
- Session operations: 100 sessions/second
- Provider switching overhead: < 1ms

### Latency Targets

- Send message: < 10ms (within region)
- Receive message: < 50ms (with 1s timeout)
- Complete message: < 5ms
- Session operations: < 100ms

### Resource Usage

- Memory per client: < 10MB baseline
- Connection pooling: Shared across operations
- Thread usage: Async-only, no thread blocking
- Cleanup: Automatic resource cleanup on drop

## Provider Compatibility Matrix

| Feature | Azure Service Bus | AWS SQS | In-Memory |
|---------|-------------------|---------|-----------|
| Basic Operations | ✅ | ✅ | ✅ |
| Sessions | ✅ Native | 🔄 FIFO Groups | ✅ Simulated |
| Batching | ✅ Up to 100 | ✅ Up to 10 | ✅ Unlimited |
| Dead Letter | ✅ Native | ✅ Native | ✅ Simulated |
| TTL | ✅ Native | ✅ Native | ✅ Simulated |
| Duplicate Detection | ✅ Native | ✅ FIFO Only | ✅ Simulated |

This queue client specification provides a unified interface for queue operations while maintaining the specific capabilities and optimizations of each provider.
