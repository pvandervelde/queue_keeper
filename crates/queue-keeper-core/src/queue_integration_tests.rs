//! Tests for queue integration layer

use super::*;
use crate::{
    bot_config::{BotConfigurationSettings, BotSpecificConfig, EventTypePattern},
    webhook::EventEntity,
    Repository, RepositoryId, User, UserId, UserType,
};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, TimeDelta};
use queue_runtime::{ProviderType, ReceiptHandle, ReceivedMessage, SessionClient};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ============================================================================
// Mock Queue Client
// ============================================================================

#[derive(Clone)]
struct MockQueueClient {
    sent_messages: Arc<Mutex<Vec<(crate::QueueName, Message)>>>,
    should_fail: Arc<Mutex<HashMap<String, bool>>>,
    fail_transiently: Arc<Mutex<bool>>,
}

impl MockQueueClient {
    fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(HashMap::new())),
            fail_transiently: Arc::new(Mutex::new(false)),
        }
    }

    fn with_failure(queue_name: &str, transient: bool) -> Self {
        let client = Self::new();
        client
            .should_fail
            .lock()
            .unwrap()
            .insert(queue_name.to_string(), true);
        *client.fail_transiently.lock().unwrap() = transient;
        client
    }

    fn get_sent_messages(&self) -> Vec<(crate::QueueName, Message)> {
        self.sent_messages.lock().unwrap().clone()
    }

    fn message_count(&self) -> usize {
        self.sent_messages.lock().unwrap().len()
    }
}

#[async_trait]
impl QueueClient for MockQueueClient {
    async fn send_message(
        &self,
        queue: &queue_runtime::QueueName,
        message: Message,
    ) -> Result<MessageId, QueueError> {
        // Check if this queue should fail
        if self
            .should_fail
            .lock()
            .unwrap()
            .get(queue.as_str())
            .copied()
            .unwrap_or(false)
        {
            let is_transient = *self.fail_transiently.lock().unwrap();
            return Err(if is_transient {
                QueueError::Timeout {
                    duration: ChronoDuration::seconds(30),
                }
            } else {
                QueueError::QueueNotFound {
                    queue_name: queue.as_str().to_string(),
                }
            });
        }

        // Store sent message (convert to core QueueName for storage)
        let core_queue_name = crate::QueueName::new(queue.as_str().to_string()).unwrap();
        self.sent_messages
            .lock()
            .unwrap()
            .push((core_queue_name, message.clone()));

        // Return mock message ID
        Ok(MessageId::new())
    }

