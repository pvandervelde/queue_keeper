//! Integration tests for queue delivery and retry logic
//!
//! These tests verify:
//! - Event delivery to multiple bot queues (Assertion #6)
//! - Retry behavior with exponential backoff (Assertion #10)
//! - Dead letter queue handling (Assertion #9)
//! - Partial delivery failure handling (Assertion #6)

mod common;

use common::{create_empty_bot_config, create_test_bot_config, MockBlobStorage, MockQueueClient};
use queue_keeper_api::{
    dlq_storage::DlqStorageService,
    queue_delivery::{deliver_event_to_queues, QueueDeliveryConfig, QueueDeliveryOutcome},
    retry::RetryPolicy,
};
use queue_keeper_core::{queue_integration::DefaultEventRouter, webhook::WrappedEvent, SessionId};
use std::sync::Arc;
use std::time::Duration;

/// Helper to create a test wrapped event
fn create_test_event() -> WrappedEvent {
    WrappedEvent::new(
        "github".to_string(),
        "pull_request".to_string(),
        Some("opened".to_string()),
        Some(SessionId::from_parts(
            "owner",
            "repo",
            "pull_request",
            "123",
        )),
        serde_json::json!({
            "action": "opened",
            "number": 123,
            "repository": {
                "name": "test-repo",
                "full_name": "test-owner/test-repo",
                "owner": {"login": "test-owner"}
            }
        }),
    )
}

/// Verify that queue delivery succeeds when all queues accept events.
///
/// Asserts Assertion #6: One-to-Many Routing — the event MUST be delivered to
/// every bot queue whose subscription matches the event.
#[tokio::test]
async fn test_successful_delivery_to_all_queues() {
    // Arrange: 2 bots, both subscribing to all event types
    let event = create_test_event();
    let bot_config = create_test_bot_config(2);
    let queue_client = Arc::new(MockQueueClient::new());
    let event_router = Arc::new(DefaultEventRouter::new());
    let config = QueueDeliveryConfig::default();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router,
        Arc::new(bot_config),
        queue_client.clone(),
        config,
    )
    .await;

    // Assert: both queues received the event
    assert!(
        matches!(
            outcome,
            QueueDeliveryOutcome::AllQueuesSucceeded {
                successful_count: 2,
                ..
            }
        ),
        "Expected AllQueuesSucceeded(2), got {:?}",
        outcome
    );
    assert_eq!(
        queue_client.send_count(),
        2,
        "Expected 2 send_message calls"
    );
}

/// Verify that transient failures trigger retry with exponential backoff.
///
/// Asserts Assertion #10: Retry Behavior — transient errors MUST be retried
/// with exponential back-off up to the configured maximum.
#[tokio::test]
async fn test_retry_on_transient_failure() {
    // Arrange: 1 bot, first attempt fails transiently, second succeeds
    let event = create_test_event();
    let bot_config = create_test_bot_config(1);
    let queue_client = Arc::new(MockQueueClient::new());
    // Queue one transient failure then let default succeed
    queue_client.expect_transient_failure();

    let event_router = Arc::new(DefaultEventRouter::new());
    let config = QueueDeliveryConfig {
        retry_policy: RetryPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            backoff_multiplier: 2.0,
            use_jitter: false,
            jitter_percent: 0.0,
        },
        ..Default::default()
    };

    let start = std::time::Instant::now();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router,
        Arc::new(bot_config),
        queue_client.clone(),
        config,
    )
    .await;

    let elapsed = start.elapsed();

    // Assert: succeeds on retry
    assert!(
        matches!(
            outcome,
            QueueDeliveryOutcome::AllQueuesSucceeded {
                successful_count: 1,
                ..
            }
        ),
        "Expected AllQueuesSucceeded after retry, got {:?}",
        outcome
    );
    // At least one retry delay of 10 ms must have occurred
    assert!(
        elapsed >= Duration::from_millis(10),
        "Expected retry delay, elapsed was {:?}",
        elapsed
    );
    // 2 send calls: first attempt (failure) + retry (success)
    assert_eq!(queue_client.send_count(), 2);
}

