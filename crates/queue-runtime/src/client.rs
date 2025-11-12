//! Client traits and implementations for queue operations.

use crate::error::QueueError;
use crate::message::{
    Message, MessageId, QueueName, ReceiptHandle, ReceivedMessage, SessionId, Timestamp,
};
use crate::provider::{ProviderType, QueueConfig, SessionSupport};
use async_trait::async_trait;
use chrono::Duration;

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;

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

/// Standard queue client implementation
pub struct StandardQueueClient;

#[async_trait]
impl QueueClient for StandardQueueClient {
    async fn send_message(
        &self,
        _queue: &QueueName,
        _message: Message,
    ) -> Result<MessageId, QueueError> {
        unimplemented!("Message sending not yet implemented")
    }

    async fn send_messages(
        &self,
        _queue: &QueueName,
        _messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError> {
        unimplemented!("Batch message sending not yet implemented")
    }

    async fn receive_message(
        &self,
        _queue: &QueueName,
        _timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        unimplemented!("Message receiving not yet implemented")
    }

    async fn receive_messages(
        &self,
        _queue: &QueueName,
        _max_messages: u32,
        _timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        unimplemented!("Batch message receiving not yet implemented")
    }

    async fn complete_message(&self, _receipt: ReceiptHandle) -> Result<(), QueueError> {
        unimplemented!("Message completion not yet implemented")
    }

    async fn abandon_message(&self, _receipt: ReceiptHandle) -> Result<(), QueueError> {
        unimplemented!("Message abandonment not yet implemented")
    }

    async fn dead_letter_message(
        &self,
        _receipt: ReceiptHandle,
        _reason: String,
    ) -> Result<(), QueueError> {
        unimplemented!("Dead letter handling not yet implemented")
    }

    async fn accept_session(
        &self,
        _queue: &QueueName,
        _session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionClient>, QueueError> {
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
    pub fn new(_config: crate::provider::InMemoryConfig) -> Self {
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
        unimplemented!("In-memory message sending not yet implemented")
    }

    async fn send_messages(
        &self,
        _queue: &QueueName,
        _messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        unimplemented!("In-memory batch sending not yet implemented")
    }

    async fn receive_message(
        &self,
        _queue: &QueueName,
        _timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        unimplemented!("In-memory message receiving not yet implemented")
    }

    async fn receive_messages(
        &self,
        _queue: &QueueName,
        _max_messages: u32,
        _timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        unimplemented!("In-memory batch receiving not yet implemented")
    }

    async fn complete_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        unimplemented!("In-memory message completion not yet implemented")
    }

    async fn abandon_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        unimplemented!("In-memory message abandonment not yet implemented")
    }

    async fn dead_letter_message(
        &self,
        _receipt: &ReceiptHandle,
        _reason: &str,
    ) -> Result<(), QueueError> {
        unimplemented!("In-memory dead letter handling not yet implemented")
    }

    async fn create_session_client(
        &self,
        _queue: &QueueName,
        _session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError> {
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
