//! Integration tests for blob storage operations
//!
//! These tests verify:
//! - Payload persistence (Assertion #3)
//! - Payload immutability (Assertion #23)
//! - Tamper detection (Assertion #23)
//! - Storage integrity (Assertion #24)
//! - Payload retrieval for replay (Assertion #25)

mod common;

use bytes::Bytes;
use queue_keeper_core::blob_storage::{BlobStorage, PayloadMetadata, WebhookPayload};
use queue_keeper_core::{EventId, Timestamp};
use std::collections::HashMap;
use std::sync::Arc;

/// Verify that webhook payloads are persisted to storage
///
/// Tests Assertion #3: Payload Persistence
#[tokio::test]
async fn test_webhook_payload_persistence() {
    // Arrange
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        std::env::temp_dir().join("test-storage-persistence"),
    )
    .await
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let mut headers = HashMap::new();
    headers.insert("x-github-event".to_string(), "pull_request".to_string());
    headers.insert(
        "x-github-delivery".to_string(),
        uuid::Uuid::new_v4().to_string(),
    );

    let payload = WebhookPayload {
        body: Bytes::from("{\"action\":\"opened\"}"),
        headers,
        metadata: PayloadMetadata {
            event_id,
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: Some(uuid::Uuid::new_v4().to_string()),
        },
    };

    // Act: Store payload
    let result = storage.store_payload(&event_id, &payload).await;

    // Assert: Storage succeeds
    assert!(result.is_ok(), "Storage should succeed: {:?}", result.err());

    let metadata = result.unwrap();
    assert_eq!(metadata.event_id, event_id);
    assert!(metadata.size_bytes > 0);
    assert!(!metadata.checksum_sha256.is_empty());

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-persistence"));
}

/// Verify that stored payloads can be retrieved
///
/// Tests Assertion #25: Replay Idempotency (payload retrieval)
#[tokio::test]
async fn test_payload_retrieval() {
    // Arrange
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        std::env::temp_dir().join("test-storage-retrieval"),
    )
    .await
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let original_body = Bytes::from("{\"action\":\"opened\",\"number\":123}");
    let mut headers = HashMap::new();
    headers.insert("x-github-event".to_string(), "pull_request".to_string());

    let payload = WebhookPayload {
        body: original_body.clone(),
        headers,
        metadata: PayloadMetadata {
            event_id,
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    // Act: Store and retrieve
    let store_result = storage.store_payload(&event_id, &payload).await;
    assert!(store_result.is_ok());

    let retrieve_result = storage.get_payload(&event_id).await;

    // Assert: Retrieved payload matches original
    assert!(
        retrieve_result.is_ok(),
        "Retrieval should succeed: {:?}",
        retrieve_result.err()
    );

    let stored = retrieve_result.unwrap().expect("Payload should exist");
    assert_eq!(stored.metadata.event_id, event_id);
    assert_eq!(stored.payload.body, original_body);
    assert_eq!(stored.payload.metadata.event_type, "pull_request");

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-retrieval"));
}

/// Verify that checksums detect payload tampering
///
/// Tests Assertion #23: Payload Immutability (tamper detection)
///
/// Stores a payload, then directly modifies the stored JSON file on disk to
/// simulate tampering, and confirms that the next retrieval returns a
/// `BlobStorageError::ChecksumMismatch`.
#[tokio::test]
async fn test_checksum_tamper_detection() {
    use queue_keeper_core::blob_storage::BlobStorageError;
    use std::io::Write;

    // Arrange
    let storage_path = std::env::temp_dir().join("test-storage-tamper");
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        storage_path.clone(),
    )
    .await
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let original_body = Bytes::from("{\"action\":\"opened\"}");
    let payload = WebhookPayload {
        body: original_body.clone(),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id,
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    // Act: Store payload
    let store_result = storage.store_payload(&event_id, &payload).await;
    assert!(store_result.is_ok(), "Store should succeed: {:?}", store_result.err());
    let blob_meta = store_result.unwrap();

    // Tamper: read the JSON file on disk and replace the stored body bytes
    // with different content (without updating the checksum).
    let blob_file = storage_path.join(&blob_meta.blob_path);
    assert!(blob_file.exists(), "Blob file must exist at {:?}", blob_file);

    let raw_json = std::fs::read_to_string(&blob_file)
        .expect("Should be able to read blob file");

    // Parse, mutate, reserialise — replacing the body but leaving the checksum intact
    let mut stored_value: serde_json::Value =
        serde_json::from_str(&raw_json).expect("Blob should be valid JSON");

    // The body is stored as a byte array (via bytes_serde). Replace it with
    // different bytes to break the checksum.
    stored_value["payload"]["body"] = serde_json::json!([1u8, 2u8, 3u8, 4u8]);

    let tampered_json = serde_json::to_string_pretty(&stored_value)
        .expect("Re-serialisation should succeed");

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&blob_file)
        .expect("Should be able to open blob file for writing");
    file.write_all(tampered_json.as_bytes())
        .expect("Should be able to write tampered content");
    drop(file);

    // Assert: retrieval detects the tampering via checksum mismatch
    let retrieve_result = storage.get_payload(&event_id).await;

    assert!(
        retrieve_result.is_err(),
        "Retrieval of tampered blob must return an error"
    );
    assert!(
        matches!(
            retrieve_result.unwrap_err(),
            BlobStorageError::ChecksumMismatch { .. }
        ),
        "Error must be ChecksumMismatch"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(storage_path);
}

/// Verify that storage handles large payloads
///
/// Tests edge case: Large Payloads
#[tokio::test]
async fn test_large_payload_storage() {
    // Arrange: Create 1MB payload
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        std::env::temp_dir().join("test-storage-large"),
    )
    .await
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let large_body = Bytes::from(vec![b'A'; 1024 * 1024]); // 1MB of 'A'
    let payload = WebhookPayload {
        body: large_body.clone(),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id,
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    // Act: Store large payload
    let store_result = storage.store_payload(&event_id, &payload).await;

    // Assert: Storage succeeds
    assert!(store_result.is_ok(), "Large payload storage should succeed");

    let metadata = store_result.unwrap();
    // Size should be at least the payload size (1MB), plus overhead for JSON structure
    assert!(
        metadata.size_bytes >= 1024 * 1024,
        "Stored size should be at least payload size"
    );

    // Verify retrieval works
    let retrieve_result = storage.get_payload(&event_id).await;
    assert!(retrieve_result.is_ok());
    let stored = retrieve_result.unwrap().expect("Payload should exist");
    assert_eq!(stored.payload.body, large_body);

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-large"));
}

/// Verify that storage gracefully handles non-existent events
///
/// Tests error handling
#[tokio::test]
async fn test_retrieve_nonexistent_event() {
    // Arrange
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        std::env::temp_dir().join("test-storage-nonexistent"),
    )
    .await
    .expect("Failed to create storage");

    let nonexistent_id = EventId::new();

    // Act: Attempt to retrieve non-existent event
    let result = storage.get_payload(&nonexistent_id).await;

    // Assert: Returns Ok(None) for nonexistent event
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-nonexistent"));
}