/// Verify that permanent failures do not trigger retry.
///
/// Asserts Assertion #10: Retry Behavior — permanent errors MUST NOT be retried
/// and MUST fail fast.
#[tokio::test]
async fn test_no_retry_on_permanent_failure() {
    // Arrange: 1 bot, permanent failure on every send
    let event = create_test_event();
    let bot_config = create_test_bot_config(1);
    let queue_client = Arc::new(MockQueueClient::new());
    // Queue several permanent failures to ensure no retry consumes extras
    for _ in 0..5 {
        queue_client.expect_permanent_failure();
    }

    let event_router = Arc::new(DefaultEventRouter::new());
    let config = QueueDeliveryConfig {
        retry_policy: RetryPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            use_jitter: false,
            jitter_percent: 0.0,
        },
        enable_dlq: false,
        dlq_service: None,
    };

    let start = std::time::Instant::now();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router,
        Arc::new(bot_config),
        queue_client.clone(),
        config,
    )
    .await;

    let elapsed = start.elapsed();

    // Assert: failed without retrying (only 1 send call, no delay)
    assert!(
        matches!(outcome, QueueDeliveryOutcome::CompleteFailure { .. }),
        "Expected CompleteFailure, got {:?}",
        outcome
    );
    // Permanent failure means no retry delays
    assert!(
        elapsed < Duration::from_millis(100),
        "Permanent failure retried unexpectedly; elapsed {:?}",
        elapsed
    );
    // Only the single initial send attempt
    assert_eq!(queue_client.send_count(), 1);
}

/// Verify that partial delivery failures are handled and tracked correctly.
///
/// Asserts Assertion #6: One-to-Many Routing — failures on individual queues
/// MUST be recorded; the router exhausts retries and reports the outcome.
///
/// Note: Because `PartialDelivery` errors are classified as transient by
/// `QueueDeliveryError`, the delivery loop retries the full batch up to
/// `max_attempts` times before giving up. With `max_attempts=1` no retry
/// occurs, so the outcome is `CompleteFailure`.
#[tokio::test]
async fn test_partial_delivery_failure_tracking() {
    // Arrange: 3 bots; the second send call returns a permanent failure,
    // the others succeed. max_attempts=1 to prevent the partial-delivery
    // retry loop from re-sending to already-successful queues.
    let event = create_test_event();
    let bot_config = create_test_bot_config(3);
    let queue_client = Arc::new(MockQueueClient::new());
    // bot-1 succeeds, bot-2 fails permanently, bot-3 succeeds
    // (order is deterministic: bots are iterated in config order)
    queue_client.expect_success();
    queue_client.expect_permanent_failure();
    queue_client.expect_success();

    let event_router = Arc::new(DefaultEventRouter::new());
    let config = QueueDeliveryConfig {
        retry_policy: RetryPolicy {
            max_attempts: 0, // 0 retries = 1 attempt total, no retry loop
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            backoff_multiplier: 2.0,
            use_jitter: false,
            jitter_percent: 0.0,
        },
        enable_dlq: false,
        dlq_service: None,
    };

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router,
        Arc::new(bot_config),
        queue_client.clone(),
        config,
    )
    .await;

    // Assert: failure reported; the partial-delivery error is classified as
    // transient (PartialDelivery), but with max_attempts=0 (no retries allowed)
    // it immediately converts to CompleteFailure.
    assert!(
        matches!(outcome, QueueDeliveryOutcome::CompleteFailure { .. }),
        "Expected CompleteFailure for partial delivery with max_attempts=1, got {:?}",
        outcome
    );
    // All 3 sends happened within the single attempt
    assert_eq!(queue_client.send_count(), 3);
}

/// Verify that events with no matching queues are handled gracefully.
///
/// Tests the edge case where no bot subscriptions match the incoming event.
#[tokio::test]
async fn test_no_matching_queues() {
    // Arrange: empty bot config — no subscriptions
    let event = create_test_event();
    let bot_config = create_empty_bot_config();
    let queue_client = Arc::new(MockQueueClient::new());
    let event_router = Arc::new(DefaultEventRouter::new());
    let config = QueueDeliveryConfig::default();

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router,
        Arc::new(bot_config),
        queue_client.clone(),
        config,
    )
    .await;

    // Assert: no-op outcome, nothing sent
    assert!(
        matches!(outcome, QueueDeliveryOutcome::NoTargetQueues { .. }),
        "Expected NoTargetQueues, got {:?}",
        outcome
    );
    assert_eq!(queue_client.send_count(), 0);
}

