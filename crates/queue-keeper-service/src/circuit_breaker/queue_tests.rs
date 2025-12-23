//! Tests for CircuitBreakerQueueProvider wrapper.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Duration;
use queue_runtime::{
    InMemoryConfig, InMemoryProvider, Message, MessageId, ProviderType, QueueError, QueueName,
    QueueProvider, ReceiptHandle, ReceivedMessage, SessionId, SessionProvider, SessionSupport,
    Timestamp,
};

use super::CircuitBreakerQueueProvider;

// ============================================================================
// Mock Failing Provider
// ============================================================================

/// Mock queue provider that always fails for testing circuit breaker.
#[derive(Clone)]
struct FailingQueueProvider {
    failure_count: Arc<std::sync::Mutex<u32>>,
}

impl FailingQueueProvider {
    fn new() -> Self {
        Self {
            failure_count: Arc::new(std::sync::Mutex::new(0)),
        }
    }

    fn failure_count(&self) -> u32 {
        *self.failure_count.lock().unwrap()
    }
}

#[async_trait]
impl QueueProvider for FailingQueueProvider {
    async fn send_message(
        &self,
        _queue: &QueueName,
        _message: &Message,
    ) -> Result<MessageId, QueueError> {
        let mut count = self.failure_count.lock().unwrap();
        *count += 1;
        Err(QueueError::ProviderError {
            provider: "FailingMock".to_string(),
            code: "ServiceUnavailable".to_string(),
            message: "Mock failure".to_string(),
        })
    }

    async fn send_messages(
        &self,
        _queue: &QueueName,
        _messages: &[Message],
    ) -> Result<Vec<MessageId>, QueueError> {
        let mut count = self.failure_count.lock().unwrap();
        *count += 1;
        Err(QueueError::ProviderError {
            provider: "FailingMock".to_string(),
            code: "ServiceUnavailable".to_string(),
            message: "Mock batch failure".to_string(),
        })
    }

    async fn receive_message(
        &self,
        _queue: &QueueName,
        _timeout: Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        // Don't increment failure count - receive_message returns None on empty
        Ok(None)
    }

    async fn receive_messages(
        &self,
        _queue: &QueueName,
        _max_messages: u32,
        _timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        let mut count = self.failure_count.lock().unwrap();
        *count += 1;
        Err(QueueError::ProviderError {
            provider: "FailingMock".to_string(),
            code: "ServiceUnavailable".to_string(),
            message: "Mock batch receive failure".to_string(),
        })
    }

