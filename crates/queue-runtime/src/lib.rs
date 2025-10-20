//! # Queue Runtime
//!
//! Multi-provider queue runtime for reliable message processing with support for
//! Azure Service Bus, AWS SQS, and in-memory implementations.
//!
//! This library provides:
//! - Provider-agnostic queue operations
//! - Session-based ordered message processing
//! - Dead letter queue support
//! - Retry policies with exponential backoff
//! - Batch operations where supported
//!
//! See specs/interfaces/queue-client.md for complete specification.

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};
use thiserror::Error;

// ============================================================================
// Core Types
// ============================================================================

/// Validated queue name that follows provider naming conventions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueueName(String);

impl QueueName {
    /// Create new queue name with validation
    pub fn new(name: String) -> Result<Self, ValidationError> {
        // Validate length
        if name.is_empty() || name.len() > 260 {
            return Err(ValidationError::OutOfRange {
                field: "queue_name".to_string(),
                message: "must be 1-260 characters".to_string(),
            });
        }

        // Validate characters (ASCII alphanumeric, hyphens, underscores)
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ValidationError::InvalidFormat {
                field: "queue_name".to_string(),
                message: "only ASCII alphanumeric, hyphens, and underscores allowed".to_string(),
            });
        }

        // Validate no consecutive hyphens or leading/trailing hyphens
        if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
            return Err(ValidationError::InvalidFormat {
                field: "queue_name".to_string(),
                message: "no leading/trailing hyphens or consecutive hyphens".to_string(),
            });
        }

        Ok(Self(name))
    }

    /// Create queue name with prefix
    pub fn with_prefix(prefix: &str, base_name: &str) -> Result<Self, ValidationError> {
        let full_name = format!("{}-{}", prefix, base_name);
        Self::new(full_name)
    }

    /// Get queue name as string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for QueueName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for QueueName {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

/// Unique identifier for messages within the queue system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl MessageId {
    /// Generate new random message ID
    pub fn new() -> Self {
        let id = uuid::Uuid::new_v4();
        Self(id.to_string())
    }

    /// Get message ID as string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for MessageId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ValidationError::Required {
                field: "message_id".to_string(),
            });
        }

        Ok(Self(s.to_string()))
    }
}

/// Identifier for grouping related messages for ordered processing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Create new session ID with validation
    pub fn new(id: String) -> Result<Self, ValidationError> {
        if id.is_empty() {
            return Err(ValidationError::Required {
                field: "session_id".to_string(),
            });
        }

        if id.len() > 128 {
            return Err(ValidationError::OutOfRange {
                field: "session_id".to_string(),
                message: "maximum 128 characters".to_string(),
            });
        }

        // Validate ASCII printable characters only
        if !id.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
            return Err(ValidationError::InvalidFormat {
                field: "session_id".to_string(),
                message: "only ASCII printable characters allowed".to_string(),
            });
        }

        Ok(Self(id))
    }

    /// Create session ID from parts (for GitHub events)
    pub fn from_parts(owner: &str, repo: &str, entity_type: &str, entity_id: &str) -> Self {
        let id = format!("{}/{}/{}/{}", owner, repo, entity_type, entity_id);
        // Use unchecked creation since we control the format
        Self(id)
    }

    /// Get session ID as string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

/// Timestamp wrapper for consistent time handling
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Create timestamp for current time
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Create timestamp from DateTime
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }

    /// Get underlying DateTime
    pub fn as_datetime(&self) -> DateTime<Utc> {
        self.0
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%d %H:%M:%S UTC"))
    }
}

impl FromStr for Timestamp {
    type Err = chrono::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let dt = s.parse::<DateTime<Utc>>()?;
        Ok(Self::from_datetime(dt))
    }
}

/// A message to be sent through the queue system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(with = "bytes_serde")]
    pub body: Bytes,
    pub attributes: HashMap<String, String>,
    pub session_id: Option<SessionId>,
    pub correlation_id: Option<String>,
    pub time_to_live: Option<Duration>,
}

/// Custom serialization for Bytes
mod bytes_serde {
    use base64::{engine::general_purpose, Engine as _};
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = general_purpose::STANDARD.encode(bytes);
        encoded.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let decoded = general_purpose::STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)?;
        Ok(Bytes::from(decoded))
    }
}

