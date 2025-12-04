//! Tests for DLQ storage module
//!
//! These tests verify:
//! - FailedEventRecord creation and serialization
//! - DLQ storage service operations
//! - Blob path generation for DLQ records
//! - Integration with BlobStorage trait

use super::*;
use async_trait::async_trait;
use queue_keeper_core::{
    blob_storage::{
        BlobMetadata, BlobStorage, BlobStorageError, PayloadFilter, StorageHealthStatus,
        StorageMetrics, StoredWebhook, WebhookPayload,
    },
    webhook::{EventEntity, EventEnvelope},
    EventId, Repository, RepositoryId, Timestamp, User, UserId, UserType,
};
use std::collections::HashMap;
use std::sync::Mutex;

// ============================================================================
// Mock Blob Storage
// ============================================================================

/// Mock blob storage for testing DLQ operations
struct MockBlobStorage {
    stored: Mutex<HashMap<EventId, StoredWebhook>>,
    should_fail: Mutex<bool>,
}

impl MockBlobStorage {
    fn new() -> Self {
        Self {
            stored: Mutex::new(HashMap::new()),
            should_fail: Mutex::new(false),
        }
    }

    fn set_should_fail(&self, fail: bool) {
        *self.should_fail.lock().unwrap() = fail;
    }

    fn get_stored_count(&self) -> usize {
        self.stored.lock().unwrap().len()
    }
}

#[async_trait]
impl BlobStorage for MockBlobStorage {
    async fn store_payload(
        &self,
        event_id: &EventId,
        payload: &WebhookPayload,
    ) -> Result<BlobMetadata, BlobStorageError> {
        if *self.should_fail.lock().unwrap() {
            return Err(BlobStorageError::ConnectionFailed {
                message: "Mock storage failure".to_string(),
            });
        }

        let blob_path = event_id.to_blob_path();
        let size = payload.body.len() as u64;

        let metadata = BlobMetadata {
            event_id: *event_id,
            blob_path: blob_path.clone(),
            size_bytes: size,
            content_type: "application/json".to_string(),
            created_at: Timestamp::now(),
            checksum_sha256: "mock_checksum".to_string(),
            metadata: payload.metadata.clone(),
        };

        let stored_webhook = StoredWebhook {
            metadata: metadata.clone(),
            payload: payload.clone(),
        };

        self.stored
            .lock()
            .unwrap()
            .insert(*event_id, stored_webhook);

        Ok(metadata)
    }

    async fn get_payload(
        &self,
        event_id: &EventId,
    ) -> Result<Option<StoredWebhook>, BlobStorageError> {
        if *self.should_fail.lock().unwrap() {
            return Err(BlobStorageError::ConnectionFailed {
                message: "Mock storage failure".to_string(),
            });
        }

        Ok(self.stored.lock().unwrap().get(event_id).cloned())
    }

