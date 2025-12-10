//! Tests for queue delivery module

use super::*;

// ============================================================================
// QueueDeliveryConfig Tests
// ============================================================================

#[test]
fn test_queue_delivery_config_default() {
    let config = QueueDeliveryConfig::default();

    assert_eq!(config.retry_policy.max_attempts, 5);
    assert!(config.enable_dlq);
    assert!(config.dlq_service.is_none());
}

#[test]
fn test_queue_delivery_config_with_dlq_service() {
    use queue_keeper_core::blob_storage::BlobStorage;
    use std::sync::Arc;

    // Create a mock storage (we'll use a simple in-memory one)
    struct MockStorage;

    #[async_trait::async_trait]
    impl BlobStorage for MockStorage {
        async fn store_payload(
            &self,
            _event_id: &queue_keeper_core::EventId,
            _payload: &queue_keeper_core::blob_storage::WebhookPayload,
        ) -> Result<
            queue_keeper_core::blob_storage::BlobMetadata,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }

        async fn get_payload(
            &self,
            _event_id: &queue_keeper_core::EventId,
        ) -> Result<
            Option<queue_keeper_core::blob_storage::StoredWebhook>,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }

        async fn list_payloads(
            &self,
            _filter: &queue_keeper_core::blob_storage::PayloadFilter,
        ) -> Result<
            Vec<queue_keeper_core::blob_storage::BlobMetadata>,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }

        async fn delete_payload(
            &self,
            _event_id: &queue_keeper_core::EventId,
        ) -> Result<(), queue_keeper_core::blob_storage::BlobStorageError> {
            unimplemented!()
        }

        async fn health_check(
            &self,
        ) -> Result<
            queue_keeper_core::blob_storage::StorageHealthStatus,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }
    }

    let storage = Arc::new(MockStorage);
    let dlq_service = Arc::new(DlqStorageService::new(storage));

    let config = QueueDeliveryConfig::default().with_dlq_service(dlq_service.clone());

    assert!(config.dlq_service.is_some());
    assert!(Arc::ptr_eq(&config.dlq_service.unwrap(), &dlq_service));
}

// ============================================================================
// QueueDeliveryOutcome Tests
// ============================================================================

#[test]
fn test_delivery_outcome_all_queues_succeeded_is_success() {
    let outcome = QueueDeliveryOutcome::AllQueuesSucceeded {
        event_id: queue_keeper_core::EventId::new(),
        successful_count: 3,
    };

    assert!(outcome.is_success());
    assert!(!outcome.has_failures());
}

#[test]
fn test_delivery_outcome_no_target_queues_is_success() {
    let outcome = QueueDeliveryOutcome::NoTargetQueues {
        event_id: queue_keeper_core::EventId::new(),
    };

    assert!(outcome.is_success());
    assert!(!outcome.has_failures());
}

#[test]
fn test_delivery_outcome_some_queues_failed_has_failures() {
    let outcome = QueueDeliveryOutcome::SomeQueuesFailed {
        event_id: queue_keeper_core::EventId::new(),
        successful_count: 2,
        failed_count: 1,
        persisted_to_dlq: false,
    };

    assert!(!outcome.is_success());
    assert!(outcome.has_failures());
}

#[test]
fn test_delivery_outcome_complete_failure_has_failures() {
    let outcome = QueueDeliveryOutcome::CompleteFailure {
        event_id: queue_keeper_core::EventId::new(),
        error: "All failed".to_string(),
        persisted_to_dlq: false,
    };

    assert!(!outcome.is_success());
    assert!(outcome.has_failures());
}

#[test]
fn test_delivery_outcome_dlq_persistence_tracking() {
    // Test SomeQueuesFailed with DLQ
    let outcome = QueueDeliveryOutcome::SomeQueuesFailed {
        event_id: queue_keeper_core::EventId::new(),
        successful_count: 1,
        failed_count: 2,
        persisted_to_dlq: true,
    };

    match outcome {
        QueueDeliveryOutcome::SomeQueuesFailed {
            persisted_to_dlq, ..
        } => {
            assert!(persisted_to_dlq);
        }
        _ => panic!("Expected SomeQueuesFailed"),
    }

    // Test CompleteFailure with DLQ
    let outcome = QueueDeliveryOutcome::CompleteFailure {
        event_id: queue_keeper_core::EventId::new(),
        error: "Error".to_string(),
        persisted_to_dlq: true,
    };

    match outcome {
        QueueDeliveryOutcome::CompleteFailure {
            persisted_to_dlq, ..
        } => {
            assert!(persisted_to_dlq);
        }
        _ => panic!("Expected CompleteFailure"),
    }
}

// ============================================================================
// Retry Policy Integration Tests
// ============================================================================