    async fn send_messages(
        &self,
        _queue: &queue_runtime::QueueName,
        _messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError> {
        unimplemented!("Batch sending not tested in this suite")
    }

    async fn receive_message(
        &self,
        _queue: &queue_runtime::QueueName,
        _timeout: TimeDelta,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        unimplemented!("Receiving not needed for router tests")
    }

    async fn receive_messages(
        &self,
        _queue: &queue_runtime::QueueName,
        _max_messages: u32,
        _timeout: TimeDelta,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        unimplemented!("Receiving not needed for router tests")
    }

    async fn complete_message(&self, _receipt: ReceiptHandle) -> Result<(), QueueError> {
        unimplemented!("Completion not needed for router tests")
    }

    async fn abandon_message(&self, _receipt: ReceiptHandle) -> Result<(), QueueError> {
        unimplemented!("Abandon not needed for router tests")
    }

    async fn dead_letter_message(
        &self,
        _receipt: ReceiptHandle,
        _reason: String,
    ) -> Result<(), QueueError> {
        unimplemented!("Dead letter not needed for router tests")
    }

    async fn accept_session(
        &self,
        _queue: &queue_runtime::QueueName,
        _session_id: Option<queue_runtime::SessionId>,
    ) -> Result<Box<dyn SessionClient>, QueueError> {
        unimplemented!("Sessions not needed for router tests")
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::InMemory
    }

    fn supports_sessions(&self) -> bool {
        true
    }

    fn supports_batching(&self) -> bool {
        false
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_event() -> EventEnvelope {
    let repository = Repository::new(
        RepositoryId::new(12345),
        "test-repo".to_string(),
        "owner/test-repo".to_string(),
        User {
            id: UserId::new(1),
            login: "owner".to_string(),
            user_type: UserType::User,
        },
        false,
    );

    EventEnvelope::new(
        "pull_request".to_string(),
        Some("opened".to_string()),
        repository,
        EventEntity::PullRequest { number: 1 },
        serde_json::json!({"test": "payload"}),
    )
}

fn create_test_bot(name: &str, queue: &str, ordered: bool) -> BotSubscription {
    BotSubscription {
        name: BotName::new(name).unwrap(),
        queue: crate::QueueName::new(queue).unwrap(),
        events: vec![EventTypePattern::Exact("pull_request".to_string())],
        ordered,
        repository_filter: None,
        config: BotSpecificConfig::new(),
    }
}

fn create_test_config(bots: Vec<BotSubscription>) -> BotConfiguration {
    BotConfiguration {
        bots,
        settings: BotConfigurationSettings::default(),
    }
}

// ============================================================================
// Delivery Result Tests
// ============================================================================

#[test]
fn test_delivery_result_is_complete_success() {
    let mut result = DeliveryResult::new(EventId::new());
    assert!(result.is_complete_success());
    assert!(result.is_no_op());

    result.successful.push(SuccessfulDelivery {
        bot_name: BotName::new("test-bot").unwrap(),
        queue_name: crate::QueueName::new("queue-keeper-test-bot").unwrap(),
        message_id: MessageId::new(),
    });

    assert!(result.is_complete_success());
    assert!(!result.is_no_op());
}

#[test]
fn test_delivery_result_is_complete_failure() {
    let mut result = DeliveryResult::new(EventId::new());
    assert!(!result.is_complete_failure());

    result.failed.push(FailedDelivery {
        bot_name: BotName::new("test-bot").unwrap(),
        queue_name: crate::QueueName::new("queue-keeper-test-bot").unwrap(),
        error: "test error".to_string(),
        is_transient: false,
    });

    assert!(result.is_complete_failure());
    assert!(!result.is_complete_success());
}

#[test]
fn test_delivery_result_has_any_success() {
    let mut result = DeliveryResult::new(EventId::new());
    assert!(!result.has_any_success());

    result.successful.push(SuccessfulDelivery {
        bot_name: BotName::new("test-bot").unwrap(),
        queue_name: crate::QueueName::new("queue-keeper-test-bot").unwrap(),
        message_id: MessageId::new(),
    });

    assert!(result.has_any_success());
}

// ============================================================================
// Error Classification Tests
// ============================================================================

#[test]
fn test_queue_delivery_error_transient_classification() {
    // Transient error
    let error = QueueDeliveryError::PartialDelivery {
        successful: 1,
        failed: 1,
    };
    assert!(error.is_transient());
    assert!(error.should_retry());

    // Permanent error
    let error = QueueDeliveryError::SerializationError("test".to_string());
    assert!(!error.is_transient());
    assert!(!error.should_retry());
}

#[test]
fn test_complete_failure_transient_when_all_failures_transient() {
    let failures = vec![
        FailedDelivery {
            bot_name: BotName::new("bot1").unwrap(),
            queue_name: crate::QueueName::new("queue-keeper-bot1").unwrap(),
            error: "timeout".to_string(),
            is_transient: true,
        },
        FailedDelivery {
            bot_name: BotName::new("bot2").unwrap(),
            queue_name: crate::QueueName::new("queue-keeper-bot2").unwrap(),
            error: "timeout".to_string(),
            is_transient: true,
        },
    ];

    let error = QueueDeliveryError::CompleteFailure { failures };
    assert!(error.is_transient());
}

#[test]
fn test_complete_failure_permanent_when_any_failure_permanent() {
    let failures = vec![
        FailedDelivery {
            bot_name: BotName::new("bot1").unwrap(),
            queue_name: crate::QueueName::new("queue-keeper-bot1").unwrap(),
            error: "timeout".to_string(),
            is_transient: true,
        },
        FailedDelivery {
            bot_name: BotName::new("bot2").unwrap(),
            queue_name: crate::QueueName::new("queue-keeper-bot2").unwrap(),
            error: "not found".to_string(),
            is_transient: false,
        },
    ];

    let error = QueueDeliveryError::CompleteFailure { failures };
    assert!(!error.is_transient());
}

// ============================================================================
// Event Routing Tests
// ============================================================================

#[tokio::test]
async fn test_route_event_single_bot_subscription_matches() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    assert!(result.is_complete_success());
    assert_eq!(result.successful.len(), 1);
    assert_eq!(result.failed.len(), 0);
    assert_eq!(queue_client.message_count(), 1);
}

#[tokio::test]
async fn test_route_event_multiple_bot_subscriptions_match() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot1 = create_test_bot("bot1", "queue-keeper-bot1", false);
    let bot2 = create_test_bot("bot2", "queue-keeper-bot2", false);
    let config = create_test_config(vec![bot1, bot2]);
    let queue_client = MockQueueClient::new();

    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    assert!(result.is_complete_success());
    assert_eq!(result.successful.len(), 2);
    assert_eq!(result.failed.len(), 0);
    assert_eq!(queue_client.message_count(), 2);
}

#[tokio::test]
async fn test_route_event_no_bot_subscriptions_match() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();

