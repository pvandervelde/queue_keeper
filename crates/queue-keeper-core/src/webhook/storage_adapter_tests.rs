//! Tests for webhook storage adapter

use super::*;
use crate::blob_storage::{
    BlobMetadata, BlobStorage, BlobStorageError, PayloadFilter, StorageHealthStatus,
    StorageMetrics, StoredWebhook,
};
use crate::webhook::{PayloadFilters, WebhookHeaders, WebhookRequest};
use crate::{EventId, Timestamp};
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock blob storage for testing
struct MockBlobStorage {
    stored_payloads: Arc<Mutex<HashMap<EventId, WebhookPayload>>>,
    should_fail: bool,
}

impl MockBlobStorage {
    fn new() -> Self {
        Self {
            stored_payloads: Arc::new(Mutex::new(HashMap::new())),
            should_fail: false,
        }
    }

    fn with_failure() -> Self {
        Self {
            stored_payloads: Arc::new(Mutex::new(HashMap::new())),
            should_fail: true,
        }
    }

    fn get_stored_count(&self) -> usize {
        self.stored_payloads.lock().unwrap().len()
    }
}

#[async_trait]
impl BlobStorage for MockBlobStorage {
    async fn store_payload(
        &self,
        event_id: &EventId,
        payload: &WebhookPayload,
    ) -> Result<BlobMetadata, BlobStorageError> {
        if self.should_fail {
            return Err(BlobStorageError::InternalError {
                message: "Mock storage failure".to_string(),
            });
        }

        self.stored_payloads
            .lock()
            .unwrap()
            .insert(payload.metadata.event_id, payload.clone());

        Ok(BlobMetadata {
            event_id: payload.metadata.event_id,
            blob_path: format!("webhook-payloads/test/{}.json", event_id),
            size_bytes: payload.body.len() as u64,
            content_type: "application/json".to_string(),
            created_at: Timestamp::now(),
            metadata: payload.metadata.clone(),
        })
    }

    async fn get_payload(
        &self,
        event_id: &EventId,
    ) -> Result<Option<StoredWebhook>, BlobStorageError> {
        if self.should_fail {
            return Err(BlobStorageError::InternalError {
                message: "Mock storage failure".to_string(),
            });
        }

        Ok(self
            .stored_payloads
            .lock()
            .unwrap()
            .get(event_id)
            .map(|payload| StoredWebhook {
                metadata: BlobMetadata {
                    event_id: payload.metadata.event_id,
                    blob_path: format!("webhook-payloads/test/{}.json", event_id),
                    size_bytes: payload.body.len() as u64,
                    content_type: "application/json".to_string(),
                    created_at: Timestamp::now(),
                    metadata: payload.metadata.clone(),
                },
                payload: payload.clone(),
            }))
    }

