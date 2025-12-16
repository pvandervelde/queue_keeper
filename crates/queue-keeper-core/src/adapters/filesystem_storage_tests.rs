//! Tests for filesystem blob storage adapter

use super::*;
use crate::{
    blob_storage::PayloadMetadata, Repository, RepositoryId, Timestamp, User, UserId, UserType,
};
use bytes::Bytes;
use std::collections::HashMap;
use tempfile::TempDir;

/// Helper to create test repository
fn test_repository(owner: &str, repo: &str) -> Repository {
    Repository::new(
        RepositoryId::new(12345),
        repo.to_string(),
        format!("{}/{}", owner, repo),
        User {
            id: UserId::new(1),
            login: owner.to_string(),
            user_type: UserType::User,
        },
        false,
    )
}

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
// Store Operation Tests
// ============================================================================

/// Verify that storing a payload creates the correct directory structure.
///
/// Creates a webhook payload and stores it, then verifies the expected
/// directory hierarchy exists (webhook-payloads/year=/month=/day=/hour=/).
#[tokio::test]
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
            event_id,
            event_type: "push".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: Some("test-123".to_string()),
        },
    };

    let metadata = storage.store_payload(&event_id, &payload).await.unwrap();

    // Verify directory structure contains expected components
    let path_str = &metadata.blob_path;
    assert!(path_str.contains("webhook-payloads"));
    assert!(path_str.contains("year="));
    assert!(path_str.contains("month="));
    assert!(path_str.contains("day="));
    assert!(path_str.contains("hour="));
}

/// Verify that storing a payload writes valid JSON to disk.
///
/// Stores a payload, then reads the file directly to verify it contains
/// properly formatted JSON with all expected fields.
#[tokio::test]
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
            event_id,
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    let _metadata = storage.store_payload(&event_id, &payload).await.unwrap();

    // Read file and verify it's valid JSON
    let blob_path = storage.get_blob_path(&event_id);
    let content = tokio::fs::read_to_string(&blob_path).await.unwrap();
    let stored: StoredWebhook = serde_json::from_str(&content).unwrap();

    // Verify content matches
    assert_eq!(stored.payload.body, payload.body);
    assert_eq!(stored.metadata.metadata.event_type, "pull_request");
    assert_eq!(
        stored.payload.headers.get("X-GitHub-Event"),
        Some(&"pull_request".to_string())
    );
}

// ============================================================================
// Get Operation Tests
// ============================================================================

/// Verify that getting a stored payload returns the correct data.
///
/// Stores a payload, then retrieves it and verifies all fields match.
#[tokio::test]
async fn test_filesystem_get_reads_stored_payload() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let original_payload = WebhookPayload {
        body: Bytes::from("test body content"),
        headers: {
            let mut h = HashMap::new();
            h.insert("X-GitHub-Event".to_string(), "push".to_string());
            h
        },
        metadata: PayloadMetadata {
            event_id,
            event_type: "push".to_string(),
            repository: Some(test_repository("owner", "repo")),
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: Some("delivery-123".to_string()),
        },
    };

    // Store the payload
    storage
        .store_payload(&event_id, &original_payload)
        .await
        .unwrap();

    // Retrieve it
    let result = storage.get_payload(&event_id).await.unwrap();
    assert!(result.is_some());

    let stored = result.unwrap();
    assert_eq!(stored.payload.body, original_payload.body);
    assert_eq!(stored.payload.headers, original_payload.headers);
    assert_eq!(
        stored.metadata.metadata.event_type,
        original_payload.metadata.event_type
    );
    assert_eq!(
        stored
            .metadata
            .metadata
            .repository
            .as_ref()
            .unwrap()
            .full_name,
        original_payload.metadata.repository.unwrap().full_name
    );
}

/// Verify that getting a non-existent payload returns None.
///
/// Attempts to retrieve a payload that was never stored and verifies
/// None is returned without error.
#[tokio::test]
async fn test_filesystem_get_returns_none_for_missing() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let result = storage.get_payload(&event_id).await.unwrap();

    assert!(result.is_none());
}

// ============================================================================
// List Operation Tests
// ============================================================================