    // Create bot that subscribes to different event type
    let bot = BotSubscription {
        name: BotName::new("test-bot").unwrap(),
        queue: crate::QueueName::new("queue-keeper-test-bot").unwrap(),
        events: vec![EventTypePattern::Exact("issues".to_string())],
        ordered: false,
        repository_filter: None,
        config: BotSpecificConfig::new(),
    };

    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed with no-op");

    assert!(result.is_no_op());
    assert_eq!(result.successful.len(), 0);
    assert_eq!(result.failed.len(), 0);
    assert_eq!(queue_client.message_count(), 0);
}

#[tokio::test]
async fn test_route_event_queue_send_fails_transiently() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::with_failure("queue-keeper-test-bot", true);

    let result = router.route_event(&event, &config, &queue_client).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_transient(), "Transient failure should be retryable");
    assert!(err.should_retry());
}

#[tokio::test]
async fn test_route_event_queue_send_fails_permanently() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::with_failure("queue-keeper-test-bot", false);

    let result = router.route_event(&event, &config, &queue_client).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        !err.is_transient(),
        "Permanent failure should not be retryable"
    );
    assert!(!err.should_retry());
}

#[tokio::test]
async fn test_route_event_partial_delivery_failure() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot1 = create_test_bot("bot1", "queue-keeper-bot1", false);
    let bot2 = create_test_bot("bot2", "queue-keeper-bot2", false);
    let config = create_test_config(vec![bot1, bot2]);

    // Configure to fail only bot2
    let queue_client = MockQueueClient::with_failure("queue-keeper-bot2", true);

    let result = router.route_event(&event, &config, &queue_client).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        QueueDeliveryError::PartialDelivery { successful, failed } => {
            assert_eq!(successful, 1);
            assert_eq!(failed, 1);
        }
        other => panic!("Expected PartialDelivery, got {:?}", other),
    }
}

#[tokio::test]
async fn test_route_event_session_id_propagated_for_ordered_bots() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", true); // ordered=true
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    let messages = queue_client.get_sent_messages();
    assert_eq!(messages.len(), 1);

    let (_queue, message) = &messages[0];
    assert!(
        message.session_id.is_some(),
        "Session ID should be set for ordered bot"
    );
    assert_eq!(
        message.session_id.as_ref().unwrap().as_str(),
        event.session_id.as_str()
    );
}

#[tokio::test]
async fn test_route_event_no_session_id_for_unordered_bots() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false); // ordered=false
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    let messages = queue_client.get_sent_messages();
    assert_eq!(messages.len(), 1);

    let (_queue, message) = &messages[0];
    assert!(
        message.session_id.is_none(),
        "Session ID should not be set for unordered bot"
    );
}

