//! Circuit breaker wrapper for queue operations.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Duration;
use queue_keeper_core::circuit_breaker::{
    service_bus_circuit_breaker_config, CircuitBreaker, CircuitBreakerError, CircuitBreakerFactory,
    DefaultCircuitBreaker, DefaultCircuitBreakerFactory,
};
use queue_runtime::{
    Message, MessageId, ProviderType, QueueClient, QueueError, QueueName, QueueProvider,
    ReceiptHandle, ReceivedMessage, SessionClient, SessionId, SessionProvider, SessionSupport,
};

/// Queue provider with circuit breaker protection.
///
/// Wraps queue_runtime::QueueProvider with circuit breaker protection to prevent
/// cascading failures when queue service experiences issues.
#[derive(Clone)]
pub struct CircuitBreakerQueueProvider {
    /// Underlying queue provider
    inner: Arc<dyn QueueProvider>,
    /// Circuit breaker for protecting queue operations
    circuit_breaker_send: DefaultCircuitBreaker<Vec<MessageId>, QueueError>,
    circuit_breaker_receive: DefaultCircuitBreaker<Vec<ReceivedMessage>, QueueError>,
}

impl CircuitBreakerQueueProvider {
    /// Create new circuit breaker protected queue provider.
    ///
    /// # Arguments
    /// - `inner`: Underlying QueueProvider to protect
    pub fn new(inner: Arc<dyn QueueProvider>) -> Self {
        let factory = DefaultCircuitBreakerFactory;
        let circuit_breaker_config = service_bus_circuit_breaker_config();

        // Use separate circuit breakers for send and receive operations
        // as they may have different failure modes
        let circuit_breaker_send =
            factory.create_typed_circuit_breaker(circuit_breaker_config.clone());
        let circuit_breaker_receive = factory.create_typed_circuit_breaker(circuit_breaker_config);

        Self {
            inner,
            circuit_breaker_send,
            circuit_breaker_receive,
        }
    }
}

#[async_trait]
impl QueueProvider for CircuitBreakerQueueProvider {
    async fn send_message(
        &self,
        queue: &QueueName,
        message: &Message,
    ) -> Result<MessageId, QueueError> {
        let inner = Arc::clone(&self.inner);
        let queue = queue.clone();
        let message = message.clone();

        self.circuit_breaker_send
            .call(|| async move {
                let message_id = inner.send_message(&queue, &message).await?;
                Ok(vec![message_id])
            })
            .await
            // INVARIANT: the closure always returns Ok(vec![one_id]), so the
            // vec is guaranteed to have exactly one element; unwrap cannot panic.
            .map(|ids| ids.into_iter().next().unwrap())
            .map_err(map_send_cb_error)
    }

    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        let inner = Arc::clone(&self.inner);
        let queue = queue.clone();
        let messages = messages.to_vec();

