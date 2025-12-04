//! Tests for async queue delivery with retry loop
//!
//! These tests verify the queue delivery behavior including:
//! - Successful delivery to all queues
//! - Retry logic for transient failures
//! - Handling of permanent failures
//! - Partial delivery scenarios
//! - Background task spawning

use super::*;
use async_trait::async_trait;
use queue_keeper_core::{
    bot_config::{BotConfigurationSettings, BotSpecificConfig, BotSubscription, EventTypePattern},
    queue_integration::{FailedDelivery, QueueDeliveryError, SuccessfulDelivery},
    webhook::{EventEntity, EventEnvelope},
    BotName, EventId, QueueName as CoreQueueName, Repository, RepositoryId, User, UserId, UserType,
};
use queue_runtime::{Message, MessageId, QueueError, QueueName, ReceivedMessage};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

// Re-import chrono Duration for queue client trait methods
use chrono::Duration as ChronoDuration;

// ============================================================================
// Mock Types
// ============================================================================

/// Mock event router that can be configured to return specific results
struct MockEventRouter {
    /// Results to return on each call (in order)
    results: Mutex<Vec<Result<DeliveryResult, QueueDeliveryError>>>,
    /// Track number of calls
    call_count: AtomicU32,
}

impl MockEventRouter {
    fn new(results: Vec<Result<DeliveryResult, QueueDeliveryError>>) -> Self {
        Self {
            results: Mutex::new(results),
            call_count: AtomicU32::new(0),
        }
    }

    fn with_success(event_id: EventId, count: usize) -> Self {
        let result = Ok(DeliveryResult {
            event_id: event_id.clone(),
            successful: (0..count)
                .map(|i| SuccessfulDelivery {
                    bot_name: BotName::new(format!("bot-{}", i)).unwrap(),
                    queue_name: CoreQueueName::new(format!("queue-keeper-bot-{}", i)).unwrap(),
                    message_id: MessageId::new(),
                })
                .collect(),
            failed: vec![],
        });
        Self::new(vec![result])
    }

    fn with_no_targets(event_id: EventId) -> Self {
        let result = Ok(DeliveryResult {
            event_id,
            successful: vec![],
            failed: vec![],
        });
        Self::new(vec![result])
    }

    fn with_transient_failure_then_success(event_id: EventId) -> Self {
        // First call fails with transient error
        let failure = Ok(DeliveryResult {
            event_id: event_id.clone(),
            successful: vec![],
            failed: vec![FailedDelivery {
                bot_name: BotName::new("bot-1".to_string()).unwrap(),
                queue_name: CoreQueueName::new("queue-keeper-bot-1".to_string()).unwrap(),
                error: "Transient connection error".to_string(),
                is_transient: true,
            }],
        });

        // Second call succeeds
        let success = Ok(DeliveryResult {
            event_id: event_id.clone(),
            successful: vec![SuccessfulDelivery {
                bot_name: BotName::new("bot-1".to_string()).unwrap(),
                queue_name: CoreQueueName::new("queue-keeper-bot-1".to_string()).unwrap(),
                message_id: MessageId::new(),
            }],
            failed: vec![],
        });

        Self::new(vec![failure, success])
    }

    fn with_permanent_failure(event_id: EventId) -> Self {
        let result = Ok(DeliveryResult {
            event_id,
            successful: vec![],
            failed: vec![FailedDelivery {
                bot_name: BotName::new("bot-1".to_string()).unwrap(),
                queue_name: CoreQueueName::new("queue-keeper-bot-1".to_string()).unwrap(),
                error: "Invalid queue name".to_string(),
                is_transient: false,
            }],
        });
        Self::new(vec![result])
    }

    fn with_partial_success(event_id: EventId) -> Self {
        let result = Ok(DeliveryResult {
            event_id,
            successful: vec![SuccessfulDelivery {
                bot_name: BotName::new("bot-1".to_string()).unwrap(),
                queue_name: CoreQueueName::new("queue-keeper-bot-1".to_string()).unwrap(),
                message_id: MessageId::new(),
            }],
            failed: vec![FailedDelivery {
                bot_name: BotName::new("bot-2".to_string()).unwrap(),
                queue_name: CoreQueueName::new("queue-keeper-bot-2".to_string()).unwrap(),
                error: "Permanent failure".to_string(),
                is_transient: false,
            }],
        });
        Self::new(vec![result])
    }

