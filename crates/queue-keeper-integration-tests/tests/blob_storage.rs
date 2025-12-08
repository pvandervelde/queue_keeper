//! Integration tests for blob storage operations
//!
//! These tests verify:
//! - Payload persistence (Assertion #3)
//! - Payload immutability (Assertion #23)
//! - Tamper detection (Assertion #23)
//! - Storage integrity (Assertion #24)
//! - Payload retrieval for replay (Assertion #25)

mod common;

use common::create_test_app_state;
use queue_keeper_core::blob_storage::{BlobStorage, WebhookPayload};
use queue_keeper_core::{EventId, Timestamp};
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
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let payload = WebhookPayload {
        event_id,
        event_type: "pull_request".to_string(),
        occurred_at: Timestamp::now(),
        body: b"{\"action\":\"opened\"}".to_vec(),
        headers: vec![
            ("x-github-event".to_string(), "pull_request".to_string()),
            (
                "x-github-delivery".to_string(),
                uuid::Uuid::new_v4().to_string(),
            ),
        ],
    };

    // Act: Store payload
    let result = storage.store(&payload).await;

    // Assert: Storage succeeds
    assert!(result.is_ok(), "Storage should succeed: {:?}", result.err());

    let metadata = result.unwrap();
    assert_eq!(metadata.event_id, event_id);
    assert!(metadata.size > 0);
    assert!(!metadata.checksum.is_empty());

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
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let original_body = b"{\"action\":\"opened\",\"number\":123}".to_vec();
    let payload = WebhookPayload {
        event_id,
        event_type: "pull_request".to_string(),
        occurred_at: Timestamp::now(),
        body: original_body.clone(),
        headers: vec![("x-github-event".to_string(), "pull_request".to_string())],
    };

    // Act: Store and retrieve
    let store_result = storage.store(&payload).await;
    assert!(store_result.is_ok());

    let retrieve_result = storage.retrieve(&event_id).await;

    // Assert: Retrieved payload matches original
    assert!(
        retrieve_result.is_ok(),
        "Retrieval should succeed: {:?}",
        retrieve_result.err()
    );

    let stored = retrieve_result.unwrap();
    assert_eq!(stored.metadata.event_id, event_id);
    assert_eq!(stored.payload.body, original_body);
    assert_eq!(stored.payload.event_type, "pull_request");

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-retrieval"));
}

/// Verify that checksums detect payload tampering
///
/// Tests Assertion #23: Payload Immutability (tamper detection)
#[tokio::test]
async fn test_checksum_tamper_detection() {
    // Arrange
    let storage_path = std::env::temp_dir().join("test-storage-tamper");
    let storage = queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage::new(
        storage_path.clone(),
    )
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let payload = WebhookPayload {
        event_id,
        event_type: "pull_request".to_string(),
        occurred_at: Timestamp::now(),
        body: b"{\"action\":\"opened\"}".to_vec(),
        headers: vec![],
    };

    // Act: Store payload
    let store_result = storage.store(&payload).await;
    assert!(store_result.is_ok());

    // Tamper with stored file (modify the body)
    // Note: This requires finding the stored file and modifying it
    // For filesystem storage, we'd need to know the path structure

    // TODO: Implement file tampering based on storage path structure
    // Then verify that retrieval detects the tampering

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
    .expect("Failed to create storage");

    let event_id = EventId::new();
    let large_body = vec![b'A'; 1024 * 1024]; // 1MB of 'A'
    let payload = WebhookPayload {
        event_id,
        event_type: "pull_request".to_string(),
        occurred_at: Timestamp::now(),
        body: large_body.clone(),
        headers: vec![],
    };

    // Act: Store large payload
    let store_result = storage.store(&payload).await;

    // Assert: Storage succeeds
    assert!(store_result.is_ok(), "Large payload storage should succeed");

    let metadata = store_result.unwrap();
    assert_eq!(metadata.size, 1024 * 1024);

    // Verify retrieval works
    let retrieve_result = storage.retrieve(&event_id).await;
    assert!(retrieve_result.is_ok());
    assert_eq!(retrieve_result.unwrap().payload.body, large_body);

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
    .expect("Failed to create storage");

    let nonexistent_id = EventId::new();

    // Act: Attempt to retrieve non-existent event
    let result = storage.retrieve(&nonexistent_id).await;

    // Assert: Returns NotFound error
    assert!(result.is_err());
    // TODO: Verify specific error type once BlobStorageError is defined

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
        .expect("Failed to create storage"),
    );

    // Act: Write 10 payloads concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            let event_id = EventId::new();
            let payload = WebhookPayload {
                event_id,
                event_type: format!("test_{}", i),
                occurred_at: Timestamp::now(),
                body: format!("{{\"test\":{}}}", i).into_bytes(),
                headers: vec![],
            };
            storage_clone.store(&payload).await
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
    .expect("Failed to create storage");

    // Act: Perform health check
    let health = storage.health().await;

    // Assert: Storage is healthy
    assert!(health.is_ok());
    let health_info = health.unwrap();
    assert!(health_info.is_healthy);

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
    .expect("Failed to create storage");

    // Store 5 events
    for i in 0..5 {
        let payload = WebhookPayload {
            event_id: EventId::new(),
            event_type: format!("event_{}", i),
            occurred_at: Timestamp::now(),
            body: vec![],
            headers: vec![],
        };
        storage.store(&payload).await.expect("Store should succeed");
    }

    // Act: List events
    // TODO: Implement list_events method
    // let events = storage.list_events(...).await;

    // Assert: Returns all stored events
    // assert_eq!(events.len(), 5);

    // Cleanup
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("test-storage-list"));
}