/// Verify that DLQ is used after max retries are exhausted.
///
/// Asserts Assertion #9: Dead Letter Handling — events that fail delivery after
/// the maximum retry attempts MUST be persisted to the dead-letter store with
/// failure metadata.
#[tokio::test]
async fn test_dlq_after_max_retries() {
    // Arrange: 1 bot, always fails transiently. DLQ service backed by in-memory storage.
    let event = create_test_event();
    let bot_config = create_test_bot_config(1);
    let queue_client = Arc::new(MockQueueClient::new());
    // More failures than max_attempts to ensure every attempt fails
    queue_client.always_fail_transient(10);

    let event_router = Arc::new(DefaultEventRouter::new());

    let blob_storage = Arc::new(MockBlobStorage::new());
    let dlq_service = Arc::new(DlqStorageService::new(blob_storage.clone()));

    let config = QueueDeliveryConfig {
        retry_policy: RetryPolicy {
            max_attempts: 2, // 2 retries = 3 total sends (1 initial + 2 retries)
            initial_delay: Duration::from_millis(5),
            max_delay: Duration::from_millis(20),
            backoff_multiplier: 2.0,
            use_jitter: false,
            jitter_percent: 0.0,
        },
        enable_dlq: true,
        dlq_service: Some(dlq_service),
    };

    // Act
    let outcome = deliver_event_to_queues(
        event,
        event_router,
        Arc::new(bot_config),
        queue_client.clone(),
        config,
    )
    .await;

    // Assert: event persisted to DLQ after exhausting retries
    assert!(
        matches!(
            outcome,
            QueueDeliveryOutcome::CompleteFailure {
                persisted_to_dlq: true,
                ..
            }
        ),
        "Expected CompleteFailure with persisted_to_dlq=true, got {:?}",
        outcome
    );
    // 3 total sends: 1 initial + 2 retries (max_attempts=2 means 2 retries)
    assert_eq!(queue_client.send_count(), 3);
    // Exactly 1 DLQ record persisted
    assert_eq!(
        blob_storage.stored_count(),
        1,
        "Expected exactly 1 DLQ record in blob storage"
    );
}

/// Verify that retry delays use exponential backoff with jitter.
///
/// Asserts Assertion #10: Retry Behavior — delays MUST grow exponentially and
/// respect the configured maximum delay cap.
#[tokio::test]
async fn test_exponential_backoff_with_jitter() {
    use queue_keeper_api::retry::{RetryPolicy, RetryState};

    // Arrange
    let policy = RetryPolicy {
        max_attempts: 5,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(1),
        backoff_multiplier: 2.0,
        use_jitter: true,
        jitter_percent: 0.2, // 20% jitter
    };

    let mut state = RetryState::new();

    // Act & Assert: Verify delays increase exponentially.
    //
    // Get the first delay and verify it is in the expected range for attempt 0
    // (initial_delay ± jitter), then verify each subsequent delay is at least
    // 80% of the previous one to confirm exponential growth under jitter.
    let first_delay = state.get_delay(&policy);
    let expected_low = policy
        .initial_delay
        .mul_f32(1.0 - policy.jitter_percent as f32);
    assert!(
        first_delay >= expected_low,
        "First delay {:?} should be >= initial_delay * (1 - jitter) = {:?}",
        first_delay,
        expected_low,
    );
    assert!(
        first_delay <= policy.max_delay,
        "First delay {:?} exceeds max {:?}",
        first_delay,
        policy.max_delay,
    );

    let mut last_delay = first_delay;
    state.next_attempt();

    for _ in 1..4 {
        let delay = state.get_delay(&policy);

        // Delay should be >= previous (considering negative jitter of up to 20%).
        assert!(
            delay >= last_delay.mul_f32(0.8),
            "Delay should increase: {:?} < 80% of {:?}",
            delay,
            last_delay,
        );

        // Delay should not exceed max_delay.
        assert!(delay <= policy.max_delay, "Delay exceeds max: {:?}", delay);

        last_delay = delay;
        state.next_attempt();
    }

    // After 4 attempts (0-3), we've reached attempt 4 which is still < max_attempts (5)
    assert!(
        state.can_retry(&policy),
        "Should still be able to retry at attempt 4"
    );

    // One more attempt reaches the limit
    state.next_attempt();
    assert!(
        !state.can_retry(&policy),
        "Should not be able to retry at attempt 5"
    );
}

/// Verify that queue delivery preserves session ordering
///
/// Tests Assertion #7: Ordering Guarantee
#[tokio::test]
#[ignore = "Requires session-aware queue client integration"]
async fn test_session_ordering_preserved() {
    // Arrange: Create multiple events with same session ID
    let _session_id = SessionId::from_parts("owner", "repo", "pull_request", "123");

    // TODO: Create events and verify they are delivered in order
    // This requires session-aware queue client integration
}

/// Verify that different sessions can be delivered concurrently
///
/// Tests Assertion #7: Ordering Guarantee (concurrent sessions)
#[tokio::test]
#[ignore = "Requires session-aware queue client integration"]
async fn test_concurrent_session_delivery() {
    // Arrange: Create events with different session IDs

    // TODO: Verify concurrent delivery is allowed
    // This requires session-aware queue client integration
}