/// Verify that storage handles concurrent writes
///
/// Tests concurrency safety
#[tokio::test]
async fn test_concurrent_storage_writes() {
    // Arrange
    let storage = Arc::new(
        queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
            std::env::temp_dir().join("test-storage-concurrent"),
        )
        .await
        .expect("Failed to create storage"),
    );

    // Act: Write 10 payloads concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            let event_id = EventId::new();
            let payload = WebhookPayload {
                body: Bytes::from(format!("{{\"test\":{}}}", i)),
                headers: HashMap::new(),
                metadata: PayloadMetadata {
                    event_id,
                    event_type: format!("test_{}", i),
                    repository: None,
                    signature_valid: true,
                    received_at: Timestamp::now(),
                    delivery_id: None,
                },
            };
            storage_clone.store_payload(&event_id, &payload).await
        });
        handles.push(handle);
    }

    // Assert: All writes succeed
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        assert!(result.is_ok(), "Concurrent write should succeed");
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-concurrent"));
}

/// Verify that storage health check works
///
/// Tests monitoring integration
#[tokio::test]
async fn test_storage_health_check() {
    // Arrange
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        std::env::temp_dir().join("test-storage-health"),
    )
    .await
    .expect("Failed to create storage");

    // Act: Perform health check
    let health = storage.health_check().await;

    // Assert: Storage is healthy
    assert!(health.is_ok());
    let health_info = health.unwrap();
    assert!(health_info.healthy);
    assert!(health_info.connected);

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-health"));
}

/// Verify that storage can list stored events
///
/// Tests query capability for admin operations
#[tokio::test]
#[ignore = "Requires list_events implementation"]
async fn test_list_stored_events() {
    // Arrange: Store multiple events
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        std::env::temp_dir().join("test-storage-list"),
    )
    .await
    .expect("Failed to create storage");

    // Store 5 events
    for i in 0..5 {
        let event_id = EventId::new();
        let payload = WebhookPayload {
            body: Bytes::from(""),
            headers: HashMap::new(),
            metadata: PayloadMetadata {
                event_id,
                event_type: format!("event_{}", i),
                repository: None,
                signature_valid: true,
                received_at: Timestamp::now(),
                delivery_id: None,
            },
        };
        storage
            .store_payload(&event_id, &payload)
            .await
            .expect("Store should succeed");
    }

    // Act: List events
    // TODO: Implement list_events method
    // let events = storage.list_events(...).await;

    // Assert: Returns all stored events
    // assert_eq!(events.len(), 5);

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-list"));
}