    fn get_call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl EventRouter for MockEventRouter {
    async fn route_event(
        &self,
        _event: &EventEnvelope,
        _config: &BotConfiguration,
        _queue_client: &dyn QueueClient,
    ) -> Result<DeliveryResult, QueueDeliveryError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        let mut results = self.results.lock().unwrap();
        if results.is_empty() {
            // Return success if no more results configured
            Ok(DeliveryResult {
                event_id: EventId::new(),
                successful: vec![],
                failed: vec![],
            })
        } else {
            results.remove(0)
        }
    }
}

/// Mock queue client for testing
struct MockQueueClient;

#[async_trait]
impl QueueClient for MockQueueClient {
    async fn send_message(
        &self,
        _queue: &QueueName,
        _message: Message,
    ) -> Result<MessageId, QueueError> {
        Ok(MessageId::new())
    }

    async fn send_messages(
        &self,
        _queue: &QueueName,
        _messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError> {
        Ok(vec![MessageId::new()])
    }

    async fn receive_message(
        &self,
        _queue: &QueueName,
        _timeout: ChronoDuration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        Ok(None)
    }

    async fn receive_messages(
        &self,
        _queue: &QueueName,
        _max_messages: u32,
        _timeout: ChronoDuration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        Ok(vec![])
    }

    async fn complete_message(
        &self,
        _receipt: queue_runtime::ReceiptHandle,
    ) -> Result<(), QueueError> {
        Ok(())
    }

    async fn abandon_message(
        &self,
        _receipt: queue_runtime::ReceiptHandle,
    ) -> Result<(), QueueError> {
        Ok(())
    }

    async fn dead_letter_message(
        &self,
        _receipt: queue_runtime::ReceiptHandle,
        _reason: String,
    ) -> Result<(), QueueError> {
        Ok(())
    }

    async fn accept_session(
        &self,
        _queue: &QueueName,
        _session_id: Option<queue_runtime::SessionId>,
    ) -> Result<Box<dyn queue_runtime::SessionClient>, QueueError> {
        Err(QueueError::SessionNotFound {
            session_id: "mock".to_string(),
        })
    }

    fn provider_type(&self) -> queue_runtime::ProviderType {
        queue_runtime::ProviderType::InMemory
    }

    fn supports_sessions(&self) -> bool {
        false
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
        "test-org/test-repo".to_string(),
        User {
            id: UserId::new(1),
            login: "test-org".to_string(),
            user_type: UserType::Organization,
        },
        false,
    );

    let entity = EventEntity::PullRequest { number: 123 };

    EventEnvelope::new(
        "pull_request".to_string(),
        Some("opened".to_string()),
        repository,
        entity,
        serde_json::json!({"action": "opened"}),
    )
}

fn create_test_bot_config() -> BotConfiguration {
    BotConfiguration {
        bots: vec![BotSubscription {
            name: BotName::new("test-bot".to_string()).unwrap(),
            queue: CoreQueueName::new("queue-keeper-test-bot".to_string()).unwrap(),
            events: vec![EventTypePattern::EntityAll("pull_request".to_string())],
            ordered: true,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        }],
        settings: BotConfigurationSettings::default(),
    }
}

fn create_fast_retry_config() -> QueueDeliveryConfig {
    QueueDeliveryConfig {
        retry_policy: RetryPolicy::new(
            3,                         // max attempts
            Duration::from_millis(10), // very short for tests
            Duration::from_millis(100),
            2.0,
        )
        .without_jitter(), // disable jitter for deterministic tests
        enable_dlq: true,
        dlq_service: None, // No DLQ storage for fast tests
    }
}

// ============================================================================
// Tests
// ============================================================================

/// Verify successful delivery returns AllQueuesSucceeded outcome
#[tokio::test]
async fn test_successful_delivery_to_all_queues() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let event_router = Arc::new(MockEventRouter::with_success(event_id.clone(), 2));
    let delivery_config = create_fast_retry_config();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;

    // Assert
    assert!(outcome.is_success());
    assert!(!outcome.has_failures());