impl Message {
    /// Create new message with body
    pub fn new(body: Bytes) -> Self {
        Self {
            body,
            attributes: HashMap::new(),
            session_id: None,
            correlation_id: None,
            time_to_live: None,
        }
    }

    /// Add session ID for ordered processing
    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Add message attribute
    pub fn with_attribute(mut self, key: String, value: String) -> Self {
        self.attributes.insert(key, value);
        self
    }

    /// Add correlation ID for tracking
    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Add time-to-live for message expiration
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.time_to_live = Some(ttl);
        self
    }
}

/// A message received from the queue with processing metadata
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub message_id: MessageId,
    #[allow(dead_code)]
    pub body: Bytes,
    pub attributes: HashMap<String, String>,
    pub session_id: Option<SessionId>,
    pub correlation_id: Option<String>,
    pub receipt_handle: ReceiptHandle,
    pub delivery_count: u32,
    pub first_delivered_at: Timestamp,
    pub delivered_at: Timestamp,
}

impl ReceivedMessage {
    /// Convert back to Message (for forwarding/replaying)
    pub fn message(&self) -> Message {
        Message {
            body: self.body.clone(),
            attributes: self.attributes.clone(),
            session_id: self.session_id.clone(),
            correlation_id: self.correlation_id.clone(),
            time_to_live: None, // TTL is not preserved in received messages
        }
    }

    /// Check if message has exceeded maximum delivery count
    pub fn has_exceeded_max_delivery_count(&self, max_count: u32) -> bool {
        self.delivery_count > max_count
    }
}

/// Opaque token for acknowledging or rejecting received messages
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptHandle {
    handle: String,
    expires_at: Timestamp,
    provider_type: ProviderType,
}

impl ReceiptHandle {
    /// Create new receipt handle
    pub fn new(handle: String, expires_at: Timestamp, provider_type: ProviderType) -> Self {
        Self {
            handle,
            expires_at,
            provider_type,
        }
    }

    /// Get handle string
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// Check if receipt handle is expired
    pub fn is_expired(&self) -> bool {
        Timestamp::now() >= self.expires_at
    }

    /// Get time until expiry
    pub fn time_until_expiry(&self) -> Duration {
        let now = Timestamp::now();
        if now >= self.expires_at {
            Duration::zero()
        } else {
            self.expires_at.as_datetime() - now.as_datetime()
        }
    }

    /// Get provider type
    pub fn provider_type(&self) -> ProviderType {
        self.provider_type
    }
}

// ============================================================================
// Provider Types and Capabilities
// ============================================================================

/// Enumeration of supported queue providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderType {
    AzureServiceBus,
    AwsSqs,
    InMemory,
}

impl ProviderType {
    /// Get session support level for provider
    pub fn supports_sessions(&self) -> SessionSupport {
        match self {
            Self::AzureServiceBus => SessionSupport::Native,
            Self::AwsSqs => SessionSupport::Emulated, // Via FIFO queues
            Self::InMemory => SessionSupport::Native,
        }
    }

    /// Check if provider supports batch operations
    pub fn supports_batching(&self) -> bool {
        match self {
            Self::AzureServiceBus => true,
            Self::AwsSqs => true,
            Self::InMemory => true,
        }
    }

    /// Get maximum message size for provider
    pub fn max_message_size(&self) -> usize {
        match self {
            Self::AzureServiceBus => 1024 * 1024, // 1MB
            Self::AwsSqs => 256 * 1024,           // 256KB
            Self::InMemory => 10 * 1024 * 1024,   // 10MB
        }
    }
}

/// Level of session support provided by different providers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSupport {
    /// Provider has built-in session support (Azure Service Bus)
    Native,
    /// Provider emulates sessions via other mechanisms (AWS SQS FIFO)
    Emulated,
    /// Provider cannot support session ordering
    Unsupported,
}

// ============================================================================
// Core Trait Definitions
// ============================================================================

/// Main interface for queue operations across all providers
#[async_trait]
pub trait QueueClient: Send + Sync {
    /// Send single message to queue
    async fn send_message(
        &self,
        queue: &QueueName,
        message: Message,
    ) -> Result<MessageId, QueueError>;