        self.circuit_breaker_send
            .call(|| async move { inner.send_messages(&queue, &messages).await })
            .await
            .map_err(map_send_cb_error)
    }

    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // Don't circuit break receive_message since None is a valid result
        // Circuit breaking is better suited for send/receive_messages which always expect results
        self.inner.receive_message(queue, timeout).await
    }

    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        let inner = Arc::clone(&self.inner);
        let queue = queue.clone();

        self.circuit_breaker_receive
            .call(|| async move { inner.receive_messages(&queue, max_messages, timeout).await })
            .await
            .map_err(|e| match e {
                CircuitBreakerError::CircuitOpen => QueueError::ProviderError {
                    provider: "CircuitBreaker".to_string(),
                    code: "CircuitOpen".to_string(),
                    message: "Queue receive circuit breaker is open".to_string(),
                },
                CircuitBreakerError::Timeout { timeout_ms } => QueueError::ProviderError {
                    provider: "CircuitBreaker".to_string(),
                    code: "Timeout".to_string(),
                    message: format!(
                        "Queue batch receive operation timed out after {}ms",
                        timeout_ms
                    ),
                },
                CircuitBreakerError::OperationFailed(e) => e,
                CircuitBreakerError::TooManyConcurrentRequests => QueueError::ProviderError {
                    provider: "CircuitBreaker".to_string(),
                    code: "TooManyConcurrentRequests".to_string(),
                    message: "Too many concurrent queue receive requests".to_string(),
                },
                CircuitBreakerError::InternalError { message } => QueueError::ProviderError {
                    provider: "CircuitBreaker".to_string(),
                    code: "InternalError".to_string(),
                    message,
                },
            })
    }

    // Pass through operations that don't need circuit breaker protection
    // (these are typically lower-risk or use receipts from successful operations)

    async fn complete_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        self.inner.complete_message(receipt).await
    }

    async fn abandon_message(&self, receipt: &ReceiptHandle) -> Result<(), QueueError> {
        self.inner.abandon_message(receipt).await
    }

    async fn dead_letter_message(
        &self,
        receipt: &ReceiptHandle,
        reason: &str,
    ) -> Result<(), QueueError> {
        self.inner.dead_letter_message(receipt, reason).await
    }

    async fn create_session_client(
        &self,
        queue: &QueueName,
        session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError> {
        self.inner.create_session_client(queue, session_id).await
    }

    fn provider_type(&self) -> ProviderType {
        self.inner.provider_type()
    }

    fn supports_sessions(&self) -> SessionSupport {
        self.inner.supports_sessions()
    }

    fn supports_batching(&self) -> bool {
        self.inner.supports_batching()
    }

    fn max_batch_size(&self) -> u32 {
        self.inner.max_batch_size()
    }
}

/// Queue client with circuit breaker protection.
///
/// Wraps a [`QueueClient`] (as returned by [`QueueClientFactory::create_client`]) with
/// circuit breaker protection to prevent cascading failures when the queue service
/// experiences issues.
///
/// Use this when only a `QueueClient` reference is available, such as for AWS SQS which
/// is accessed via `QueueClientFactory` rather than a bare provider.
///
/// # Circuit Breaker Behaviour
///
/// - Send operations (`send_message`, `send_messages`) share a single circuit breaker.
/// - Receive-batch operations (`receive_messages`) have their own circuit breaker.
/// - Pass-through operations that consume an already-established receipt
///   (`complete_message`, `abandon_message`, `dead_letter_message`) are never circuit-broken
///   because they are lower-risk follow-ups to a successfully received message.
#[derive(Clone)]
pub struct CircuitBreakerQueueClient {
    /// Underlying queue client
    inner: Arc<dyn QueueClient>,
    /// Circuit breaker for protecting send operations
    circuit_breaker_send: DefaultCircuitBreaker<Vec<MessageId>, QueueError>,
    /// Circuit breaker for protecting batch receive operations
    circuit_breaker_receive: DefaultCircuitBreaker<Vec<ReceivedMessage>, QueueError>,
}

impl CircuitBreakerQueueClient {
    /// Create a new circuit-breaker-protected queue client.
    ///
    /// # Arguments
    /// - `inner`: Underlying [`QueueClient`] implementation to protect.
    pub fn new(inner: Arc<dyn QueueClient>) -> Self {
        let factory = DefaultCircuitBreakerFactory;
        let circuit_breaker_config = service_bus_circuit_breaker_config();

        // Separate circuit breakers for send and receive — different failure modes.
        let circuit_breaker_send =
            factory.create_typed_circuit_breaker(circuit_breaker_config.clone());
        let circuit_breaker_receive = factory.create_typed_circuit_breaker(circuit_breaker_config);

        Self {
            inner,
            circuit_breaker_send,
            circuit_breaker_receive,
        }
    }
}