    async fn list_payloads(
        &self,
        _filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError> {
        Ok(self
            .stored
            .lock()
            .unwrap()
            .values()
            .map(|s| s.metadata.clone())
            .collect())
    }

    async fn delete_payload(&self, event_id: &EventId) -> Result<(), BlobStorageError> {
        self.stored.lock().unwrap().remove(event_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError> {
        Ok(StorageHealthStatus {
            healthy: true,
            connected: true,
            last_success: Some(Timestamp::now()),
            error_message: None,
            metrics: StorageMetrics {
                avg_write_latency_ms: 1.0,
                avg_read_latency_ms: 1.0,
                success_rate: 1.0,
            },
        })
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

fn create_test_failed_record(event: EventEnvelope) -> FailedEventRecord {
    FailedEventRecord::new(
        event,
        DlqReason::RetriesExhausted { attempts: 3 },
        vec![FailedQueueInfo {
            bot_name: "test-bot".to_string(),
            queue_name: "queue-keeper-test-bot".to_string(),
            error: "Connection timeout".to_string(),
            was_transient: true,
        }],
        vec![],
        3,
        Timestamp::now(),
    )
}

// ============================================================================
// DLQ Reason Tests
// ============================================================================

/// Verify DlqReason serialization for RetriesExhausted
#[test]
fn test_dlq_reason_retries_exhausted_serialization() {
    let reason = DlqReason::RetriesExhausted { attempts: 5 };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("retries_exhausted"));
    assert!(json.contains("\"attempts\":5"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

/// Verify DlqReason serialization for PermanentFailure
#[test]
fn test_dlq_reason_permanent_failure_serialization() {
    let reason = DlqReason::PermanentFailure {
        reason: "Invalid queue name".to_string(),
    };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("permanent_failure"));
    assert!(json.contains("Invalid queue name"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

/// Verify DlqReason serialization for AllQueuesFailed
#[test]
fn test_dlq_reason_all_queues_failed_serialization() {
    let reason = DlqReason::AllQueuesFailed { queue_count: 3 };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("all_queues_failed"));
    assert!(json.contains("\"queue_count\":3"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

/// Verify DlqReason serialization for RoutingError
#[test]
fn test_dlq_reason_routing_error_serialization() {
    let reason = DlqReason::RoutingError {
        error: "No routes configured".to_string(),
    };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("routing_error"));
    assert!(json.contains("No routes configured"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

// ============================================================================
// FailedQueueInfo Tests
// ============================================================================

/// Verify FailedQueueInfo serialization
#[test]
fn test_failed_queue_info_serialization() {
    let info = FailedQueueInfo {
        bot_name: "merge-warden".to_string(),
        queue_name: "queue-keeper-merge-warden".to_string(),
        error: "Service unavailable".to_string(),
        was_transient: true,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("merge-warden"));
    assert!(json.contains("queue-keeper-merge-warden"));
    assert!(json.contains("Service unavailable"));
    assert!(json.contains("\"was_transient\":true"));

    let deserialized: FailedQueueInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, info);
}

// ============================================================================
// FailedEventRecord Tests
// ============================================================================

/// Verify FailedEventRecord creation sets correct fields
#[test]
fn test_failed_event_record_creation() {
    let event = create_test_event();
    let event_id = event.event_id;
    let first_attempt_at = Timestamp::now();

    let record = FailedEventRecord::new(
        event.clone(),
        DlqReason::RetriesExhausted { attempts: 3 },
        vec![FailedQueueInfo {
            bot_name: "bot-1".to_string(),
            queue_name: "queue-1".to_string(),
            error: "timeout".to_string(),
            was_transient: true,
        }],
        vec!["bot-2/queue-2".to_string()],
        3,
        first_attempt_at,
    );

    assert_eq!(record.event_id, event_id);
    assert_eq!(record.retry_attempts, 3);
    assert_eq!(record.failed_queues.len(), 1);
    assert_eq!(record.successful_queues.len(), 1);
    assert_eq!(record.first_attempt_at, first_attempt_at);
    assert!(record.moved_to_dlq_at >= first_attempt_at);
}

/// Verify blob path generation for DLQ records
#[test]
fn test_failed_event_record_blob_path() {
    let event = create_test_event();
    let record = create_test_failed_record(event);

    let path = record.to_blob_path();

    // Should start with dlq/ prefix
    assert!(path.starts_with("dlq/"));

    // Should contain year/month/day/hour partitioning
    assert!(path.contains("/year="));
    assert!(path.contains("/month="));
    assert!(path.contains("/day="));
    assert!(path.contains("/hour="));

    // Should end with event_id.json
    assert!(path.ends_with(".json"));
    assert!(path.contains(&record.event_id.to_string()));
}

/// Verify FailedEventRecord serialization roundtrip
#[test]
fn test_failed_event_record_serialization_roundtrip() {
    let event = create_test_event();
    let record = create_test_failed_record(event);

    let json = serde_json::to_string_pretty(&record).unwrap();

    // Verify key fields are present
    assert!(json.contains(&record.event_id.to_string()));
    assert!(json.contains("retries_exhausted"));
    assert!(json.contains("test-bot"));
    assert!(json.contains("Connection timeout"));

    // Verify roundtrip
    let deserialized: FailedEventRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.event_id, record.event_id);
    assert_eq!(deserialized.retry_attempts, record.retry_attempts);
    assert_eq!(deserialized.failed_queues.len(), record.failed_queues.len());
}

// ============================================================================
// DlqStorageService Tests
// ============================================================================

/// Verify DLQ storage service can persist failed events
#[tokio::test]
async fn test_dlq_storage_service_persist_event() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage.clone());

    let event = create_test_event();
    let record = create_test_failed_record(event);

    let result = service.persist_failed_event(&record).await;

    assert!(result.is_ok());
    assert_eq!(storage.get_stored_count(), 1);

    let blob_path = result.unwrap();
    assert!(blob_path.contains(&record.event_id.to_string()));
}

/// Verify DLQ storage service can retrieve failed events
#[tokio::test]
async fn test_dlq_storage_service_get_failed_event() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage.clone());

    let event = create_test_event();
    let event_id = event.event_id;
    let record = create_test_failed_record(event);

    // Store the record
    service.persist_failed_event(&record).await.unwrap();

    // Retrieve it
    let retrieved = service.get_failed_event(&event_id).await.unwrap();

    assert!(retrieved.is_some());
    let retrieved_record = retrieved.unwrap();
    assert_eq!(retrieved_record.event_id, event_id);
    assert_eq!(retrieved_record.retry_attempts, record.retry_attempts);
}

/// Verify DLQ storage service returns None for non-existent events
#[tokio::test]
async fn test_dlq_storage_service_get_nonexistent_event() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage);

    let event_id = EventId::new();
    let retrieved = service.get_failed_event(&event_id).await.unwrap();

    assert!(retrieved.is_none());
}

/// Verify DLQ storage service handles storage failures
#[tokio::test]
async fn test_dlq_storage_service_handles_storage_failure() {
    let storage = Arc::new(MockBlobStorage::new());
    storage.set_should_fail(true);

    let service = DlqStorageService::new(storage);

    let event = create_test_event();
    let record = create_test_failed_record(event);

    let result = service.persist_failed_event(&record).await;

    assert!(result.is_err());
    match result {
        Err(BlobStorageError::ConnectionFailed { message }) => {
            assert!(message.contains("Mock storage failure"));
        }
        _ => panic!("Expected ConnectionFailed error"),
    }
}

// ============================================================================
// Helper Function Tests
// ============================================================================

/// Verify create_failed_event_record helper creates correct record
#[test]
fn test_create_failed_event_record_helper() {
    let event = create_test_event();
    let event_id = event.event_id;
    let first_attempt = Timestamp::now();

    let failed_queues = vec![(
        queue_keeper_core::BotName::new("bot-1".to_string()).unwrap(),
        queue_keeper_core::QueueName::new("queue-keeper-bot-1".to_string()).unwrap(),
        "timeout".to_string(),
        true,
    )];

    let successful_queues = vec![(
        queue_keeper_core::BotName::new("bot-2".to_string()).unwrap(),
        queue_keeper_core::QueueName::new("queue-keeper-bot-2".to_string()).unwrap(),
    )];

    let record = create_failed_event_record(
        event,
        failed_queues,
        successful_queues,
        3,
        first_attempt,
        DlqReason::RetriesExhausted { attempts: 3 },
    );

    assert_eq!(record.event_id, event_id);
    assert_eq!(record.failed_queues.len(), 1);
    assert_eq!(record.failed_queues[0].bot_name, "bot-1");
    assert_eq!(record.failed_queues[0].error, "timeout");
    assert!(record.failed_queues[0].was_transient);

    assert_eq!(record.successful_queues.len(), 1);
    assert!(record.successful_queues[0].contains("bot-2"));
}

/// Verify persist_to_dlq returns true on success
#[tokio::test]
async fn test_persist_to_dlq_success() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage.clone());

    let event = create_test_event();
    let record = create_test_failed_record(event);

    let result = persist_to_dlq(Some(&service), &record).await;

    assert!(result);
    assert_eq!(storage.get_stored_count(), 1);
}

/// Verify persist_to_dlq returns false when service is None
#[tokio::test]
async fn test_persist_to_dlq_disabled() {
    let event = create_test_event();
    let record = create_test_failed_record(event);

    let result = persist_to_dlq(None, &record).await;

    assert!(!result);
}

/// Verify persist_to_dlq returns false on storage failure
#[tokio::test]
async fn test_persist_to_dlq_storage_failure() {
    let storage = Arc::new(MockBlobStorage::new());
    storage.set_should_fail(true);

    let service = DlqStorageService::new(storage);

    let event = create_test_event();
    let record = create_test_failed_record(event);

    let result = persist_to_dlq(Some(&service), &record).await;

    assert!(!result);
}