    /// Send multiple messages in batch (if supported)
    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError>;

    /// Receive single message from queue
    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    /// Receive multiple messages from queue
    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError>;

    /// Mark message as successfully processed
    async fn complete_message(&self, receipt: ReceiptHandle) -> Result<(), QueueError>;

    /// Return message to queue for retry
    async fn abandon_message(&self, receipt: ReceiptHandle) -> Result<(), QueueError>;

    /// Send message to dead letter queue
    async fn dead_letter_message(
        &self,
        receipt: ReceiptHandle,
        reason: String,
    ) -> Result<(), QueueError>;

    /// Accept session for ordered processing
    async fn accept_session(
        &self,
        queue: &QueueName,
        session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionClient>, QueueError>;

    /// Get provider type
    fn provider_type(&self) -> ProviderType;

    /// Check if provider supports sessions
    fn supports_sessions(&self) -> bool;

    /// Check if provider supports batch operations
    fn supports_batching(&self) -> bool;
}

/// Interface for session-based ordered message processing
#[async_trait]
pub trait SessionClient: Send + Sync {
    /// Receive message from session (maintains order)
    async fn receive_message(
        &self,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    /// Complete message in session
    async fn complete_message(&self, receipt: ReceiptHandle) -> Result<(), QueueError>;

    /// Abandon message in session
    async fn abandon_message(&self, receipt: ReceiptHandle) -> Result<(), QueueError>;

    /// Send message to dead letter queue
    async fn dead_letter_message(
        &self,
        receipt: ReceiptHandle,
        reason: String,
    ) -> Result<(), QueueError>;

    /// Renew session lock to prevent timeout
    async fn renew_session_lock(&self) -> Result<(), QueueError>;

    /// Close session and release lock
    async fn close_session(&self) -> Result<(), QueueError>;

    /// Get session ID
    fn session_id(&self) -> &SessionId;

    /// Get session expiry time
    fn session_expires_at(&self) -> Timestamp;
}

/// Interface implemented by specific queue providers (Azure, AWS, etc.)
#[async_trait]
pub trait QueueProvider: Send + Sync {
    /// Send single message
    async fn send_message(
        &self,
        queue: &QueueName,
        message: &Message,
    ) -> Result<MessageId, QueueError>;

    /// Send multiple messages
    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError>;

    /// Receive single message
    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    /// Receive multiple messages
    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError>;

    /// Complete message processing
    async fn complete_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError>;

    /// Abandon message for retry
    async fn abandon_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError>;

    /// Send to dead letter queue
    async fn dead_letter_message(
        &self,
        receipt: &ReceiptHandle,
        reason: &str,
    ) -> Result<(), QueueError>;

    /// Create session client
    async fn create_session_client(
        &self,
        queue: &QueueName,
        session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError>;

    /// Get provider type
    fn provider_type(&self) -> ProviderType;

    /// Get session support level
    fn supports_sessions(&self) -> SessionSupport;

    /// Check batch operation support
    fn supports_batching(&self) -> bool;

    /// Get maximum batch size
    fn max_batch_size(&self) -> u32;
}

/// Interface implemented by provider-specific session implementations
#[async_trait]
pub trait SessionProvider: Send + Sync {
    /// Receive message from session
    async fn receive_message(
        &self,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError>;

    /// Complete message
    async fn complete_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError>;

    /// Abandon message
    async fn abandon_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError>;

    /// Send to dead letter queue
    async fn dead_letter_message(
        &self,
        receipt: &ReceiptHandle,
        reason: &str,
    ) -> Result<(), QueueError>;

    /// Renew session lock
    async fn renew_session_lock(&self) -> Result<(), QueueError>;

    /// Close session
    async fn close_session(&self) -> Result<(), QueueError>;

    /// Get session ID
    fn session_id(&self) -> &SessionId;

    /// Get session expiry time
    fn session_expires_at(&self) -> Timestamp;
}

// ============================================================================
// Error Types
// ============================================================================

/// Comprehensive error type for all queue operations
#[derive(Debug, Error)]
pub enum QueueError {
    #[error("Queue not found: {queue_name}")]
    QueueNotFound { queue_name: String },