    match outcome {
        QueueDeliveryOutcome::AllQueuesSucceeded {
            event_id: returned_id,
            successful_count,
        } => {
            assert_eq!(returned_id, event_id);
            assert_eq!(successful_count, 2);
        }
        _ => panic!("Expected AllQueuesSucceeded outcome"),
    }

    // Should only make one call since first attempt succeeded
    assert_eq!(event_router.get_call_count(), 1);
}

/// Verify no target queues returns NoTargetQueues outcome
#[tokio::test]
async fn test_no_target_queues_returns_no_op() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let event_router = Arc::new(MockEventRouter::with_no_targets(event_id.clone()));
    let delivery_config = create_fast_retry_config();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;

    // Assert
    assert!(outcome.is_success());
    assert!(!outcome.has_failures());

    match outcome {
        QueueDeliveryOutcome::NoTargetQueues {
            event_id: returned_id,
        } => {
            assert_eq!(returned_id, event_id);
        }
        _ => panic!("Expected NoTargetQueues outcome"),
    }
}

/// Verify transient failures are retried with backoff
#[tokio::test]
async fn test_retries_transient_failures_with_backoff() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let event_router = Arc::new(MockEventRouter::with_transient_failure_then_success(
        event_id.clone(),
    ));
    let delivery_config = create_fast_retry_config();

    // Act
    let start = std::time::Instant::now();
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;
    let elapsed = start.elapsed();

    // Assert
    assert!(outcome.is_success());

    match outcome {
        QueueDeliveryOutcome::AllQueuesSucceeded {
            successful_count, ..
        } => {
            assert_eq!(successful_count, 1);
        }
        _ => panic!("Expected AllQueuesSucceeded after retry"),
    }

    // Should have made 2 calls (initial + 1 retry)
    assert_eq!(event_router.get_call_count(), 2);

    // Should have waited for at least the retry delay
    assert!(elapsed >= Duration::from_millis(10));
}

/// Verify permanent failures are not retried
#[tokio::test]
async fn test_permanent_failures_not_retried() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let event_router = Arc::new(MockEventRouter::with_permanent_failure(event_id.clone()));
    let delivery_config = create_fast_retry_config();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;

    // Assert
    assert!(outcome.has_failures());

    match outcome {
        QueueDeliveryOutcome::CompleteFailure { error, .. } => {
            assert!(error.contains("failed"));
        }
        _ => panic!("Expected CompleteFailure outcome"),
    }

    // Should only make one call since failure is permanent
    assert_eq!(event_router.get_call_count(), 1);
}

/// Verify partial success returns SomeQueuesFailed outcome
#[tokio::test]
async fn test_partial_success_returns_partial_failure_outcome() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let event_router = Arc::new(MockEventRouter::with_partial_success(event_id.clone()));
    let delivery_config = create_fast_retry_config();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;

    // Assert
    assert!(outcome.has_failures());

    match outcome {
        QueueDeliveryOutcome::SomeQueuesFailed {
            successful_count,
            failed_count,
            ..
        } => {
            assert_eq!(successful_count, 1);
            assert_eq!(failed_count, 1);
        }
        _ => panic!("Expected SomeQueuesFailed outcome"),
    }
}

/// Verify max retries are respected
#[tokio::test]
async fn test_max_retries_respected() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();

    // Create router that always returns transient failure
    let always_transient_failure = (0..10)
        .map(|_| {
            Ok(DeliveryResult {
                event_id: event_id.clone(),
                successful: vec![],
                failed: vec![FailedDelivery {
                    bot_name: BotName::new("bot-1".to_string()).unwrap(),
                    queue_name: CoreQueueName::new("queue-keeper-bot-1".to_string()).unwrap(),
                    error: "Transient error".to_string(),
                    is_transient: true,
                }],
            })
        })
        .collect();

    let event_router = Arc::new(MockEventRouter::new(always_transient_failure));
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);

    let delivery_config = QueueDeliveryConfig {
        retry_policy: RetryPolicy::new(
            3, // max 3 retry attempts
            Duration::from_millis(1),
            Duration::from_millis(10),
            2.0,
        )
        .without_jitter(),
        enable_dlq: true,
        dlq_service: None,
    };

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;

    // Assert
    assert!(outcome.has_failures());

    // Initial attempt + 3 retries = 4 total calls
    assert_eq!(event_router.get_call_count(), 4);
}