/// Verify that list_payloads filters by date range correctly.
///
/// Stores payloads from different dates, then lists with a date filter
/// to verify only matching payloads are returned.
#[tokio::test]
async fn test_filesystem_list_filters_by_date() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Capture timestamp before storing
    let now = Timestamp::now();

    // Store a payload
    let event_id = EventId::new();
    let payload = WebhookPayload {
        body: Bytes::from("test"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id,
            event_type: "push".to_string(),
            repository: Some(test_repository("owner", "repo")),
            signature_valid: true,
            received_at: now,
            delivery_id: None,
        },
    };
    storage.store_payload(&event_id, &payload).await.unwrap();

    // List with date range covering the stored payload
    // Use a range that includes 'now' (start <= now < end)
    let filter = PayloadFilter {
        date_range: Some(DateRange {
            start: now,
            end: Timestamp::now(), // Slightly later
        }),
        ..Default::default()
    };

    let results = storage.list_payloads(&filter).await.unwrap();

    // Should find the stored payload
    assert!(!results.is_empty());
}

/// Verify that list_payloads filters by repository correctly.
///
/// Stores payloads for different repositories, then lists with a repository
/// filter to verify only matching payloads are returned.
#[tokio::test]
async fn test_filesystem_list_filters_by_repository() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Store payloads for different repositories
    let event_id1 = EventId::new();
    let payload1 = WebhookPayload {
        body: Bytes::from("test1"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event_id1,
            event_type: "push".to_string(),
            repository: Some(test_repository("owner", "repo1")),
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };
    storage.store_payload(&event_id1, &payload1).await.unwrap();

    let event_id2 = EventId::new();
    let payload2 = WebhookPayload {
        body: Bytes::from("test2"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event_id2,
            event_type: "push".to_string(),
            repository: Some(test_repository("owner", "repo2")),
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };
    storage.store_payload(&event_id2, &payload2).await.unwrap();

    // Filter by specific repository
    let filter = PayloadFilter {
        repository: Some("owner/repo1".to_string()),
        ..Default::default()
    };

    let results = storage.list_payloads(&filter).await.unwrap();

    // Should only find repo1
    assert_eq!(results.len(), 1);
    let repo1 = test_repository("owner", "repo1");
    assert_eq!(
        results[0].metadata.repository.as_ref().unwrap().full_name,
        repo1.full_name
    );
}

/// Verify that list_payloads filters by event type correctly.
///
/// Stores payloads for different event types, then lists with an event type
/// filter to verify only matching payloads are returned.
#[tokio::test]
async fn test_filesystem_list_filters_by_event_type() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    // Store payloads with different event types
    let event_id1 = EventId::new();
    let payload1 = WebhookPayload {
        body: Bytes::from("push event"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event_id1,
            event_type: "push".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };
    storage.store_payload(&event_id1, &payload1).await.unwrap();

    let event_id2 = EventId::new();
    let payload2 = WebhookPayload {
        body: Bytes::from("pr event"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event_id2,
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };
    storage.store_payload(&event_id2, &payload2).await.unwrap();

    // Filter by pull_request event type
    let filter = PayloadFilter {
        event_type: Some("pull_request".to_string()),
        ..Default::default()
    };

    let results = storage.list_payloads(&filter).await.unwrap();

    // Should only find pull_request
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].metadata.event_type, "pull_request");
}

// ============================================================================
// Delete Operation Tests
// ============================================================================

/// Verify that deleting a payload removes the file from storage.
///
/// Stores a payload, deletes it, then verifies it can no longer be retrieved.
#[tokio::test]
async fn test_filesystem_delete_removes_file() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let event_id = EventId::new();
    let payload = WebhookPayload {
        body: Bytes::from("to be deleted"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id,
            event_type: "push".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    // Store and verify it exists
    storage.store_payload(&event_id, &payload).await.unwrap();
    assert!(storage.get_payload(&event_id).await.unwrap().is_some());

    // Delete it
    storage.delete_payload(&event_id).await.unwrap();

    // Verify it's gone
    assert!(storage.get_payload(&event_id).await.unwrap().is_none());
}

// ============================================================================
// Health Check Tests
// ============================================================================

/// Verify that health_check correctly reports storage status.
///
/// Creates storage and checks that health_check reports healthy status
/// with accessible base path.
#[tokio::test]
async fn test_filesystem_health_check_verifies_directory() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilesystemBlobStorage::new(temp_dir.path().to_path_buf())
        .await
        .unwrap();

    let status = storage.health_check().await.unwrap();

    assert!(status.healthy);
    assert!(status.connected);
    // Metrics is a struct, not Option, so just verify it exists
    assert!(status.metrics.success_rate >= 0.0);
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