    #[error("Message not found or receipt expired: {receipt}")]
    MessageNotFound { receipt: String },

    #[error("Session '{session_id}' is locked until {locked_until}")]
    SessionLocked {
        session_id: String,
        locked_until: Timestamp,
    },

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

    #[error("Validation error: {0}")]
    ValidationError(#[from] ValidationError),
}

impl QueueError {
    /// Check if error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        match self {
            Self::QueueNotFound { .. } => false,
            Self::MessageNotFound { .. } => false,
            Self::SessionLocked { .. } => true,
            Self::SessionNotFound { .. } => false,
            Self::Timeout { .. } => true,
            Self::ConnectionFailed { .. } => true,
            Self::AuthenticationFailed { .. } => false,
            Self::PermissionDenied { .. } => false,
            Self::MessageTooLarge { .. } => false,
            Self::BatchTooLarge { .. } => false,
            Self::ProviderError { .. } => true, // Provider-specific errors are usually transient
            Self::SerializationError(_) => false,
            Self::ConfigurationError(_) => false,
            Self::ValidationError(_) => false,
        }
    }

    /// Check if error should be retried
    pub fn should_retry(&self) -> bool {
        self.is_transient()
    }

    /// Get suggested retry delay
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::SessionLocked { .. } => Some(Duration::seconds(5)),
            Self::Timeout { .. } => Some(Duration::seconds(1)),
            Self::ConnectionFailed { .. } => Some(Duration::seconds(5)),
            _ => None,
        }
    }
}

/// Errors during message serialization/deserialization
#[derive(Debug, Error)]
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

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("Invalid configuration: {message}")]
    Invalid { message: String },

    #[error("Missing required configuration: {key}")]
    Missing { key: String },

    #[error("Configuration parsing failed: {message}")]
    Parsing { message: String },
}

/// Validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Required field missing: {field}")]
    Required { field: String },

    #[error("Invalid format for {field}: {message}")]
    InvalidFormat { field: String, message: String },

    #[error("Value out of range for {field}: {message}")]
    OutOfRange { field: String, message: String },
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Configuration for queue client initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub provider: ProviderConfig,
    pub default_timeout: Duration,
    pub max_retry_attempts: u32,
    pub retry_base_delay: Duration,
    pub enable_dead_letter: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::InMemory(InMemoryConfig::default()),
            default_timeout: Duration::seconds(30),
            max_retry_attempts: 3,
            retry_base_delay: Duration::seconds(1),
            enable_dead_letter: true,
        }
    }
}

/// Provider-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderConfig {
    AzureServiceBus(AzureServiceBusConfig),
    AwsSqs(AwsSqsConfig),
    InMemory(InMemoryConfig),
}

/// Azure Service Bus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureServiceBusConfig {
    pub connection_string: String,
    pub namespace: String,
    pub use_sessions: bool,
    pub session_timeout: Duration,
}

/// AWS SQS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsSqsConfig {
    pub region: String,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub use_fifo_queues: bool,
}

/// In-memory provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryConfig {
    pub max_queue_size: usize,
    pub enable_persistence: bool,
}

impl Default for InMemoryConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            enable_persistence: false,
        }
    }
}

// ============================================================================
// Client Factory
// ============================================================================

/// Factory for creating queue clients with appropriate providers
pub struct QueueClientFactory;

impl QueueClientFactory {
    /// Create queue client from configuration
    pub async fn create_client(_config: QueueConfig) -> Result<Box<dyn QueueClient>, QueueError> {
        // TODO: Implement client factory
        // See specs/interfaces/queue-client.md
        unimplemented!("Queue client factory not yet implemented")
    }

    /// Create test client with in-memory provider
    pub fn create_test_client() -> Box<dyn QueueClient> {
        // TODO: Implement test client creation
        // See specs/interfaces/queue-client.md
        unimplemented!("Test client creation not yet implemented")
    }
}

// ============================================================================
// Default Implementations (Stubs)
// ============================================================================

/// Standard queue client implementation
pub struct StandardQueueClient;