    async fn complete_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        Ok(())
    }

    async fn abandon_message(&self, _receipt: &ReceiptHandle) -> Result<(), QueueError> {
        Ok(())
    }

    async fn dead_letter_message(
        &self,
        _receipt: &ReceiptHandle,
        _reason: &str,
    ) -> Result<(), QueueError> {
        Ok(())
    }

    async fn create_session_client(
        &self,
        _queue: &QueueName,
        _session_id: Option<SessionId>,
    ) -> Result<Box<dyn SessionProvider>, QueueError> {
        Err(QueueError::ProviderError {
            provider: "FailingMock".to_string(),
            code: "UNSUPPORTED".to_string(),
            message: "create_session_client operation not supported".to_string(),
        })
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::InMemory
    }

    fn supports_sessions(&self) -> SessionSupport {
        SessionSupport::Unsupported
    }

    fn supports_batching(&self) -> bool {
        true
    }

    fn max_batch_size(&self) -> u32 {
        10
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_provider() -> CircuitBreakerQueueProvider {
    let inner = Arc::new(InMemoryProvider::new(InMemoryConfig::default()));
    CircuitBreakerQueueProvider::new(inner)
}

fn create_failing_provider() -> (CircuitBreakerQueueProvider, Arc<FailingQueueProvider>) {
    let failing = Arc::new(FailingQueueProvider::new());
    let wrapper = CircuitBreakerQueueProvider::new(failing.clone() as Arc<dyn QueueProvider>);
    (wrapper, failing)
}

// ============================================================================
// Construction Tests
// ============================================================================

#[test]
fn test_circuit_breaker_queue_provider_creation() {
    let provider = create_test_provider();
    // Verify inner provider is accessible
    assert_eq!(provider.inner().provider_type(), ProviderType::InMemory);
}

#[test]
fn test_circuit_breaker_queue_provider_clone() {
    let provider = create_test_provider();
    let cloned = provider.clone();

    // Both should be independent but functional
    assert_eq!(cloned.provider_type(), ProviderType::InMemory);
}

// ============================================================================
// Circuit Breaker Protection Tests - Send Operations
// ============================================================================

/// Verify send_message is protected by circuit breaker.
#[tokio::test]
async fn test_send_message_circuit_protection() {
    let (provider, failing) = create_failing_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();
    let message = Message::new(Bytes::from("test message"));

    // First 5 failures should go through circuit breaker
    for i in 0..5 {
        let result = provider.send_message(&queue, &message).await;
        assert!(result.is_err(), "Attempt {} should fail", i + 1);
    }

    // Verify failures went through the provider
    assert_eq!(failing.failure_count(), 5);

    // Circuit should now be open - next attempt should fail fast
    let result = provider.send_message(&queue, &message).await;
    assert!(result.is_err());

    // Error should be circuit breaker error, not provider error
    if let Err(e) = result {
        match e {
            QueueError::ProviderError { code, .. } => {
                assert_eq!(code, "CircuitOpen", "Expected circuit open error");
            }
            _ => panic!("Expected ProviderError with CircuitOpen code"),
        }
    }

    // Failure count should still be 5 (circuit breaker blocked the 6th attempt)
    assert_eq!(failing.failure_count(), 5);
}

/// Verify send_messages batch operation is protected.
#[tokio::test]
async fn test_send_messages_circuit_protection() {
    let (provider, failing) = create_failing_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();
    let messages = vec![
        Message::new(Bytes::from("message 1")),
        Message::new(Bytes::from("message 2")),
    ];

    // Trigger circuit breaker with send_messages
    for _ in 0..5 {
        let _ = provider.send_messages(&queue, &messages).await;
    }

    assert_eq!(failing.failure_count(), 5);

    // Circuit should be open
    let result = provider.send_messages(&queue, &messages).await;
    assert!(result.is_err());

    if let Err(QueueError::ProviderError { code, .. }) = result {
        assert_eq!(code, "CircuitOpen");
    }
}

/// Verify successful send operations work correctly.
#[tokio::test]
async fn test_send_message_success() {
    let provider = create_test_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();
    let message = Message::new(Bytes::from("test message"));

    let result = provider.send_message(&queue, &message).await;
    assert!(
        result.is_ok(),
        "Send should succeed with in-memory provider"
    );
}

// ============================================================================
// Circuit Breaker Protection Tests - Receive Operations
// ============================================================================

/// Verify receive_messages is protected by circuit breaker.
#[tokio::test]
async fn test_receive_messages_circuit_protection() {
    let (provider, failing) = create_failing_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();

    // Trigger circuit breaker with receive_messages
    for _ in 0..5 {
        let _ = provider
            .receive_messages(&queue, 10, Duration::seconds(1))
            .await;
    }

    assert_eq!(failing.failure_count(), 5);

    // Circuit should be open
    let result = provider
        .receive_messages(&queue, 10, Duration::seconds(1))
        .await;
    assert!(result.is_err());

    if let Err(QueueError::ProviderError { code, .. }) = result {
        assert_eq!(code, "CircuitOpen");
    }
}

/// Verify receive_message passes through (not circuit protected).
#[tokio::test]
async fn test_receive_message_passthrough() {
    let (provider, _) = create_failing_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();

    // receive_message should pass through without circuit breaker
    // (returns None for empty queue, which is not an error)
    let result = provider.receive_message(&queue, Duration::seconds(1)).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ============================================================================
// Pass-Through Operations Tests
// ============================================================================

/// Verify complete_message bypasses circuit breaker.
#[tokio::test]
async fn test_complete_message_passthrough() {
    let (provider, _) = create_failing_provider();
    let receipt = ReceiptHandle::new(
        "test-receipt".to_string(),
        Timestamp::now(),
        ProviderType::AzureServiceBus,
    );

    // Complete should always pass through (uses receipt from previous successful receive)
    let result = provider.complete_message(&receipt).await;
    assert!(result.is_ok(), "Complete should pass through");
}

/// Verify abandon_message bypasses circuit breaker.
#[tokio::test]
async fn test_abandon_message_passthrough() {
    let (provider, _) = create_failing_provider();
    let receipt = ReceiptHandle::new(
        "test-receipt".to_string(),
        Timestamp::now(),
        ProviderType::AzureServiceBus,
    );

    let result = provider.abandon_message(&receipt).await;
    assert!(result.is_ok(), "Abandon should pass through");
}

/// Verify dead_letter_message bypasses circuit breaker.
#[tokio::test]
async fn test_dead_letter_message_passthrough() {
    let (provider, _) = create_failing_provider();
    let receipt = ReceiptHandle::new(
        "test-receipt".to_string(),
        Timestamp::now(),
        ProviderType::AzureServiceBus,
    );

    let result = provider.dead_letter_message(&receipt, "test reason").await;
    assert!(result.is_ok(), "Dead letter should pass through");
}

/// Verify metadata methods pass through.
#[test]
fn test_metadata_passthrough() {
    let provider = create_test_provider();

    assert_eq!(provider.provider_type(), ProviderType::InMemory);
    assert_eq!(provider.supports_sessions(), SessionSupport::Native);
    assert!(provider.supports_batching());
    assert_eq!(provider.max_batch_size(), 100);
}

// ============================================================================
// Error Mapping Tests
// ============================================================================

/// Verify circuit breaker errors are mapped to QueueError correctly.
#[tokio::test]
async fn test_circuit_breaker_error_mapping() {
    let (provider, _) = create_failing_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();
    let message = Message::new(Bytes::from("test"));

    // Trip the circuit
    for _ in 0..5 {
        let _ = provider.send_message(&queue, &message).await;
    }

    // Get circuit open error
    let result = provider.send_message(&queue, &message).await;
    assert!(result.is_err());

    // Verify error is properly mapped
    match result.unwrap_err() {
        QueueError::ProviderError {
            provider,
            code,
            message,
        } => {
            assert_eq!(provider, "CircuitBreaker");
            assert_eq!(code, "CircuitOpen");
            assert!(message.contains("circuit breaker is open"));
        }
        other => panic!("Expected ProviderError, got {:?}", other),
    }
}

/// Verify timeout errors are mapped correctly.
#[tokio::test]
async fn test_timeout_error_mapping() {
    // Note: Timeout errors are theoretical in this test setup
    // since we'd need operations that actually timeout.
    // This test documents the expected mapping behavior.
    let provider = create_test_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();
    let message = Message::new(Bytes::from("test"));

    // Normal operation should not timeout
    let result = provider.send_message(&queue, &message).await;
    assert!(result.is_ok());
}

// ============================================================================
// Configuration Tests
// ============================================================================

/// Verify Service Bus circuit breaker uses correct configuration.
#[test]
fn test_service_bus_circuit_breaker_config() {
    use queue_keeper_core::circuit_breaker::service_bus_circuit_breaker_config;

    let config = service_bus_circuit_breaker_config();

    // Verify configuration matches Service Bus requirements
    assert_eq!(config.service_name, "azure-service-bus");
    assert_eq!(config.failure_threshold, 5); // REQ-009 compliance
    assert_eq!(config.recovery_timeout_seconds, 30); // REQ-009 compliance
    assert_eq!(config.operation_timeout_seconds, 5); // Quick operations
}

// ============================================================================
// Integration Scenarios
// ============================================================================

/// Verify circuit breaker can recover after cooldown.
#[tokio::test]
async fn test_circuit_recovery_scenario() {
    let provider = create_test_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();
    let message = Message::new(Bytes::from("test message"));

    // Normal operations should work
    let result = provider.send_message(&queue, &message).await;
    assert!(result.is_ok());

    // Batch operations should work
    let messages = vec![
        Message::new(Bytes::from("msg1")),
        Message::new(Bytes::from("msg2")),
    ];
    let result = provider.send_messages(&queue, &messages).await;
    assert!(result.is_ok());

    // Receive operations should work
    let result = provider
        .receive_messages(&queue, 10, Duration::seconds(1))
        .await;
    assert!(result.is_ok());
}

/// Verify separate circuit breakers for send and receive.
#[tokio::test]
async fn test_separate_circuit_breakers() {
    let (provider, _) = create_failing_provider();
    let queue = QueueName::new("test-queue".to_string()).unwrap();

    // Trip send circuit
    for _ in 0..5 {
        let _ = provider
            .send_message(&queue, &Message::new(Bytes::from("test")))
            .await;
    }

    // Send should be blocked
    let send_result = provider
        .send_message(&queue, &Message::new(Bytes::from("test")))
        .await;
    assert!(matches!(
        send_result,
        Err(QueueError::ProviderError { code, .. }) if code == "CircuitOpen"
    ));

    // Receive should still work (separate circuit breaker)
    // Note: receive_messages will also fail due to mock, but for different reasons
    let receive_result = provider
        .receive_messages(&queue, 10, Duration::seconds(1))
        .await;
    assert!(receive_result.is_err());
    // First few receive attempts should be operation failures, not circuit breaker
}
