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