#[async_trait]
impl QueueClient for StandardQueueClient {
    async fn send_message(
        &self,
        _queue: &QueueName,
        _message: Message,
    ) -> Result<MessageId, QueueError> {
        // TODO: Implement message sending
        // See specs/interfaces/queue-client.md
        unimplemented!("Message sending not yet implemented")
    }

    async fn send_messages(
        &self,
        _queue: &QueueName,
        _messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError> {
        // TODO: Implement batch message sending
        // See specs/interfaces/queue-client.md
        unimplemented!("Batch message sending not yet implemented")
    }

    async fn receive_message(
        &self,
        _queue: &QueueName,
        _timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // TODO: Implement message receiving
        // See specs/interfaces/queue-client.md
        unimplemented!("Message receiving not yet implemented")
    }

    async fn receive_messages(
        &self,
        _queue: &QueueName,
        _max_messages: u32,
        _timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        // TODO: Implement batch message receiving
        // See specs/interfaces/queue-client.md
        unimplemented!("Batch message receiving not yet implemented")
    }

    async fn complete_message(&self, _receipt: ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement message completion
        // See specs/interfaces/queue-client.md
        unimplemented!("Message completion not yet implemented")
    }

    async fn abandon_message(&self, _receipt: ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement message abandonment
        // See specs/interfaces/queue-client.md
        unimplemented!("Message abandonment not yet implemented")
    }

    async fn dead_letter_message(
        &self,
        _receipt: ReceiptHandle,
        _reason: String,
    ) -> Result<(), QueueError> {
        // TODO: Implement dead letter handling
        // See specs/interfaces/queue-client.md
        unimplemented!("Dead letter handling not yet implemented")
    }

    async fn accept_session(
        &self,
        _queue: &QueueName,
        _session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionClient>, QueueError> {
        // TODO: Implement session acceptance
        // See specs/interfaces/queue-client.md
        unimplemented!("Session acceptance not yet implemented")
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::InMemory
    }

    fn supports_sessions(&self) -> bool {
        true
    }

    fn supports_batching(&self) -> bool {
        true
    }
}

/// In-memory provider implementation for testing
pub struct InMemoryProvider;

impl Default for InMemoryProvider {
    fn default() -> Self {
        Self
    }
}

impl InMemoryProvider {
    /// Create new in-memory provider
    pub fn new(_config: InMemoryConfig) -> Self {
        Self
    }
}

#[async_trait]
impl QueueProvider for InMemoryProvider {
    async fn send_message(
        &self,
        _queue: &QueueName,
        _message: &Message,
    ) -> Result<MessageId, QueueError> {
        // TODO: Implement in-memory message sending
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory message sending not yet implemented")
    }

    async fn send_messages(
        &self,
        _queue: &QueueName,
        _messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        // TODO: Implement in-memory batch sending
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory batch sending not yet implemented")
    }

    async fn receive_message(
        &self,
        _queue: &QueueName,
        _timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // TODO: Implement in-memory message receiving
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory message receiving not yet implemented")
    }

    async fn receive_messages(
        &self,
        _queue: &QueueName,
        _max_messages: u32,
        _timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        // TODO: Implement in-memory batch receiving
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory batch receiving not yet implemented")
    }

    async fn complete_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement in-memory message completion
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory message completion not yet implemented")
    }

    async fn abandon_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement in-memory message abandonment
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory message abandonment not yet implemented")
    }

    async fn dead_letter_message(
        &self,
        _receipt: &ReceiptHandle,
        _reason: &str,
    ) -> Result<(), QueueError> {
        // TODO: Implement in-memory dead letter handling
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory dead letter handling not yet implemented")
    }

    async fn create_session_client(
        &self,
        _queue: &QueueName,
        _session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError> {
        // TODO: Implement in-memory session client creation
        // See specs/interfaces/queue-client.md
        unimplemented!("In-memory session client creation not yet implemented")
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::InMemory
    }

    fn supports_sessions(&self) -> SessionSupport {
        SessionSupport::Native
    }

    fn supports_batching(&self) -> bool {
        true
    }

    fn max_batch_size(&self) -> u32 {
        100
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
