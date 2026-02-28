//! Tests for DLQ storage module

use super::*;
use queue_keeper_core::{
    blob_storage::{
        BlobMetadata, BlobStorage, BlobStorageError, PayloadFilter, StorageHealthStatus,
        StoredWebhook, WebhookPayload,
    },
    webhook::WrappedEvent,
    BotName, EventId, QueueName, Timestamp,
};
use std::collections::HashMap;
use std::sync::Mutex;

// ============================================================================
// Mock BlobStorage
// ============================================================================

#[derive(Clone)]
struct MockBlobStorage {
    payloads: Arc<Mutex<HashMap<EventId, StoredWebhook>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl MockBlobStorage {
    fn new() -> Self {
        Self {
            payloads: Arc::new(Mutex::new(HashMap::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    fn set_should_fail(&self, fail: bool) {
        *self.should_fail.lock().unwrap() = fail;
    }

    fn get_payload_count(&self) -> usize {
        self.payloads.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
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

        let blob_path = format!("dlq/{}.json", event_id);
        let size_bytes = payload.body.len() as u64;
        let checksum = queue_keeper_core::blob_storage::compute_checksum(&payload.body);

        let metadata = BlobMetadata {
            event_id: *event_id,
            blob_path: blob_path.clone(),
            size_bytes,
            content_type: "application/json".to_string(),
            created_at: Timestamp::now(),
            checksum_sha256: checksum,
            metadata: payload.metadata.clone(),
        };

        self.payloads.lock().unwrap().insert(
            *event_id,
            StoredWebhook {
                metadata: metadata.clone(),
                payload: payload.clone(),
            },
        );

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

        Ok(self.payloads.lock().unwrap().get(event_id).cloned())
    }

    async fn list_payloads(
        &self,
        _filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError> {
        Ok(self
            .payloads
            .lock()
            .unwrap()
            .values()
            .map(|stored| stored.metadata.clone())
            .collect())
    }

    async fn delete_payload(&self, event_id: &EventId) -> Result<(), BlobStorageError> {
        self.payloads.lock().unwrap().remove(event_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError> {
        Ok(StorageHealthStatus {
            healthy: true,
            connected: true,
            last_success: Some(Timestamp::now()),
            error_message: None,
            metrics: queue_keeper_core::blob_storage::StorageMetrics {
                avg_write_latency_ms: 10.0,
                avg_read_latency_ms: 5.0,
                success_rate: 1.0,
            },
        })
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_event() -> WrappedEvent {
    WrappedEvent::new(
        "github".to_string(),
        "pull_request".to_string(),
        Some("opened".to_string()),
        None,
        serde_json::json!({
            "action": "opened",
            "repository": {
                "id": 123,
                "name": "test-repo",
                "full_name": "owner/test-repo",
                "owner": {"login": "owner", "id": 1, "type": "User"},
                "private": false
            },
            "pull_request": {"number": 42}
        }),
    )
}

fn create_failed_event_record() -> FailedEventRecord {
    let event = create_test_event();
    FailedEventRecord::new(
        event,
        DlqReason::RetriesExhausted { attempts: 5 },
        vec![
            FailedQueueInfo {
                bot_name: "bot1".to_string(),
                queue_name: "queue1".to_string(),
                error: "Connection timeout".to_string(),
                was_transient: true,
            },
            FailedQueueInfo {
                bot_name: "bot2".to_string(),
                queue_name: "queue2".to_string(),
                error: "Queue not found".to_string(),
                was_transient: false,
            },
        ],
        vec!["bot3/queue3".to_string()],
        5,
        Timestamp::now(),
    )
}

// ============================================================================
// DlqReason Tests
// ============================================================================

#[test]
fn test_dlq_reason_retries_exhausted() {
    let reason = DlqReason::RetriesExhausted { attempts: 5 };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("retries_exhausted"));
    assert!(json.contains("\"attempts\":5"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

#[test]
fn test_dlq_reason_permanent_failure() {
    let reason = DlqReason::PermanentFailure {
        reason: "Invalid message format".to_string(),
    };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("permanent_failure"));
    assert!(json.contains("Invalid message format"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

#[test]
fn test_dlq_reason_all_queues_failed() {
    let reason = DlqReason::AllQueuesFailed { queue_count: 3 };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("all_queues_failed"));
    assert!(json.contains("\"queue_count\":3"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

#[test]
fn test_dlq_reason_routing_error() {
    let reason = DlqReason::RoutingError {
        error: "Configuration not found".to_string(),
    };

    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("routing_error"));
    assert!(json.contains("Configuration not found"));

    let deserialized: DlqReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

// ============================================================================
// FailedQueueInfo Tests
// ============================================================================

#[test]
fn test_failed_queue_info_serialization() {
    let info = FailedQueueInfo {
        bot_name: "test-bot".to_string(),
        queue_name: "test-queue".to_string(),
        error: "Connection failed".to_string(),
        was_transient: true,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("test-bot"));
    assert!(json.contains("test-queue"));
    assert!(json.contains("Connection failed"));
    assert!(json.contains("\"was_transient\":true"));

    let deserialized: FailedQueueInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, info);
}

// ============================================================================
// FailedEventRecord Tests
// ============================================================================

#[test]
fn test_failed_event_record_new() {
    let event = create_test_event();
    let event_id = event.event_id;
    let correlation_id = event.correlation_id.to_string();
    let first_attempt = Timestamp::now();

    let record = FailedEventRecord::new(
        event,
        DlqReason::RetriesExhausted { attempts: 3 },
        vec![],
        vec![],
        3,
        first_attempt,
    );

    assert_eq!(record.event_id, event_id);
    assert_eq!(record.correlation_id, correlation_id);
    assert_eq!(record.retry_attempts, 3);
    assert_eq!(record.first_attempt_at, first_attempt);
    assert!(matches!(
        record.reason,
        DlqReason::RetriesExhausted { attempts: 3 }
    ));
}

#[test]
fn test_failed_event_record_to_blob_path() {
    let record = create_failed_event_record();
    let path = record.to_blob_path();

    // Should start with dlq/ prefix
    assert!(path.starts_with("dlq/"));

    // Should contain time-based partitioning
    assert!(path.contains("year="));
    assert!(path.contains("/month="));
    assert!(path.contains("/day="));
    assert!(path.contains("/hour="));

    // Should end with event ID and .json
    assert!(path.ends_with(&format!("{}.json", record.event_id)));
}

#[test]
fn test_failed_event_record_serialization() {
    let record = create_failed_event_record();

    let json = serde_json::to_string_pretty(&record).unwrap();

    // Should contain all key fields
    assert!(json.contains("event_id"));
    assert!(json.contains("reason"));
    assert!(json.contains("failed_queues"));
    assert!(json.contains("successful_queues"));
    assert!(json.contains("retry_attempts"));

    // Should be deserializable
    let deserialized: FailedEventRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.event_id, record.event_id);
    assert_eq!(deserialized.retry_attempts, record.retry_attempts);
    assert_eq!(deserialized.failed_queues.len(), record.failed_queues.len());
}

// ============================================================================
// DlqStorageService Tests
// ============================================================================

#[tokio::test]
async fn test_dlq_storage_service_persist_and_retrieve() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage.clone());

    let record = create_failed_event_record();
    let event_id = record.event_id;

    // Persist the record
    let result = service.persist_failed_event(&record).await;
    assert!(result.is_ok());

    let blob_path = result.unwrap();
    assert!(blob_path.starts_with("dlq/"));

    // Verify storage received the payload
    assert_eq!(storage.get_payload_count(), 1);

    // Retrieve the record
    let retrieved = service.get_failed_event(&event_id).await.unwrap();
    assert!(retrieved.is_some());

    let retrieved_record = retrieved.unwrap();
    assert_eq!(retrieved_record.event_id, event_id);
    assert_eq!(retrieved_record.retry_attempts, record.retry_attempts);
    assert_eq!(
        retrieved_record.failed_queues.len(),
        record.failed_queues.len()
    );
}

#[tokio::test]
async fn test_dlq_storage_service_retrieve_nonexistent() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage);

    let nonexistent_id = EventId::new();

    let result = service.get_failed_event(&nonexistent_id).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_dlq_storage_service_persist_storage_failure() {
    let storage = Arc::new(MockBlobStorage::new());
    storage.set_should_fail(true);

    let service = DlqStorageService::new(storage);

    let record = create_failed_event_record();

    let result = service.persist_failed_event(&record).await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        BlobStorageError::ConnectionFailed { .. }
    ));
}

#[tokio::test]
async fn test_dlq_storage_service_retrieve_storage_failure() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage.clone());

    let record = create_failed_event_record();
    let event_id = record.event_id;

    // First persist successfully
    service.persist_failed_event(&record).await.unwrap();

    // Then simulate storage failure
    storage.set_should_fail(true);

    let result = service.get_failed_event(&event_id).await;
    assert!(result.is_err());
}

// ============================================================================
// Helper Function Tests
// ============================================================================

#[test]
fn test_create_failed_event_record() {
    let event = create_test_event();
    let event_id = event.event_id;

    let failed_queues = vec![
        (
            BotName::new("bot1".to_string()).unwrap(),
            QueueName::new("queue-keeper-bot1".to_string()).unwrap(),
            "Error 1".to_string(),
            true,
        ),
        (
            BotName::new("bot2".to_string()).unwrap(),
            QueueName::new("queue-keeper-bot2".to_string()).unwrap(),
            "Error 2".to_string(),
            false,
        ),
    ];

    let successful_queues = vec![(
        BotName::new("bot3".to_string()).unwrap(),
        QueueName::new("queue-keeper-bot3".to_string()).unwrap(),
    )];

    let first_attempt = Timestamp::now();
    let reason = DlqReason::RetriesExhausted { attempts: 5 };

    let record = super::create_failed_event_record(
        event,
        failed_queues,
        successful_queues,
        5,
        first_attempt,
        reason,
    );

    assert_eq!(record.event_id, event_id);
    assert_eq!(record.failed_queues.len(), 2);
    assert_eq!(record.successful_queues.len(), 1);
    assert_eq!(record.retry_attempts, 5);

    // Check failed queue info
    assert_eq!(record.failed_queues[0].bot_name, "bot1");
    assert_eq!(record.failed_queues[0].queue_name, "queue-keeper-bot1");
    assert_eq!(record.failed_queues[0].error, "Error 1");
    assert!(record.failed_queues[0].was_transient);

    // Check successful queue format
    assert_eq!(record.successful_queues[0], "bot3/queue-keeper-bot3");
}

#[tokio::test]
async fn test_persist_to_dlq_success() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage.clone());

    let record = create_failed_event_record();

    let result = persist_to_dlq(Some(&service), &record).await;
    assert!(result);

    // Verify storage received the payload
    assert_eq!(storage.get_payload_count(), 1);
}

#[tokio::test]
async fn test_persist_to_dlq_no_service() {
    let record = create_failed_event_record();

    let result = persist_to_dlq(None, &record).await;
    assert!(!result); // Should return false when service is None
}

#[tokio::test]
async fn test_persist_to_dlq_storage_failure() {
    let storage = Arc::new(MockBlobStorage::new());
    storage.set_should_fail(true);

    let service = DlqStorageService::new(storage);

    let record = create_failed_event_record();

    let result = persist_to_dlq(Some(&service), &record).await;
    assert!(!result); // Should return false on storage failure
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_dlq_workflow() {
    let storage = Arc::new(MockBlobStorage::new());
    let service = DlqStorageService::new(storage.clone());

    // Create multiple failed events
    let mut records = vec![];
    for i in 0..3 {
        let mut event = create_test_event();
        event.event_type = format!("event_type_{}", i);

        let record = FailedEventRecord::new(
            event,
            DlqReason::RetriesExhausted {
                attempts: i as u32 + 1,
            },
            vec![],
            vec![],
            i as u32 + 1,
            Timestamp::now(),
        );

        records.push(record);
    }

    // Persist all records
    for record in &records {
        let result = service.persist_failed_event(record).await;
        assert!(result.is_ok());
    }

    assert_eq!(storage.get_payload_count(), 3);

    // Retrieve and verify each record
    for original in &records {
        let retrieved = service
            .get_failed_event(&original.event_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.event_id, original.event_id);
        assert_eq!(retrieved.event.event_type, original.event.event_type);
        assert_eq!(retrieved.retry_attempts, original.retry_attempts);
    }
}