#[test]
fn test_config_custom_retry_policy() {
    let custom_policy = RetryPolicy::new(
        3,
        std::time::Duration::from_millis(500),
        std::time::Duration::from_secs(5),
        1.5,
    );

    let config = QueueDeliveryConfig {
        retry_policy: custom_policy.clone(),
        enable_dlq: true,
        dlq_service: None,
    };

    assert_eq!(config.retry_policy.max_attempts, 3);
    assert_eq!(
        config.retry_policy.initial_delay,
        std::time::Duration::from_millis(500)
    );
    assert_eq!(config.retry_policy.backoff_multiplier, 1.5);
}

#[test]
fn test_config_without_dlq() {
    let config = QueueDeliveryConfig {
        retry_policy: RetryPolicy::default(),
        enable_dlq: false,
        dlq_service: None,
    };

    assert!(!config.enable_dlq);
    assert!(config.dlq_service.is_none());
}

// ============================================================================
// Outcome Pattern Matching Tests
// ============================================================================

#[test]
fn test_outcome_pattern_matching() {
    let event_id = queue_keeper_core::EventId::new();

    // Test AllQueuesSucceeded
    let outcome = QueueDeliveryOutcome::AllQueuesSucceeded {
        event_id,
        successful_count: 5,
    };

    if let QueueDeliveryOutcome::AllQueuesSucceeded {
        event_id: id,
        successful_count,
    } = outcome
    {
        assert_eq!(id, event_id);
        assert_eq!(successful_count, 5);
    } else {
        panic!("Expected AllQueuesSucceeded");
    }

    // Test NoTargetQueues
    let outcome = QueueDeliveryOutcome::NoTargetQueues { event_id };

    if let QueueDeliveryOutcome::NoTargetQueues { event_id: id } = outcome {
        assert_eq!(id, event_id);
    } else {
        panic!("Expected NoTargetQueues");
    }

    // Test SomeQueuesFailed
    let outcome = QueueDeliveryOutcome::SomeQueuesFailed {
        event_id,
        successful_count: 2,
        failed_count: 3,
        persisted_to_dlq: true,
    };

    if let QueueDeliveryOutcome::SomeQueuesFailed {
        event_id: id,
        successful_count,
        failed_count,
        persisted_to_dlq,
    } = outcome
    {
        assert_eq!(id, event_id);
        assert_eq!(successful_count, 2);
        assert_eq!(failed_count, 3);
        assert!(persisted_to_dlq);
    } else {
        panic!("Expected SomeQueuesFailed");
    }

    // Test CompleteFailure
    let outcome = QueueDeliveryOutcome::CompleteFailure {
        event_id,
        error: "Test error".to_string(),
        persisted_to_dlq: false,
    };

    if let QueueDeliveryOutcome::CompleteFailure {
        event_id: id,
        error,
        persisted_to_dlq,
    } = outcome
    {
        assert_eq!(id, event_id);
        assert_eq!(error, "Test error");
        assert!(!persisted_to_dlq);
    } else {
        panic!("Expected CompleteFailure");
    }
}

// ============================================================================
// Configuration Builder Pattern Tests
// ============================================================================

#[test]
fn test_config_builder_pattern() {
    use queue_keeper_core::blob_storage::BlobStorage;

    struct MockStorage;

    #[async_trait::async_trait]
    impl BlobStorage for MockStorage {
        async fn store_payload(
            &self,
            _event_id: &queue_keeper_core::EventId,
            _payload: &queue_keeper_core::blob_storage::WebhookPayload,
        ) -> Result<
            queue_keeper_core::blob_storage::BlobMetadata,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }

        async fn get_payload(
            &self,
            _event_id: &queue_keeper_core::EventId,
        ) -> Result<
            Option<queue_keeper_core::blob_storage::StoredWebhook>,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }

        async fn list_payloads(
            &self,
            _filter: &queue_keeper_core::blob_storage::PayloadFilter,
        ) -> Result<
            Vec<queue_keeper_core::blob_storage::BlobMetadata>,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }

        async fn delete_payload(
            &self,
            _event_id: &queue_keeper_core::EventId,
        ) -> Result<(), queue_keeper_core::blob_storage::BlobStorageError> {
            unimplemented!()
        }

        async fn health_check(
            &self,
        ) -> Result<
            queue_keeper_core::blob_storage::StorageHealthStatus,
            queue_keeper_core::blob_storage::BlobStorageError,
        > {
            unimplemented!()
        }
    }

    let storage = Arc::new(MockStorage);
    let dlq_service = Arc::new(DlqStorageService::new(storage));

    let config = QueueDeliveryConfig::default().with_dlq_service(dlq_service);

    assert!(config.dlq_service.is_some());
    assert!(config.enable_dlq);
}