/// Map a `CircuitBreakerError<QueueError>` to a `QueueError` for send operations.
fn map_send_cb_error(e: CircuitBreakerError<QueueError>) -> QueueError {
    match e {
        CircuitBreakerError::CircuitOpen => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "CircuitOpen".to_string(),
            message: "Queue send circuit breaker is open".to_string(),
        },
        CircuitBreakerError::Timeout { timeout_ms } => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "Timeout".to_string(),
            message: format!("Queue send operation timed out after {}ms", timeout_ms),
        },
        CircuitBreakerError::OperationFailed(e) => e,
        CircuitBreakerError::TooManyConcurrentRequests => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "TooManyConcurrentRequests".to_string(),
            message: "Too many concurrent queue send requests".to_string(),
        },
        CircuitBreakerError::InternalError { message } => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "InternalError".to_string(),
            message,
        },
    }
}

/// Map a `CircuitBreakerError<QueueError>` to a `QueueError` for receive operations.
fn map_receive_cb_error(e: CircuitBreakerError<QueueError>) -> QueueError {
    match e {
        CircuitBreakerError::CircuitOpen => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "CircuitOpen".to_string(),
            message: "Queue receive circuit breaker is open".to_string(),
        },
        CircuitBreakerError::Timeout { timeout_ms } => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "Timeout".to_string(),
            message: format!("Queue receive operation timed out after {}ms", timeout_ms),
        },
        CircuitBreakerError::OperationFailed(e) => e,
        CircuitBreakerError::TooManyConcurrentRequests => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "TooManyConcurrentRequests".to_string(),
            message: "Too many concurrent queue receive requests".to_string(),
        },
        CircuitBreakerError::InternalError { message } => QueueError::ProviderError {
            provider: "CircuitBreaker".to_string(),
            code: "InternalError".to_string(),
            message,
        },
    }
}

#[async_trait]
impl QueueClient for CircuitBreakerQueueClient {
    async fn send_message(
        &self,
        queue: &QueueName,
        message: Message,
    ) -> Result<MessageId, QueueError> {
        let inner = Arc::clone(&self.inner);
        let queue = queue.clone();

        self.circuit_breaker_send
            .call(|| async move {
                let message_id = inner.send_message(&queue, message).await?;
                Ok(vec![message_id])
            })
            .await
            // INVARIANT: the closure always returns Ok(vec![one_id]), so the
            // vec is guaranteed to have exactly one element; unwrap cannot panic.
            .map(|ids| ids.into_iter().next().unwrap())
            .map_err(map_send_cb_error)
    }

    async fn send_messages(
        &self,
        queue: &QueueName,
        messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError> {
        let inner = Arc::clone(&self.inner);
        let queue = queue.clone();

        self.circuit_breaker_send
            .call(|| async move { inner.send_messages(&queue, messages).await })
            .await
            .map_err(map_send_cb_error)
    }

    async fn receive_message(
        &self,
        queue: &QueueName,
        timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // Don't circuit break receive_message — None is a valid empty-queue result.
        self.inner.receive_message(queue, timeout).await
    }

    async fn receive_messages(
        &self,
        queue: &QueueName,
        max_messages: u32,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        let inner = Arc::clone(&self.inner);
        let queue = queue.clone();

        self.circuit_breaker_receive
            .call(|| async move { inner.receive_messages(&queue, max_messages, timeout).await })
            .await
            .map_err(map_receive_cb_error)
    }

    // Pass-through: follow-up operations on already-received receipts.

    async fn complete_message(&self, receipt: ReceiptHandle) -> Result<(), QueueError> {
        self.inner.complete_message(receipt).await
    }

    async fn abandon_message(&self, receipt: ReceiptHandle) -> Result<(), QueueError> {
        self.inner.abandon_message(receipt).await
    }

    async fn dead_letter_message(
        &self,
        receipt: ReceiptHandle,
        reason: String,
    ) -> Result<(), QueueError> {
        self.inner.dead_letter_message(receipt, reason).await
    }

    async fn accept_session(
        &self,
        queue: &QueueName,
        session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionClient>, QueueError> {
        self.inner.accept_session(queue, session_id).await
    }

    fn provider_type(&self) -> ProviderType {
        self.inner.provider_type()
    }

    fn supports_sessions(&self) -> bool {
        self.inner.supports_sessions()
    }

    fn supports_batching(&self) -> bool {
        self.inner.supports_batching()
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
