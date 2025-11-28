//! Tests for filesystem blob storage adapter

use super::*;
use crate::{blob_storage::PayloadMetadata, Timestamp};
use bytes::Bytes;
use std::collections::HashMap;
use tempfile::TempDir;

// ============================================================================
// Construction Tests
// ============================================================================

#[tokio::test]
async fn test_filesystem_storage_creation() {
    let temp_dir = TempDir::new().unwrap();
    let _storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .expect("Failed to create storage");

    // Verify base path is accessible
    assert!(temp_dir.path().exists());
}

#[tokio::test]
async fn test_filesystem_storage_creates_directory() {
    let temp_dir = TempDir::new().unwrap();
    let storage_path = temp_dir.path().join("new_dir");

    let _storage = FilesystemBlobStorage::new(storage_path.clone())
        .await
        .expect("Failed to create storage");

    // Verify directory was created
    assert!(storage_path.exists());
}

// ============================================================================
// Store Operation Tests (will fail until Phase 2)
// ============================================================================

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_store_creates_directory_structure() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let payload = WebhookPayload {
        body: Bytes::from("test payload"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event_id.clone(),
            event_type: "push".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: Some("test-123".to_string()),
        },
    };

    let _metadata = storage.store_payload(&event_id, &payload).await.unwrap();
}

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_store_writes_json() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let payload = WebhookPayload {
        body: Bytes::from("{}"),
        headers: {
            let mut h = HashMap::new();
            h.insert("X-GitHub-Event".to_string(), "pull_request".to_string());
            h
        },
        metadata: PayloadMetadata {
            event_id: event_id.clone(),
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    let _metadata = storage.store_payload(&event_id, &payload).await.unwrap();
}

// ============================================================================
// Get Operation Tests (will fail until Phase 2)
// ============================================================================

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_get_reads_stored_payload() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let _result = storage.get_payload(&event_id).await.unwrap();
}

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_get_returns_none_for_missing() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let _result = storage.get_payload(&event_id).await.unwrap();
}

// ============================================================================
// List Operation Tests (will fail until Phase 2)
// ============================================================================

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_list_filters_by_date() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let filter = PayloadFilter {
        date_range: Some(DateRange {
            start: Timestamp::now(),
            end: Timestamp::now(),
        }),
        ..Default::default()
    };

    let _results = storage.list_payloads(&filter).await.unwrap();
}

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_list_filters_by_repository() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let filter = PayloadFilter {
        repository: Some("owner/repo".to_string()),
        ..Default::default()
    };

    let _results = storage.list_payloads(&filter).await.unwrap();
}

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_list_filters_by_event_type() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let filter = PayloadFilter {
        event_type: Some("pull_request".to_string()),
        ..Default::default()
    };

    let _results = storage.list_payloads(&filter).await.unwrap();
}

// ============================================================================
// Delete Operation Tests (will fail until Phase 2)
// ============================================================================

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_delete_removes_file() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let _result = storage.delete_payload(&event_id).await;
}

// ============================================================================
// Health Check Tests (will fail until Phase 2)
// ============================================================================

#[tokio::test]
#[should_panic(expected = "not yet implemented")]
async fn test_filesystem_health_check_verifies_directory() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let _status = storage.health_check().await.unwrap();
}

// ============================================================================
// Path Generation Tests
// ============================================================================

#[test]
fn test_get_blob_path_combines_base_and_relative() {
    let base_path = PathBuf::from("/tmp/blobs");
    let storage = FilesystemBlobStorage {
        base_path: base_path.clone(),
    };

    let event_id = EventId::new();
    let full_path = storage.get_blob_path(&event_id);

    // Verify path starts with base
    assert!(full_path.starts_with(&base_path));

    // Verify path contains webhook-payloads
    assert!(full_path.to_string_lossy().contains("webhook-payloads"));
}