    async fn list_payloads(
        &self,
        _filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError> {
        if self.should_fail {
            return Err(BlobStorageError::InternalError {
                message: "Mock storage failure".to_string(),
            });
        }

        Ok(self
            .stored_payloads
            .lock()
            .unwrap()
            .iter()
            .map(|(event_id, payload)| BlobMetadata {
                event_id: payload.metadata.event_id,
                blob_path: format!("webhook-payloads/test/{}.json", event_id),
                size_bytes: payload.body.len() as u64,
                content_type: "application/json".to_string(),
                created_at: Timestamp::now(),
                metadata: payload.metadata.clone(),
            })
            .collect())
    }

    async fn delete_payload(&self, event_id: &EventId) -> Result<(), BlobStorageError> {
        if self.should_fail {
            return Err(BlobStorageError::InternalError {
                message: "Mock storage failure".to_string(),
            });
        }

        self.stored_payloads.lock().unwrap().remove(event_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError> {
        if self.should_fail {
            return Err(BlobStorageError::InternalError {
                message: "Mock storage failure".to_string(),
            });
        }

        Ok(StorageHealthStatus {
            healthy: true,
            connected: true,
            last_success: Some(Timestamp::now()),
            error_message: None,
            metrics: StorageMetrics {
                avg_write_latency_ms: 10.0,
                avg_read_latency_ms: 5.0,
                success_rate: 1.0,
            },
        })
    }
}

fn create_test_webhook_request() -> WebhookRequest {
    let mut headers_map = HashMap::new();
    headers_map.insert("x-github-event".to_string(), "pull_request".to_string());
    headers_map.insert(
        "x-github-delivery".to_string(),
        "12345678-1234-1234-1234-123456789012".to_string(),
    );
    headers_map.insert(
        "x-hub-signature-256".to_string(),
        "sha256=abcd1234".to_string(),
    );
    headers_map.insert("user-agent".to_string(), "GitHub-Hookshot/test".to_string());
    headers_map.insert("content-type".to_string(), "application/json".to_string());

    let headers = WebhookHeaders::from_http_headers(&headers_map).unwrap();

    // Create a valid JSON payload with repository info
    let payload = serde_json::json!({
        "action": "opened",
        "pull_request": {
            "number": 42,
            "title": "Test PR"
        },
        "repository": {
            "id": 12345,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "private": false,
            "owner": {
                "id": 67890,
                "login": "owner",
                "type": "User"
            }
        }
    });

    WebhookRequest::new(headers, Bytes::from(serde_json::to_vec(&payload).unwrap()))
}

/// Test storing a webhook payload successfully
#[tokio::test]
async fn test_storage_adapter_store_payload_success() {
    let mock_storage = Arc::new(MockBlobStorage::new());
    let adapter = BlobStorageAdapter::new(mock_storage.clone());

    let request = create_test_webhook_request();
    let result = adapter
        .store_payload(&request, ValidationStatus::Valid)
        .await;

    assert!(result.is_ok());
    let storage_ref = result.unwrap();
    assert!(storage_ref.blob_path.contains("webhook-payloads"));
    assert_eq!(storage_ref.size_bytes, request.body.len() as u64);
    assert_eq!(mock_storage.get_stored_count(), 1);
}

/// Test storing with invalid signature validation status
#[tokio::test]
async fn test_storage_adapter_store_invalid_signature() {
    let mock_storage = Arc::new(MockBlobStorage::new());
    let adapter = BlobStorageAdapter::new(mock_storage.clone());

    let request = create_test_webhook_request();
    let result = adapter
        .store_payload(&request, ValidationStatus::InvalidSignature)
        .await;

    assert!(result.is_ok());
    assert_eq!(mock_storage.get_stored_count(), 1);
}

/// Test storing with malformed payload
#[tokio::test]
async fn test_storage_adapter_store_malformed_payload() {
    let mock_storage = Arc::new(MockBlobStorage::new());
    let adapter = BlobStorageAdapter::new(mock_storage.clone());

    let mut headers_map = HashMap::new();
    headers_map.insert("x-github-event".to_string(), "pull_request".to_string());
    headers_map.insert(
        "x-github-delivery".to_string(),
        "12345678-1234-1234-1234-123456789012".to_string(),
    );
    headers_map.insert("x-hub-signature-256".to_string(), "sha256=test".to_string());
    let headers = WebhookHeaders::from_http_headers(&headers_map).unwrap();

    // Malformed JSON
    let request = WebhookRequest::new(headers, Bytes::from("{invalid json"));

    let result = adapter
        .store_payload(&request, ValidationStatus::MalformedPayload)
        .await;

    // Should still succeed with placeholder metadata
    assert!(result.is_ok());
    assert_eq!(mock_storage.get_stored_count(), 1);
}

/// Test storage failure handling
#[tokio::test]
async fn test_storage_adapter_store_failure() {
    let mock_storage = Arc::new(MockBlobStorage::with_failure());
    let adapter = BlobStorageAdapter::new(mock_storage.clone());

    let request = create_test_webhook_request();
    let result = adapter
        .store_payload(&request, ValidationStatus::Valid)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        StorageError::OperationFailed { message } => {
            assert!(message.contains("Mock storage failure"));
        }
        _ => panic!("Expected OperationFailed error"),
    }
}

/// Test retrieving a stored payload
#[tokio::test]
async fn test_storage_adapter_retrieve_payload() {
    let mock_storage = Arc::new(MockBlobStorage::new());
    let adapter = BlobStorageAdapter::new(mock_storage.clone());

    // Store a payload first
    let request = create_test_webhook_request();
    let storage_ref = adapter
        .store_payload(&request, ValidationStatus::Valid)
        .await
        .unwrap();

    // Retrieve it
    let result = adapter.retrieve_payload(&storage_ref).await;

    assert!(result.is_ok());
    let retrieved = result.unwrap();
    assert_eq!(retrieved.event_type(), request.event_type());
    assert_eq!(retrieved.delivery_id(), request.delivery_id());
    assert_eq!(retrieved.body, request.body);
}

/// Test retrieving with invalid blob path
#[tokio::test]
async fn test_storage_adapter_retrieve_invalid_path() {
    let mock_storage = Arc::new(MockBlobStorage::new());
    let adapter = BlobStorageAdapter::new(mock_storage);

    let storage_ref = StorageReference {
        blob_path: "invalid/path/format".to_string(),
        stored_at: Timestamp::now(),
        size_bytes: 100,
    };

    let result = adapter.retrieve_payload(&storage_ref).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        StorageError::OperationFailed { message } => {
            assert!(message.contains("Invalid blob path format"));
        }
        _ => panic!("Expected OperationFailed error"),
    }
}

/// Test listing payloads
#[tokio::test]
async fn test_storage_adapter_list_payloads() {
    let mock_storage = Arc::new(MockBlobStorage::new());
    let adapter = BlobStorageAdapter::new(mock_storage.clone());

    // Store multiple payloads
    for _ in 0..3 {
        let request = create_test_webhook_request();
        adapter
            .store_payload(&request, ValidationStatus::Valid)
            .await
            .unwrap();
    }

    // List them
    let filters = PayloadFilters {
        event_type: Some("pull_request".to_string()),
        ..Default::default()
    };

    let result = adapter.list_payloads(filters).await;

    assert!(result.is_ok());
    let refs = result.unwrap();
    assert_eq!(refs.len(), 3);
}

/// Test listing with no results
#[tokio::test]
async fn test_storage_adapter_list_empty() {
    let mock_storage = Arc::new(MockBlobStorage::new());
    let adapter = BlobStorageAdapter::new(mock_storage);

    let filters = PayloadFilters::default();
    let result = adapter.list_payloads(filters).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

/// Test error mapping from blob storage errors
#[tokio::test]
async fn test_storage_adapter_error_mapping() {
    let mock_storage = Arc::new(MockBlobStorage::with_failure());
    let adapter = BlobStorageAdapter::new(mock_storage);

    let request = create_test_webhook_request();
    let result = adapter
        .store_payload(&request, ValidationStatus::Valid)
        .await;

    assert!(result.is_err());
    // Error should be mapped to StorageError
    assert!(matches!(
        result.unwrap_err(),
        StorageError::OperationFailed { .. }
    ));
}