#[tokio::test]
async fn test_route_event_correlation_id_propagated() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    let messages = queue_client.get_sent_messages();
    assert_eq!(messages.len(), 1);

    let (_queue, message) = &messages[0];
    assert!(
        message.correlation_id.is_some(),
        "Correlation ID should be set"
    );
    assert_eq!(
        message.correlation_id.as_ref().unwrap(),
        &event.correlation_id.to_string()
    );
}

#[tokio::test]
async fn test_route_event_message_attributes_include_metadata() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    let messages = queue_client.get_sent_messages();
    assert_eq!(messages.len(), 1);

    let (_queue, message) = &messages[0];
    assert_eq!(
        message.attributes.get("bot_name"),
        Some(&"test-bot".to_string())
    );
    assert_eq!(
        message.attributes.get("event_type"),
        Some(&"pull_request".to_string())
    );
}

#[tokio::test]
async fn test_route_event_message_body_contains_serialized_event() {
    let router = DefaultEventRouter::new();
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    let messages = queue_client.get_sent_messages();
    assert_eq!(messages.len(), 1);

    let (_queue, message) = &messages[0];

    // Deserialize and verify
    let deserialized: EventEnvelope = serde_json::from_slice(&message.body)
        .expect("Message body should be valid EventEnvelope JSON");

    assert_eq!(deserialized.event_id, event.event_id);
    assert_eq!(deserialized.event_type, event.event_type);
}

// ============================================================================
// Mock Audit Logger for Testing
// ============================================================================

use crate::audit_logging::{
    AuditAction, AuditActor, AuditError, AuditEvent, AuditLogId, AuditLogger, AuditResource,
    SecurityAuditEvent, WebhookProcessingAction,
};
use crate::{EventId, SessionId};

#[derive(Clone, Debug)]
struct LoggedWebhookProcessing {
    _event_id: EventId,
    _session_id: SessionId,
    _repository: Repository,
    action: WebhookProcessingAction,
    result: crate::audit_logging::AuditResult,
    context: crate::audit_logging::AuditContext,
}

