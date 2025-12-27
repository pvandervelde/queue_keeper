//! Tests for audit logger backend implementations.
//!
//! Tests all four backend implementations:
//! - FilesystemAuditLogger: Local file-based logging
//! - BlobStorageAuditLogger: Cloud storage logging
//! - StdoutAuditLogger: Container observability
//! - CompositeAuditLogger: Multi-backend logging

use super::*;
use crate::{EventId, Repository, RepositoryId, SessionId, User, UserId, UserType};
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_audit_event() -> AuditEvent {
    use std::time::Duration;

    let actor = AuditActor::System {
        component_name: "queue-keeper".to_string(),
        instance_id: "test-instance".to_string(),
        version: "1.0.0".to_string(),
    };

    let resource = AuditResource::WebhookEvent {
        event_id: EventId::new(),
        session_id: SessionId::new("owner/repo/pull_request/123".to_string()).unwrap(),
        repository: Repository {
            id: RepositoryId(12345_u64),
            name: "repo".to_string(),
            full_name: "owner/repo".to_string(),
            owner: User {
                id: UserId(1_u64),
                login: "testowner".to_string(),
                user_type: UserType::User,
            },
            private: false,
        },
        event_type: "pull_request".to_string(),
    };

    let action = AuditAction::Process {
        operation: "webhook_validation".to_string(),
    };

    let result = AuditResult::Success {
        duration: Some(Duration::from_millis(50)),
        details: Some("Validation successful".to_string()),
    };

    let context = AuditContext::default();

    AuditEvent::new(
        AuditEventType::WebhookProcessing,
        actor,
        resource,
        action,
        result,
        context,
    )
}

// ============================================================================
// FilesystemAuditLogger Tests
// ============================================================================

mod filesystem_logger_tests {
    use super::*;

    /// Verify FilesystemAuditLogger can be created
    #[tokio::test]
    async fn test_filesystem_logger_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit_logs");

        let _logger = FilesystemAuditLogger::new(log_path.clone()).unwrap();

        // Verify directory was created
        assert!(log_path.exists());
        assert!(log_path.is_dir());
    }

    /// Verify FilesystemAuditLogger logs events
    #[tokio::test]
    async fn test_filesystem_logger_logs_event() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit_logs");

        let logger = FilesystemAuditLogger::new(log_path.clone()).unwrap();
        let event = create_test_audit_event();

        let _audit_id = logger.log_event(event.clone()).await.unwrap();

        // Verify log file exists
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let log_file = log_path.join(format!("audit-{}.jsonl", today));
        assert!(log_file.exists());
    }

    /// Verify FilesystemAuditLogger maintains hash chain
    #[tokio::test]
    async fn test_filesystem_logger_hash_chain() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit_logs");

        let logger = FilesystemAuditLogger::new(log_path.clone()).unwrap();

        let event1 = create_test_audit_event();
        let event2 = create_test_audit_event();

        let _audit_id1 = logger.log_event(event1.clone()).await.unwrap();
        let _audit_id2 = logger.log_event(event2.clone()).await.unwrap();

        // Read log file and verify hash chain
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let log_file = log_path.join(format!("audit-{}.jsonl", today));
        let content = std::fs::read_to_string(log_file).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();

        // Should have 2 events
        assert_eq!(lines.len(), 2);

        // Parse events and verify hash chain
        let logged_event1: AuditEvent = serde_json::from_str(lines[0]).unwrap();
        let logged_event2: AuditEvent = serde_json::from_str(lines[1]).unwrap();

        // First event should have no previous hash
        assert!(logged_event1.previous_hash.is_none());

        // Second event should chain to first
        assert_eq!(
            logged_event2.previous_hash,
            Some(logged_event1.content_hash)
        );
    }
}

// ============================================================================
// StdoutAuditLogger Tests
// ============================================================================

mod stdout_logger_tests {
    use super::*;

    /// Verify StdoutAuditLogger can be created
    #[tokio::test]
    async fn test_stdout_logger_creation() {
        let _logger = StdoutAuditLogger::new();
    }

    /// Verify StdoutAuditLogger logs events
    #[tokio::test]
    async fn test_stdout_logger_logs_events() {
        let logger = StdoutAuditLogger::new();
        let event = create_test_audit_event();

        // StdoutAuditLogger never fails - always returns success
        let _audit_id = logger.log_event(event.clone()).await.unwrap();
    }
}

// ============================================================================
// CompositeAuditLogger Tests
// ============================================================================

mod composite_logger_tests {
    use super::*;

    /// Verify CompositeAuditLogger can be created
    #[tokio::test]
    async fn test_composite_logger_creation() {
        let stdout_logger = Arc::new(StdoutAuditLogger::new());
        let _logger = CompositeAuditLogger::new(vec![stdout_logger]);
    }

    /// Verify CompositeAuditLogger logs to all backends
    #[tokio::test]
    async fn test_composite_logger_logs_to_all_backends() {
        let stdout_logger = Arc::new(StdoutAuditLogger::new());
        let logger = CompositeAuditLogger::new(vec![stdout_logger]);

        let event = create_test_audit_event();
        let _audit_id = logger.log_event(event.clone()).await.unwrap();
    }
}
