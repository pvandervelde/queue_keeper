//! In-memory queue provider implementation for testing and development.
//!
//! This module provides a fully functional in-memory queue implementation that:
//! - Supports session-based ordered message processing
//! - Implements visibility timeouts and message TTL
//! - Simulates dead letter queue behavior
//! - Provides thread-safe concurrent access
//!
//! This provider is intended for:
//! - Unit testing of queue-runtime consumers
//! - Development and prototyping
//! - Reference implementation for cloud providers

use crate::client::{QueueProvider, SessionProvider};
use crate::error::QueueError;
use crate::message::{
    Message, MessageId, QueueName, ReceiptHandle, ReceivedMessage, SessionId, Timestamp,
};
use crate::provider::{InMemoryConfig, ProviderType, SessionSupport};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Duration;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;

// ============================================================================
// Internal Storage Structures
// ============================================================================

/// Thread-safe storage for all queues
struct QueueStorage {
    queues: HashMap<QueueName, InMemoryQueue>,
    config: InMemoryConfig,
}

impl QueueStorage {
    fn new(config: InMemoryConfig) -> Self {
        Self {
            queues: HashMap::new(),
            config,
        }
    }

    /// Get or create a queue
    fn get_or_create_queue(&mut self, queue_name: &QueueName) -> &mut InMemoryQueue {
        self.queues
            .entry(queue_name.clone())
            .or_insert_with(|| InMemoryQueue::new(self.config.clone()))
    }
}

/// Internal queue state for a single queue
struct InMemoryQueue {
    /// Main message queue (FIFO order)
    messages: VecDeque<StoredMessage>,
    /// Dead letter queue for failed messages
    dead_letter: VecDeque<StoredMessage>,
    /// In-flight messages being processed
    in_flight: HashMap<String, InFlightMessage>,
    /// Session state tracking
    sessions: HashMap<SessionId, SessionState>,
    /// Configuration
    config: InMemoryConfig,
}

impl InMemoryQueue {
    fn new(config: InMemoryConfig) -> Self {
        Self {
            messages: VecDeque::new(),
            dead_letter: VecDeque::new(),
            in_flight: HashMap::new(),
            sessions: HashMap::new(),
            config,
        }
    }
}

/// A message stored in the queue with metadata
#[derive(Clone)]
struct StoredMessage {
    message_id: MessageId,
    body: Bytes,
    attributes: HashMap<String, String>,
    session_id: Option<SessionId>,
    correlation_id: Option<String>,
    enqueued_at: Timestamp,
    delivery_count: u32,
    available_at: Timestamp,
    expires_at: Option<Timestamp>,
}

impl StoredMessage {
    fn from_message(message: &Message, message_id: MessageId) -> Self {
        let now = Timestamp::now();
        let expires_at = message
            .time_to_live
            .map(|ttl| Timestamp::from_datetime(now.as_datetime() + ttl));

        Self {
            message_id,
            body: message.body.clone(),
            attributes: message.attributes.clone(),
            session_id: message.session_id.clone(),
            correlation_id: message.correlation_id.clone(),
            enqueued_at: now.clone(),
            delivery_count: 0,
            available_at: now,
            expires_at,
        }
    }

    /// Check if message is expired based on TTL
    fn is_expired(&self) -> bool {
        if let Some(ref expires_at) = self.expires_at {
            Timestamp::now() >= *expires_at
        } else {
            false
        }
    }

    /// Check if message is available for receiving
    fn is_available(&self) -> bool {
        Timestamp::now() >= self.available_at
    }
}

/// A message currently being processed
struct InFlightMessage {
    message: StoredMessage,
    receipt_handle: String,
    lock_expires_at: Timestamp,
}

impl InFlightMessage {
    fn is_expired(&self) -> bool {
        Timestamp::now() >= self.lock_expires_at
    }
}

/// State tracking for a message session
struct SessionState {
    locked: bool,
    lock_expires_at: Option<Timestamp>,
    locked_by: Option<String>, // Session client ID
}

impl SessionState {
    fn new() -> Self {
        Self {
            locked: false,
            lock_expires_at: None,
            locked_by: None,
        }
    }