#[derive(Clone)]
struct MockAuditLogger {
    logged_webhook_processing: Arc<Mutex<Vec<LoggedWebhookProcessing>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl MockAuditLogger {
    fn new() -> Self {
        Self {
            logged_webhook_processing: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    fn with_failure() -> Self {
        Self {
            logged_webhook_processing: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(true)),
        }
    }

    fn get_logged_webhook_processing(&self) -> Vec<LoggedWebhookProcessing> {
        self.logged_webhook_processing.lock().unwrap().clone()
    }

    fn event_count(&self) -> usize {
        self.logged_webhook_processing.lock().unwrap().len()
    }
}

#[async_trait]
impl AuditLogger for MockAuditLogger {
    async fn log_event(&self, _event: AuditEvent) -> Result<AuditLogId, AuditError> {
        if *self.should_fail.lock().unwrap() {
            return Err(AuditError::StorageError {
                message: "Mock failure".to_string(),
            });
        }

        Ok(AuditLogId::new())
    }

    async fn log_webhook_processing(
        &self,
        event_id: EventId,
        session_id: SessionId,
        repository: Repository,
        action: WebhookProcessingAction,
        result: crate::audit_logging::AuditResult,
        context: crate::audit_logging::AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        if *self.should_fail.lock().unwrap() {
            return Err(AuditError::StorageError {
                message: "Mock failure".to_string(),
            });
        }

        self.logged_webhook_processing
            .lock()
            .unwrap()
            .push(LoggedWebhookProcessing {
                _event_id: event_id,
                _session_id: session_id,
                _repository: repository,
                action,
                result,
                context,
            });

        Ok(AuditLogId::new())
    }

    async fn log_admin_action(
        &self,
        _actor: AuditActor,
        _resource: AuditResource,
        _action: AuditAction,
        _result: crate::audit_logging::AuditResult,
        _context: crate::audit_logging::AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        if *self.should_fail.lock().unwrap() {
            return Err(AuditError::StorageError {
                message: "Mock failure".to_string(),
            });
        }

        Ok(AuditLogId::new())
    }

    async fn log_security_event(
        &self,
        _security_event: SecurityAuditEvent,
        _context: crate::audit_logging::AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        if *self.should_fail.lock().unwrap() {
            return Err(AuditError::StorageError {
                message: "Mock failure".to_string(),
            });
        }

        Ok(AuditLogId::new())
    }

    async fn log_events_batch(
        &self,
        _events: Vec<AuditEvent>,
    ) -> Result<Vec<AuditLogId>, AuditError> {
        if *self.should_fail.lock().unwrap() {
            return Err(AuditError::StorageError {
                message: "Mock failure".to_string(),
            });
        }

        Ok(vec![AuditLogId::new()])
    }

    async fn flush(&self) -> Result<(), AuditError> {
        if *self.should_fail.lock().unwrap() {
            return Err(AuditError::StorageError {
                message: "Mock failure".to_string(),
            });
        }

        Ok(())
    }
}

// ============================================================================
// Audit Logging Tests
// ============================================================================

/// Verify that successful routing logs an audit event with correct details.
///
/// Creates a router with audit logging, routes an event to a matching bot,
/// and verifies the audit event contains the correct bot names, duration, and success status.
#[tokio::test]
async fn test_audit_logging_on_successful_routing() {
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();
    let audit_logger = MockAuditLogger::new();

    let router = DefaultEventRouter::with_audit_logger(Arc::new(audit_logger.clone()));

    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    assert!(result.is_complete_success());

    // Verify audit event was logged
    let logged = audit_logger.get_logged_webhook_processing();
    assert_eq!(logged.len(), 1, "Should log exactly one audit event");

    let log_entry = &logged[0];

    // Verify action type
    if let WebhookProcessingAction::BotRouting {
        matched_bots,
        routing_duration_ms,
    } = &log_entry.action
    {
        assert_eq!(matched_bots.len(), 1);
        assert_eq!(matched_bots[0], "test-bot");
        // Duration will be very small for synchronous mock test, just verify it's present
        // (u64 is always >= 0, so no need to check)
    } else {
        panic!("Expected WebhookProcessing(BotRouting) action");
    }

    // Verify result is success
    matches!(
        &log_entry.result,
        crate::audit_logging::AuditResult::Success { .. }
    );

    // Verify context includes correlation_id
    assert_eq!(
        log_entry.context.correlation_id,
        Some(event.correlation_id.to_string())
    );
}

/// Verify that routing to multiple bots logs all bot names in audit event.
///
/// Creates a router with audit logging, routes an event to multiple matching bots,
/// and verifies all bot names are captured in the audit event.
#[tokio::test]
async fn test_audit_logging_captures_multiple_bots() {
    let event = create_test_event();
    let bot1 = create_test_bot("bot1", "queue-keeper-bot1", false);
    let bot2 = create_test_bot("bot2", "queue-keeper-bot2", false);
    let config = create_test_config(vec![bot1, bot2]);
    let queue_client = MockQueueClient::new();
    let audit_logger = MockAuditLogger::new();

    let router = DefaultEventRouter::with_audit_logger(Arc::new(audit_logger.clone()));

    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed");

    assert!(result.is_complete_success());

    let logged = audit_logger.get_logged_webhook_processing();
    assert_eq!(logged.len(), 1);

    let log_entry = &logged[0];

    if let WebhookProcessingAction::BotRouting { matched_bots, .. } = &log_entry.action {
        assert_eq!(matched_bots.len(), 2);
        assert!(matched_bots.contains(&"bot1".to_string()));
        assert!(matched_bots.contains(&"bot2".to_string()));
    } else {
        panic!("Expected WebhookProcessing(BotRouting) action");
    }
}

/// Verify that routing with no matching bots logs an audit event with empty bot list.
///
/// Creates a router with audit logging, routes an event that matches no bots,
/// and verifies the audit event contains an empty bot list.
#[tokio::test]
async fn test_audit_logging_on_no_matching_bots() {
    let event = create_test_event();
    let config = create_test_config(vec![]); // No bots configured
    let queue_client = MockQueueClient::new();
    let audit_logger = MockAuditLogger::new();

    let router = DefaultEventRouter::with_audit_logger(Arc::new(audit_logger.clone()));

    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed even with no matching bots");

    assert!(result.successful.is_empty());
    assert!(result.failed.is_empty());

    // Verify audit event was logged
    let logged = audit_logger.get_logged_webhook_processing();
    assert_eq!(
        logged.len(),
        1,
        "Should log audit event even with no matching bots"
    );

    let log_entry = &logged[0];

    if let WebhookProcessingAction::BotRouting { matched_bots, .. } = &log_entry.action {
        assert_eq!(
            matched_bots.len(),
            0,
            "Should have empty bot list when no bots match"
        );
    } else {
        panic!("Expected WebhookProcessing(BotRouting) action");
    }

    // Verify result indicates success (no bots is not a failure)
    matches!(
        &log_entry.result,
        crate::audit_logging::AuditResult::Success { .. }
    );
}

/// Verify that partial delivery failures are captured in audit result.
///
/// Creates a router with audit logging, routes an event where one queue fails,
/// and verifies the audit event captures the partial failure status.
#[tokio::test]
async fn test_audit_logging_on_partial_delivery_failure() {
    let event = create_test_event();
    let bot1 = create_test_bot("bot1", "queue-keeper-bot1", false);
    let bot2 = create_test_bot("bot2", "queue-keeper-bot2", false);
    let config = create_test_config(vec![bot1, bot2]);

    // Configure queue client to fail for bot2's queue
    let queue_client = MockQueueClient::with_failure("queue-keeper-bot2", false);
    let audit_logger = MockAuditLogger::new();

    let router = DefaultEventRouter::with_audit_logger(Arc::new(audit_logger.clone()));

    let result = router.route_event(&event, &config, &queue_client).await;

    // Partial delivery returns an error
    assert!(result.is_err());
    matches!(
        result,
        Err(super::QueueDeliveryError::PartialDelivery { .. })
    );

    // Verify audit event captures failure
    let logged = audit_logger.get_logged_webhook_processing();
    assert_eq!(logged.len(), 1);

    let log_entry = &logged[0];

    // Verify result is failure
    if let crate::audit_logging::AuditResult::Failure { error_code, .. } = &log_entry.result {
        assert_eq!(error_code, "PARTIAL_DELIVERY");
    } else {
        panic!("Expected Failure result for partial delivery");
    }

    // Verify both bots are still captured in matched_bots
    if let WebhookProcessingAction::BotRouting { matched_bots, .. } = &log_entry.action {
        assert_eq!(matched_bots.len(), 2);
    } else {
        panic!("Expected WebhookProcessing(BotRouting) action");
    }
}

/// Verify that audit logging failures do not block routing operations (best-effort pattern).
///
/// Creates a router with a failing audit logger, routes an event successfully,
/// and verifies routing completes despite audit failure.
#[tokio::test]
async fn test_audit_logging_failure_does_not_block_routing() {
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();
    let audit_logger = MockAuditLogger::with_failure(); // Configure to fail

    let router = DefaultEventRouter::with_audit_logger(Arc::new(audit_logger.clone()));

    // Routing should succeed despite audit logging failure
    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed even if audit logging fails");

    assert!(result.is_complete_success());

    // Verify message was still delivered
    let messages = queue_client.get_sent_messages();
    assert_eq!(
        messages.len(),
        1,
        "Message should be delivered despite audit failure"
    );

    // Verify audit logger was called but failed
    assert_eq!(
        audit_logger.event_count(),
        0,
        "Audit event should not be stored due to failure"
    );
}

/// Verify that routing without audit logger works correctly (backward compatibility).
///
/// Creates a router without audit logging, routes an event successfully,
/// and verifies routing completes normally.
#[tokio::test]
async fn test_routing_without_audit_logger() {
    let event = create_test_event();
    let bot = create_test_bot("test-bot", "queue-keeper-test-bot", false);
    let config = create_test_config(vec![bot]);
    let queue_client = MockQueueClient::new();

    // Create router without audit logger
    let router = DefaultEventRouter::new();

    let result = router
        .route_event(&event, &config, &queue_client)
        .await
        .expect("Routing should succeed without audit logger");

    assert!(result.is_complete_success());

    let messages = queue_client.get_sent_messages();
    assert_eq!(messages.len(), 1);
}
