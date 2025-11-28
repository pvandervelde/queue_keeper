//! Tests for blob storage interface

use super::*;

// ============================================================================
// Type Tests
// ============================================================================

#[test]
fn test_webhook_payload_creation() {
    let payload = WebhookPayload {
        body: Bytes::from("test body"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: EventId::new(),
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: Some("test-123".to_string()),
        },
    };

    assert_eq!(payload.body.as_ref(), b"test body");
    assert_eq!(payload.metadata.event_type, "pull_request");
    assert!(payload.metadata.signature_valid);
}

#[test]
fn test_blob_metadata_creation() {
    let event_id = EventId::new();
    let metadata = BlobMetadata {
        event_id: event_id.clone(),
        blob_path: "test/path.json".to_string(),
        size_bytes: 1024,
        content_type: "application/json".to_string(),
        created_at: Timestamp::now(),
        metadata: PayloadMetadata {
            event_id: event_id.clone(),
            event_type: "issues".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    assert_eq!(metadata.size_bytes, 1024);
    assert_eq!(metadata.content_type, "application/json");
}

#[test]
fn test_stored_webhook_structure() {
    let event_id = EventId::new();
    let payload = WebhookPayload {
        body: Bytes::from("{}"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event_id.clone(),
            event_type: "push".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    let stored = StoredWebhook {
        metadata: BlobMetadata {
            event_id: event_id.clone(),
            blob_path: "test/path.json".to_string(),
            size_bytes: 2,
            content_type: "application/json".to_string(),
            created_at: Timestamp::now(),
            metadata: payload.metadata.clone(),
        },
        payload: payload.clone(),
    };

    assert_eq!(stored.metadata.event_id, event_id);
    assert_eq!(stored.payload.metadata.event_type, "push");
}

#[test]
fn test_payload_filter_default() {
    let filter = PayloadFilter::default();

    assert!(filter.date_range.is_none());
    assert!(filter.repository.is_none());
    assert!(filter.event_type.is_none());
    assert!(filter.limit.is_none());
    assert!(filter.offset.is_none());
}

#[test]
fn test_payload_filter_with_values() {
    let filter = PayloadFilter {
        date_range: Some(DateRange {
            start: Timestamp::now(),
            end: Timestamp::now(),
        }),
        repository: Some("owner/repo".to_string()),
        event_type: Some("pull_request".to_string()),
        limit: Some(100),
        offset: Some(50),
    };

    assert!(filter.date_range.is_some());
    assert_eq!(filter.repository, Some("owner/repo".to_string()));
    assert_eq!(filter.event_type, Some("pull_request".to_string()));
    assert_eq!(filter.limit, Some(100));
    assert_eq!(filter.offset, Some(50));
}

// ============================================================================
// Error Classification Tests
// ============================================================================

#[test]
fn test_connection_failed_is_transient() {
    let error = BlobStorageError::ConnectionFailed {
        message: "Network error".to_string(),
    };

    assert!(error.is_transient());
}

#[test]
fn test_timeout_is_transient() {
    let error = BlobStorageError::Timeout { timeout_ms: 5000 };

    assert!(error.is_transient());
}

#[test]
fn test_internal_error_is_transient() {
    let error = BlobStorageError::InternalError {
        message: "Temporary failure".to_string(),
    };

    assert!(error.is_transient());
}

#[test]
fn test_authentication_failed_is_permanent() {
    let error = BlobStorageError::AuthenticationFailed {
        message: "Invalid credentials".to_string(),
    };

    assert!(!error.is_transient());
}

#[test]
fn test_blob_not_found_is_permanent() {
    let error = BlobStorageError::BlobNotFound {
        event_id: EventId::new(),
    };

    assert!(!error.is_transient());
}

#[test]
fn test_permission_denied_is_permanent() {
    let error = BlobStorageError::PermissionDenied {
        operation: "store".to_string(),
    };

    assert!(!error.is_transient());
}

#[test]
fn test_quota_exceeded_is_permanent() {
    let error = BlobStorageError::QuotaExceeded;

    assert!(!error.is_transient());
}

#[test]
fn test_invalid_path_is_permanent() {
    let error = BlobStorageError::InvalidPath {
        path: "../etc/passwd".to_string(),
    };

    assert!(!error.is_transient());
}

#[test]
fn test_serialization_failed_is_permanent() {
    let error = BlobStorageError::SerializationFailed {
        message: "Invalid JSON".to_string(),
    };

    assert!(!error.is_transient());
}

// ============================================================================
// Path Generation Tests
// ============================================================================

#[test]
fn test_blob_path_follows_convention() {
    let event_id = EventId::new();
    let path = event_id.to_blob_path();

    // Verify path structure
    assert!(path.starts_with("webhook-payloads/year="));
    assert!(path.contains("/month="));
    assert!(path.contains("/day="));
    assert!(path.contains("/hour="));
    assert!(path.ends_with(".json"));

    // Verify path contains event ID
    let event_id_str = event_id.to_string();
    assert!(path.contains(&event_id_str));
}

#[test]
fn test_blob_path_is_stable() {
    let event_id = EventId::new();
    let path1 = event_id.to_blob_path();
    let path2 = event_id.to_blob_path();

    // Same event ID should generate same path
    assert_eq!(path1, path2);
}

#[test]
fn test_blob_path_format() {
    let event_id = EventId::new();
    let path = event_id.to_blob_path();

    // Verify format matches spec
    let parts: Vec<&str> = path.split('/').collect();
    assert_eq!(parts[0], "webhook-payloads");
    assert!(parts[1].starts_with("year="));
    assert!(parts[2].starts_with("month="));
    assert!(parts[3].starts_with("day="));
    assert!(parts[4].starts_with("hour="));
    assert!(parts[5].ends_with(".json"));
}

// ============================================================================
// Health Status Tests
// ============================================================================

#[test]
fn test_storage_health_status_healthy() {
    let status = StorageHealthStatus {
        healthy: true,
        connected: true,
        last_success: Some(Timestamp::now()),
        error_message: None,
        metrics: StorageMetrics {
            avg_write_latency_ms: 50.0,
            avg_read_latency_ms: 30.0,
            success_rate: 1.0,
        },
    };

    assert!(status.healthy);
    assert!(status.connected);
    assert!(status.last_success.is_some());
    assert!(status.error_message.is_none());
    assert_eq!(status.metrics.success_rate, 1.0);
}

#[test]
fn test_storage_health_status_unhealthy() {
    let status = StorageHealthStatus {
        healthy: false,
        connected: false,
        last_success: None,
        error_message: Some("Connection timeout".to_string()),
        metrics: StorageMetrics {
            avg_write_latency_ms: 0.0,
            avg_read_latency_ms: 0.0,
            success_rate: 0.0,
        },
    };

    assert!(!status.healthy);
    assert!(!status.connected);
    assert!(status.last_success.is_none());
    assert_eq!(status.error_message, Some("Connection timeout".to_string()));
    assert_eq!(status.metrics.success_rate, 0.0);
}

#[test]
fn test_storage_metrics_values() {
    let metrics = StorageMetrics {
        avg_write_latency_ms: 125.5,
        avg_read_latency_ms: 75.3,
        success_rate: 0.98,
    };

    assert_eq!(metrics.avg_write_latency_ms, 125.5);
    assert_eq!(metrics.avg_read_latency_ms, 75.3);
    assert_eq!(metrics.success_rate, 0.98);
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_webhook_payload_serialization() {
    let payload = WebhookPayload {
        body: Bytes::from("test"),
        headers: {
            let mut h = HashMap::new();
            h.insert("X-GitHub-Event".to_string(), "push".to_string());
            h
        },
        metadata: PayloadMetadata {
            event_id: EventId::new(),
            event_type: "push".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: Some("abc-123".to_string()),
        },
    };

    // Test round-trip serialization
    let json = serde_json::to_string(&payload).expect("Serialization failed");
    let deserialized: WebhookPayload = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(payload.body, deserialized.body);
    assert_eq!(payload.headers, deserialized.headers);
    assert_eq!(
        payload.metadata.event_type,
        deserialized.metadata.event_type
    );
}

#[test]
fn test_blob_metadata_serialization() {
    let event_id = EventId::new();
    let metadata = BlobMetadata {
        event_id: event_id.clone(),
        blob_path: "test/path.json".to_string(),
        size_bytes: 2048,
        content_type: "application/json".to_string(),
        created_at: Timestamp::now(),
        metadata: PayloadMetadata {
            event_id: event_id.clone(),
            event_type: "pull_request".to_string(),
            repository: None,
            signature_valid: true,
            received_at: Timestamp::now(),
            delivery_id: None,
        },
    };

    // Test round-trip serialization
    let json = serde_json::to_string(&metadata).expect("Serialization failed");
    let deserialized: BlobMetadata = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(metadata.blob_path, deserialized.blob_path);
    assert_eq!(metadata.size_bytes, deserialized.size_bytes);
    assert_eq!(metadata.content_type, deserialized.content_type);
}

#[test]
fn test_stored_webhook_serialization() {
    let event_id = EventId::new();
    let payload = WebhookPayload {
        body: Bytes::from("{}"),
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event_id.clone(),
            event_type: "issues".to_string(),
            repository: None,
            signature_valid: false,
            received_at: Timestamp::now(),
            delivery_id: Some("delivery-456".to_string()),
        },
    };

    let stored = StoredWebhook {
        metadata: BlobMetadata {
            event_id: event_id.clone(),
            blob_path: "path/to/blob.json".to_string(),
            size_bytes: 2,
            content_type: "application/json".to_string(),
            created_at: Timestamp::now(),
            metadata: payload.metadata.clone(),
        },
        payload: payload.clone(),
    };

    // Test round-trip serialization
    let json = serde_json::to_string(&stored).expect("Serialization failed");
    let deserialized: StoredWebhook = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(stored.metadata.blob_path, deserialized.metadata.blob_path);
    assert_eq!(stored.payload.body, deserialized.payload.body);
}