    fn is_locked(&self) -> bool {
        if !self.locked {
            return false;
        }

        // Check if lock has expired
        if let Some(ref expires_at) = self.lock_expires_at {
            if Timestamp::now() >= *expires_at {
                return false;
            }
        }

        true
    }
}

// ============================================================================
// InMemoryProvider
// ============================================================================

/// In-memory queue provider implementation
pub struct InMemoryProvider {
    storage: Arc<RwLock<QueueStorage>>,
}

impl InMemoryProvider {
    /// Create new in-memory provider with configuration
    pub fn new(config: InMemoryConfig) -> Self {
        Self {
            storage: Arc::new(RwLock::new(QueueStorage::new(config))),
        }
    }
}

impl Default for InMemoryProvider {
    fn default() -> Self {
        Self::new(InMemoryConfig::default())
    }
}

#[async_trait]
impl QueueProvider for InMemoryProvider {
    async fn send_message(
        &self,
        _queue: &QueueName,
        _message: &Message,
    ) -> Result<MessageId, QueueError> {
        // TODO: Implement in subtask 10.2
        unimplemented!("send_message will be implemented in subtask 10.2")
    }

    async fn send_messages(
        &self,
        _queue: &QueueName,
        _messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        // TODO: Implement in subtask 10.2
        unimplemented!("send_messages will be implemented in subtask 10.2")
    }

    async fn receive_message(
        &self,
        _queue: &QueueName,
        _timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // TODO: Implement in subtask 10.2
        unimplemented!("receive_message will be implemented in subtask 10.2")
    }

    async fn receive_messages(
        &self,
        _queue: &QueueName,
        _max_messages: u32,
        _timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        // TODO: Implement in subtask 10.2
        unimplemented!("receive_messages will be implemented in subtask 10.2")
    }

    async fn complete_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.3
        unimplemented!("complete_message will be implemented in subtask 10.3")
    }

    async fn abandon_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.3
        unimplemented!("abandon_message will be implemented in subtask 10.3")
    }

    async fn dead_letter_message(
        &self,
        _receipt: &ReceiptHandle,
        _reason: &str,
    ) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.4
        unimplemented!("dead_letter_message will be implemented in subtask 10.4")
    }

    async fn create_session_client(
        &self,
        _queue: &QueueName,
        _session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError> {
        // TODO: Implement in subtask 10.5
        unimplemented!("create_session_client will be implemented in subtask 10.5")
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

// ============================================================================
// InMemorySessionProvider
// ============================================================================

/// In-memory session provider implementation
pub struct InMemorySessionProvider {
    storage: Arc<RwLock<QueueStorage>>,
    queue_name: QueueName,
    session_id: SessionId,
    client_id: String,
}

impl InMemorySessionProvider {
    fn new(
        storage: Arc<RwLock<QueueStorage>>,
        queue_name: QueueName,
        session_id: SessionId,
    ) -> Self {
        let client_id = uuid::Uuid::new_v4().to_string();
        Self {
            storage,
            queue_name,
            session_id,
            client_id,
        }
    }
}

#[async_trait]
impl SessionProvider for InMemorySessionProvider {
    async fn receive_message(
        &self,
        _timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // TODO: Implement in subtask 10.5
        unimplemented!("SessionProvider::receive_message will be implemented in subtask 10.5")
    }

    async fn complete_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.5
        unimplemented!("SessionProvider::complete_message will be implemented in subtask 10.5")
    }

    async fn abandon_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.5
        unimplemented!("SessionProvider::abandon_message will be implemented in subtask 10.5")
    }

    async fn dead_letter_message(
        &self,
        _receipt: &ReceiptHandle,
        _reason: &str,
    ) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.5
        unimplemented!("SessionProvider::dead_letter_message will be implemented in subtask 10.5")
    }

    async fn renew_session_lock(&self) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.5
        unimplemented!("renew_session_lock will be implemented in subtask 10.5")
    }

    async fn close_session(&self) -> Result<(), QueueError> {
        // TODO: Implement in subtask 10.5
        unimplemented!("close_session will be implemented in subtask 10.5")
    }

    fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    fn session_expires_at(&self) -> Timestamp {
        // TODO: Implement proper expiration tracking in subtask 10.5
        Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::minutes(5))
    }
}