/// Verify spawn_queue_delivery creates a background task
#[tokio::test]
async fn test_spawn_queue_delivery_creates_background_task() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let event_router = Arc::new(MockEventRouter::with_success(event_id.clone(), 1));
    let delivery_config = create_fast_retry_config();

    // Act
    let handle = spawn_queue_delivery(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    );

    // Wait for task to complete
    let outcome = handle.await.expect("Task should complete successfully");

    // Assert
    assert!(outcome.is_success());
    assert_eq!(event_router.get_call_count(), 1);
}

/// Verify QueueDeliveryOutcome helper methods work correctly
#[test]
fn test_queue_delivery_outcome_helpers() {
    let event_id = EventId::new();

    // AllQueuesSucceeded
    let success = QueueDeliveryOutcome::AllQueuesSucceeded {
        event_id: event_id.clone(),
        successful_count: 2,
    };
    assert!(success.is_success());
    assert!(!success.has_failures());

    // NoTargetQueues
    let no_targets = QueueDeliveryOutcome::NoTargetQueues {
        event_id: event_id.clone(),
    };
    assert!(no_targets.is_success());
    assert!(!no_targets.has_failures());

    // SomeQueuesFailed
    let partial = QueueDeliveryOutcome::SomeQueuesFailed {
        event_id: event_id.clone(),
        successful_count: 1,
        failed_count: 1,
        persisted_to_dlq: false,
    };
    assert!(!partial.is_success());
    assert!(partial.has_failures());

    // CompleteFailure
    let failure = QueueDeliveryOutcome::CompleteFailure {
        event_id: event_id.clone(),
        error: "Test error".to_string(),
        persisted_to_dlq: false,
    };
    assert!(!failure.is_success());
    assert!(failure.has_failures());
}

/// Verify default configuration uses appropriate values
#[test]
fn test_default_queue_delivery_config() {
    let config = QueueDeliveryConfig::default();

    assert!(config.enable_dlq);
    assert_eq!(config.retry_policy.max_attempts, 5);
    assert_eq!(config.retry_policy.initial_delay, Duration::from_secs(1));
    assert_eq!(config.retry_policy.max_delay, Duration::from_secs(16));
    assert!((config.retry_policy.backoff_multiplier - 2.0).abs() < f64::EPSILON);
}

/// Verify routing error with transient classification triggers retry
#[tokio::test]
async fn test_routing_error_transient_triggers_retry() {
    // Arrange
    let event = create_test_event();
    let event_id = event.event_id.clone();

    // First call returns transient routing error, second succeeds
    let results = vec![
        Err(QueueDeliveryError::QueueClientError(QueueError::Timeout {
            duration: ChronoDuration::seconds(5),
        })),
        Ok(DeliveryResult {
            event_id: event_id.clone(),
            successful: vec![SuccessfulDelivery {
                bot_name: BotName::new("bot-1".to_string()).unwrap(),
                queue_name: CoreQueueName::new("queue-keeper-bot-1".to_string()).unwrap(),
                message_id: MessageId::new(),
            }],
            failed: vec![],
        }),
    ];

    let event_router = Arc::new(MockEventRouter::new(results));
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let delivery_config = create_fast_retry_config();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;

    // Assert
    assert!(outcome.is_success());
    assert_eq!(event_router.get_call_count(), 2);
}

/// Verify routing error with permanent classification does not retry
#[tokio::test]
async fn test_routing_error_permanent_no_retry() {
    // Arrange
    let event = create_test_event();

    // Return permanent routing error
    let results = vec![Err(QueueDeliveryError::SerializationError(
        "Invalid JSON".to_string(),
    ))];

    let event_router = Arc::new(MockEventRouter::new(results));
    let bot_config = Arc::new(create_test_bot_config());
    let queue_client = Arc::new(MockQueueClient);
    let delivery_config = create_fast_retry_config();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router.clone(),
        bot_config,
        queue_client,
        delivery_config,
    )
    .await;

    // Assert
    assert!(outcome.has_failures());
    assert_eq!(event_router.get_call_count(), 1); // No retry for permanent error

    match outcome {
        QueueDeliveryOutcome::CompleteFailure { error, .. } => {
            assert!(error.contains("Invalid JSON"));
        }
        _ => panic!("Expected CompleteFailure outcome"),
    }
}
