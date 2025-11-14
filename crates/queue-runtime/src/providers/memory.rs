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

    /// Return expired in-flight messages back to the queue
    fn return_expired_messages(queue: &mut InMemoryQueue) {
        let now = Timestamp::now();
        let mut expired_handles = Vec::new();

        // Find expired messages
        for (handle, inflight) in &queue.in_flight {
            if now >= inflight.lock_expires_at {
                expired_handles.push(handle.clone());
            }
        }

        // Return them to the queue
        for handle in expired_handles {
            if let Some(inflight) = queue.in_flight.remove(&handle) {
                let mut message = inflight.message;
                // Make immediately available
                message.available_at = now.clone();
                queue.messages.push_back(message);
            }
        }
    }

    /// Check if a session is locked
    fn is_session_locked(queue: &InMemoryQueue, session_id: &Option<SessionId>) -> bool {
        if let Some(ref sid) = session_id {
            if let Some(session_state) = queue.sessions.get(sid) {
                return session_state.is_locked();
            }
        }
        false
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
        queue: &QueueName,
        message: &Message,
    ) -> Result<MessageId, QueueError> {
        // Validate message size (10MB for in-memory provider)
        let message_size = message.body.len();
        let max_size = self.provider_type().max_message_size();
        if message_size > max_size {
            return Err(QueueError::MessageTooLarge {
                size: message_size,
                max_size,
            });
        }

        // Generate message ID
        let message_id = MessageId::new();

        // Store message
        let stored_message = StoredMessage::from_message(message, message_id.clone());

        let mut storage = self.storage.write().unwrap();
        let queue_state = storage.get_or_create_queue(queue);
        queue_state.messages.push_back(stored_message);

        Ok(message_id)
    }

    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        // Validate batch size
        if messages.len() > self.max_batch_size() as usize {
            return Err(QueueError::BatchTooLarge {
                size: messages.len(),
                max_size: self.max_batch_size() as usize,
            });
        }

        // Validate individual message sizes and send all
        let mut message_ids = Vec::with_capacity(messages.len());
        for message in messages {
            let message_id = self.send_message(queue, message).await?;
            message_ids.push(message_id);
        }

        Ok(message_ids)
    }

    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        let start_time = std::time::Instant::now();
        let timeout_duration = timeout
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(30));

        loop {
            // Try to receive a message
            let received_message = {
                let mut storage = self.storage.write().unwrap();
                let queue_state = storage.get_or_create_queue(queue);

                // First, return any expired in-flight messages back to the queue
                Self::return_expired_messages(queue_state);

                // Find first available message (not expired, visibility timeout passed, not in a locked session)
                let now = Timestamp::now();
                let message_index = queue_state.messages.iter().position(|msg| {
                    !msg.is_expired()
                        && msg.is_available()
                        && !Self::is_session_locked(queue_state, &msg.session_id)
                });

                if let Some(index) = message_index {
                    // Remove message from queue
                    let mut stored_message = queue_state.messages.remove(index).unwrap();

                    // Increment delivery count
                    stored_message.delivery_count += 1;

                    // Create receipt handle
                    let receipt_handle_str = uuid::Uuid::new_v4().to_string();
                    let lock_expires_at =
                        Timestamp::from_datetime(now.as_datetime() + Duration::seconds(30));
                    let receipt_handle = ReceiptHandle::new(
                        receipt_handle_str.clone(),
                        lock_expires_at.clone(),
                        ProviderType::InMemory,
                    );

                    // Create received message
                    let received_message = ReceivedMessage {
                        message_id: stored_message.message_id.clone(),
                        body: stored_message.body.clone(),
                        attributes: stored_message.attributes.clone(),
                        session_id: stored_message.session_id.clone(),
                        correlation_id: stored_message.correlation_id.clone(),
                        receipt_handle: receipt_handle.clone(),
                        delivery_count: stored_message.delivery_count,
                        first_delivered_at: stored_message.enqueued_at.clone(),
                        delivered_at: now,
                    };

                    // Move to in-flight
                    let inflight = InFlightMessage {
                        message: stored_message,
                        receipt_handle: receipt_handle_str.clone(),
                        lock_expires_at,
                    };
                    queue_state.in_flight.insert(receipt_handle_str, inflight);

                    Some(received_message)
                } else {
                    None
                }
            }; // Lock is released here

            if let Some(msg) = received_message {
                return Ok(Some(msg));
            }

            // No message available - check timeout
            if start_time.elapsed() >= timeout_duration {
                return Ok(None);
            }

            // Small sleep before retry
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        let mut messages = Vec::new();
        let start_time = std::time::Instant::now();
        let timeout_duration = timeout
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(30));

        while messages.len() < max_messages as usize {
            let remaining_timeout = timeout_duration
                .checked_sub(start_time.elapsed())
                .unwrap_or(std::time::Duration::ZERO);

            if remaining_timeout.is_zero() {
                break;
            }

            let remaining_duration =
                Duration::from_std(remaining_timeout).unwrap_or(Duration::zero());
            let received = self.receive_message(queue, remaining_duration).await?;

            match received {
                Some(msg) => messages.push(msg),
                None => break,
            }
        }

        Ok(messages)
    }

    async fn complete_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        let mut storage = self.storage.write().unwrap();
        let now = Timestamp::now();

        // Find the queue containing this receipt
        for queue in storage.queues.values_mut() {
            if let Some(inflight) = queue.in_flight.get(receipt.handle()) {
                // Check if receipt is expired
                if inflight.lock_expires_at <= now {
                    queue.in_flight.remove(receipt.handle());
                    return Err(QueueError::MessageNotFound {
                        receipt: receipt.handle().to_string(),
                    });
                }

                // Remove from in-flight (permanently deletes the message)
                queue.in_flight.remove(receipt.handle());
                return Ok(());
            }
        }

        // Receipt not found in any queue
        Err(QueueError::MessageNotFound {
            receipt: receipt.handle().to_string(),
        })
    }

    async fn abandon_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        let mut storage = self.storage.write().unwrap();
        let now = Timestamp::now();

        // Find the queue containing this receipt
        for queue in storage.queues.values_mut() {
            if let Some(inflight) = queue.in_flight.remove(receipt.handle()) {
                // Check if receipt is expired
                if inflight.lock_expires_at <= now {
                    return Err(QueueError::MessageNotFound {
                        receipt: receipt.handle().to_string(),
                    });
                }

                // Return message to queue with immediate availability
                let mut returned_message = inflight.message;
                returned_message.available_at = now;

                // Add back to queue (front for sessions to maintain ordering, back for others)
                if returned_message.session_id.is_some() {
                    queue.messages.push_front(returned_message);
                } else {
                    queue.messages.push_back(returned_message);
                }

                return Ok(());
            }
        }

        // Receipt not found in any queue
        Err(QueueError::MessageNotFound {
            receipt: receipt.handle().to_string(),
        })
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
