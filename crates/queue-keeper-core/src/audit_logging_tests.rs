//! Tests for audit logging module.

use super::*;
use crate::{RepositoryId, User, UserId, UserType};

// ============================================================================
// AuditLogId Tests
// ============================================================================

#[test]
fn test_audit_log_id_creation() {
    let id1 = AuditLogId::new();
    let id2 = AuditLogId::new();

    assert_ne!(id1, id2);
    assert!(!id1.as_str().is_empty());
}

#[test]
fn test_audit_log_id_from_string() {
    let id = AuditLogId::new();
    let id_str = id.as_str();
    let parsed_id = AuditLogId::from_str(&id_str).expect("Should parse valid ID");

    assert_eq!(id, parsed_id);
}

#[test]
fn test_audit_log_id_invalid_string() {
    let result = AuditLogId::from_str("invalid-id");
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AuditError::InvalidAuditId { .. }
    ));
}

#[test]
fn test_audit_log_id_display() {
    let id = AuditLogId::new();
    let display_str = format!("{}", id);
    let as_str = id.as_str();

    assert_eq!(display_str, as_str);
}

// ============================================================================
// AuditEvent Creation and Basic Tests
// ============================================================================

#[test]
fn test_audit_event_creation() {
    let event_type = AuditEventType::WebhookProcessing;
    let actor = AuditActor::System {
        component_name: "queue-keeper".to_string(),
        instance_id: "instance-1".to_string(),
        version: "1.0.0".to_string(),
    };
    let resource = AuditResource::WebhookEvent {
        event_id: EventId::new(),
        session_id: SessionId::from_parts("owner", "repo", "pull_request", "123"),
        repository: Repository {
            id: RepositoryId::new(123),
            owner: User {
                id: UserId::new(456),
                login: "owner".to_string(),
                user_type: UserType::User,
            },
            name: "repo".to_string(),
            full_name: "owner/repo".to_string(),
            private: false,
        },
        event_type: "push".to_string(),
    };
    let action = AuditAction::Process {
        operation: "webhook_processing".to_string(),
    };
    let result = AuditResult::Success {
        duration: Some(Duration::from_millis(100)),
        details: Some("Success".to_string()),
    };
    let context = AuditContext::default();

    let audit_event = AuditEvent::new(event_type, actor, resource, action, result, context);

    assert!(audit_event.verify_integrity());
    assert_eq!(
        audit_event.get_compliance_category(),
        ComplianceCategory::Operational
    );
}

#[test]
fn test_audit_event_has_unique_id() {
    let event1 = create_test_audit_event();
    let event2 = create_test_audit_event();

    assert_ne!(event1.audit_id, event2.audit_id);
}

#[test]
fn test_audit_event_timestamps_set() {
    let event = create_test_audit_event();

    // Timestamps should be set (year should be reasonable)
    assert!(event.occurred_at.year() > 2020);
    assert!(event.logged_at.year() > 2020);

    // logged_at should be >= occurred_at
    assert!(event.logged_at >= event.occurred_at);
}

#[test]
fn test_audit_event_content_hash_generated() {
    let event = create_test_audit_event();

    assert!(!event.content_hash.is_empty());
}

#[test]
fn test_audit_event_no_previous_hash_on_creation() {
    let event = create_test_audit_event();

    assert!(event.previous_hash.is_none());
}

// ============================================================================
// Content Hash and Integrity Tests
// ============================================================================

#[test]
fn test_audit_event_verify_integrity_success() {
    let event = create_test_audit_event();

    assert!(event.verify_integrity());
}

#[test]
fn test_audit_event_verify_integrity_detects_tampering() {
    let mut event = create_test_audit_event();

    // Tamper with the content hash
    event.content_hash = "tampered_hash".to_string();

    assert!(!event.verify_integrity());
}

#[test]
fn test_audit_event_different_events_have_different_hashes() {
    let event1 = create_test_audit_event();
    let event2 = create_test_audit_event();

    // Even though they're created similarly, they should have different hashes
    // because they have different audit_ids and timestamps
    assert_ne!(event1.content_hash, event2.content_hash);
}

#[test]
fn test_audit_event_with_previous_hash() {
    let event1 = create_test_audit_event();
    let mut event2 = create_test_audit_event();

    // Link event2 to event1
    event2.previous_hash = Some(event1.content_hash.clone());

    assert_eq!(event2.previous_hash, Some(event1.content_hash));
}

// ============================================================================
// Compliance Category Tests
// ============================================================================

#[test]
fn test_compliance_category_security() {
    let event = create_audit_event_with_type(AuditEventType::Security);
    assert_eq!(
        event.get_compliance_category(),
        ComplianceCategory::Security
    );
}

#[test]
fn test_compliance_category_administration() {
    let event = create_audit_event_with_type(AuditEventType::Administration);
    assert_eq!(
        event.get_compliance_category(),
        ComplianceCategory::Operational
    );
}

#[test]
fn test_compliance_category_configuration() {
    let event = create_audit_event_with_type(AuditEventType::Configuration);
    assert_eq!(
        event.get_compliance_category(),
        ComplianceCategory::Operational
    );
}

#[test]
fn test_compliance_category_data_access() {
    let event = create_audit_event_with_type(AuditEventType::DataAccess);
    assert_eq!(event.get_compliance_category(), ComplianceCategory::Privacy);
}

#[test]
fn test_compliance_category_compliance() {
    let event = create_audit_event_with_type(AuditEventType::Compliance);
    assert_eq!(
        event.get_compliance_category(),
        ComplianceCategory::Financial
    );
}

#[test]
fn test_compliance_category_system() {
    let event = create_audit_event_with_type(AuditEventType::System);
    assert_eq!(
        event.get_compliance_category(),
        ComplianceCategory::Operational
    );
}

#[test]
fn test_compliance_category_webhook_processing() {
    let event = create_audit_event_with_type(AuditEventType::WebhookProcessing);
    assert_eq!(
        event.get_compliance_category(),
        ComplianceCategory::Operational
    );
}

// ============================================================================
// Encryption Requirement Tests
// ============================================================================

#[test]
fn test_requires_encryption_security_events() {
    let event = create_audit_event_with_type(AuditEventType::Security);
    assert!(event.requires_encryption());
}

#[test]
fn test_requires_encryption_data_access_events() {
    let event = create_audit_event_with_type(AuditEventType::DataAccess);
    assert!(event.requires_encryption());
}

#[test]
fn test_requires_encryption_compliance_events() {
    let event = create_audit_event_with_type(AuditEventType::Compliance);
    assert!(event.requires_encryption());
}

#[test]
fn test_no_encryption_required_webhook_processing() {
    let event = create_audit_event_with_type(AuditEventType::WebhookProcessing);
    // Assumes webhook events with non-sensitive resources don't require encryption
    assert!(!event.requires_encryption());
}

#[test]
fn test_requires_encryption_sensitive_resource() {
    let actor = create_test_actor();
    let action = create_test_action();
    let result = create_test_result();
    let context = AuditContext::default();

    // Secret resource is sensitive
    let sensitive_resource = AuditResource::Secret {
        secret_name: "webhook-secret".to_string(),
        key_vault: "production-vault".to_string(),
    };

    let event = AuditEvent::new(
        AuditEventType::Administration,
        actor,
        sensitive_resource,
        action,
        result,
        context,
    );

    assert!(event.requires_encryption());
}

// ============================================================================
// Retention Period Tests
// ============================================================================

#[test]
fn test_retention_period_financial() {
    let event = create_audit_event_with_type(AuditEventType::Compliance);
    let retention = event.get_retention_period();

    // Financial (7 years)
    assert_eq!(retention.as_secs(), 7 * 365 * 24 * 3600);
}

#[test]
fn test_retention_period_security() {
    let event = create_audit_event_with_type(AuditEventType::Security);
    let retention = event.get_retention_period();

    // Security (3 years)
    assert_eq!(retention.as_secs(), 3 * 365 * 24 * 3600);
}

#[test]
fn test_retention_period_privacy() {
    let event = create_audit_event_with_type(AuditEventType::DataAccess);
    let retention = event.get_retention_period();

    // Privacy (2 years)
    assert_eq!(retention.as_secs(), 2 * 365 * 24 * 3600);
}

#[test]
fn test_retention_period_operational() {
    let event = create_audit_event_with_type(AuditEventType::WebhookProcessing);
    let retention = event.get_retention_period();

    // Operational (1 year)
    assert_eq!(retention.as_secs(), 365 * 24 * 3600);
}

// ============================================================================
// AuditEventType Tests
// ============================================================================

#[test]
fn test_event_type_compliance_level() {
    assert_eq!(
        AuditEventType::Security.get_compliance_level(),
        ComplianceLevel::Critical
    );
    assert_eq!(
        AuditEventType::Compliance.get_compliance_level(),
        ComplianceLevel::Critical
    );
    assert_eq!(
        AuditEventType::Administration.get_compliance_level(),
        ComplianceLevel::Important
    );
}

#[test]
fn test_event_type_requires_encryption() {
    assert!(AuditEventType::Security.requires_encryption());
    assert!(AuditEventType::DataAccess.requires_encryption());
    assert!(AuditEventType::Compliance.requires_encryption());
    assert!(!AuditEventType::System.requires_encryption());
}

#[test]
fn test_event_type_retention_period() {
    let security_retention = AuditEventType::Security.get_retention_period();
    assert_eq!(security_retention.as_secs(), 3 * 365 * 24 * 3600);

    let system_retention = AuditEventType::System.get_retention_period();
    assert_eq!(system_retention.as_secs(), 365 * 24 * 3600);
}

// ============================================================================
// AuditActor Tests
// ============================================================================

#[test]
fn test_audit_actor_user_description() {
    let user_actor = AuditActor::User {
        user_id: "user123".to_string(),
        username: "john_doe".to_string(),
        email: Some("john@example.com".to_string()),
        role: Some("Admin".to_string()),
    };

    assert_eq!(user_actor.get_description(), "john_doe (Admin)");
    assert!(user_actor.is_privileged());
    assert_eq!(user_actor.get_actor_id(), "user123");
}

#[test]
fn test_audit_actor_user_without_role() {
    let user_actor = AuditActor::User {
        user_id: "user456".to_string(),
        username: "jane_doe".to_string(),
        email: None,
        role: None,
    };

    assert_eq!(user_actor.get_description(), "jane_doe");
    assert!(!user_actor.is_privileged());
}

#[test]
fn test_audit_actor_system_description() {
    let system_actor = AuditActor::System {
        component_name: "queue-keeper".to_string(),
        instance_id: "instance-1".to_string(),
        version: "1.0.0".to_string(),
    };

    assert_eq!(system_actor.get_description(), "queue-keeper v1.0.0");
    assert!(system_actor.is_privileged());
    assert_eq!(system_actor.get_actor_id(), "queue-keeper:instance-1");
}

#[test]
fn test_audit_actor_external_service() {
    let external_actor = AuditActor::ExternalService {
        service_name: "github".to_string(),
        service_id: "webhook-delivery".to_string(),
        authenticated: true,
    };

    assert!(external_actor.get_description().contains("github"));
    assert!(external_actor.is_privileged()); // Authenticated services are privileged
    assert_eq!(external_actor.get_actor_id(), "github:webhook-delivery");
}

#[test]
fn test_audit_actor_anonymous() {
    let anon_actor = AuditActor::Anonymous {
        source_ip: Some("192.168.1.100".to_string()),
        user_agent: Some("Mozilla/5.0".to_string()),
    };

    assert!(anon_actor.get_description().contains("Anonymous"));
    assert!(!anon_actor.is_privileged());
}

// ============================================================================
// AuditResource Tests
// ============================================================================

#[test]
fn test_audit_resource_webhook_event() {
    let resource = AuditResource::WebhookEvent {
        event_id: EventId::new(),
        session_id: SessionId::from_parts("owner", "repo", "pr", "123"),
        repository: create_test_repository(),
        event_type: "pull_request.opened".to_string(),
    };

    assert_eq!(resource.get_resource_type(), "webhook_event");
    assert!(!resource.is_sensitive());
}

#[test]
fn test_audit_resource_secret_is_sensitive() {
    let resource = AuditResource::Secret {
        secret_name: "webhook-secret".to_string(),
        key_vault: "production".to_string(),
    };

    assert_eq!(resource.get_resource_type(), "secret");
    assert!(resource.is_sensitive());
}

#[test]
fn test_audit_resource_bot_configuration() {
    let resource = AuditResource::BotConfiguration {
        bot_name: "security-bot".to_string(),
        configuration_version: Some("v2.0.0".to_string()),
    };

    assert_eq!(resource.get_resource_type(), "bot_configuration");
    assert_eq!(resource.get_resource_id(), "security-bot");
}

// ============================================================================
// AuditAction Tests
// ============================================================================

#[test]
fn test_audit_action_create() {
    let action = AuditAction::Create {
        details: Some("Created new bot configuration".to_string()),
    };

    assert_eq!(action.get_category(), ActionCategory::DataOperation);
    assert!(!action.is_high_risk());
}

#[test]
fn test_audit_action_delete_is_high_risk() {
    let action = AuditAction::Delete {
        reason: Some("Deprecated configuration".to_string()),
    };

    assert!(action.is_high_risk());
    assert_eq!(action.get_approval_level(), ApprovalLevel::Manager);
}

#[test]
fn test_audit_action_configure() {
    let action = AuditAction::Configure {
        setting: "webhook_timeout".to_string(),
        value: Some("30s".to_string()),
    };

    assert_eq!(
        action.get_category(),
        ActionCategory::AdministrativeOperation
    );
}

// ============================================================================
// AuditResult Tests
// ============================================================================

#[test]
fn test_audit_result_success() {
    let success = AuditResult::Success {
        duration: Some(Duration::from_millis(100)),
        details: None,
    };

    assert!(success.is_successful());
    assert!(!success.is_error());
    assert_eq!(success.get_error_code(), None);
}

#[test]
fn test_audit_result_failure() {
    let failure = AuditResult::Failure {
        error_code: "ERR001".to_string(),
        error_message: "Something went wrong".to_string(),
        retryable: true,
    };

    assert!(!failure.is_successful());
    assert!(failure.is_error());
    assert_eq!(failure.get_error_code(), Some("ERR001"));
}

#[test]
fn test_audit_result_partial() {
    let partial = AuditResult::Partial {
        success_count: 2,
        failure_count: 1,
        details: "2 of 3 queues delivered successfully".to_string(),
    };

    assert!(!partial.is_successful());
    assert!(!partial.is_error());
}

#[test]
fn test_audit_result_skipped() {
    let skipped = AuditResult::Skipped {
        reason: "Already processed".to_string(),
    };

    assert!(!skipped.is_successful());
    assert!(!skipped.is_error());
}

// ============================================================================
// AuditContext Tests
// ============================================================================

#[test]
fn test_audit_context_default() {
    let context = AuditContext::default();

    assert!(context.correlation_id.is_none());
    assert!(context.request_id.is_none());
    assert!(context.source_ip.is_none());
    assert!(context.additional_data.is_empty());
}

#[test]
fn test_audit_context_with_correlation_id() {
    let mut context = AuditContext::default();
    context.correlation_id = Some("corr-123".to_string());
    context.request_id = Some("req-456".to_string());

    assert_eq!(context.correlation_id, Some("corr-123".to_string()));
    assert_eq!(context.request_id, Some("req-456".to_string()));
}

#[test]
fn test_audit_context_with_performance_metrics() {
    let mut context = AuditContext::default();
    context.performance = Some(PerformanceContext {
        duration_ms: 250,
        memory_usage_bytes: Some(1024 * 1024),
        cpu_usage_percent: Some(45.5),
        network_bytes_sent: Some(2048),
        network_bytes_received: Some(4096),
    });

    assert!(context.performance.is_some());
    assert_eq!(context.performance.as_ref().unwrap().duration_ms, 250);
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_audit_event_serialization() {
    let event = create_test_audit_event();

    let json = serde_json::to_string(&event).expect("Should serialize to JSON");
    assert!(!json.is_empty());
    assert!(json.contains("audit_id"));
    assert!(json.contains("event_type"));
}

#[test]
fn test_audit_event_deserialization() {
    let event = create_test_audit_event();
    let json = serde_json::to_string(&event).expect("Should serialize");

    let deserialized: AuditEvent = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(event.audit_id, deserialized.audit_id);
    assert_eq!(event.event_type, deserialized.event_type);
}

#[test]
fn test_audit_event_structured_logging_format() {
    let event = create_test_audit_event();
    let json = serde_json::to_value(&event).expect("Should serialize to JSON value");

    // Verify structured logging fields per assertion #18
    assert!(json.get("audit_id").is_some());
    assert!(json.get("occurred_at").is_some());
    assert!(json.get("event_type").is_some());
    assert!(json.get("actor").is_some());
    assert!(json.get("resource").is_some());
    assert!(json.get("action").is_some());
    assert!(json.get("result").is_some());
    assert!(json.get("content_hash").is_some());
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_audit_event() -> AuditEvent {
    AuditEvent::new(
        AuditEventType::WebhookProcessing,
        create_test_actor(),
        create_test_resource(),
        create_test_action(),
        create_test_result(),
        AuditContext::default(),
    )
}

fn create_audit_event_with_type(event_type: AuditEventType) -> AuditEvent {
    AuditEvent::new(
        event_type,
        create_test_actor(),
        create_test_resource(),
        create_test_action(),
        create_test_result(),
        AuditContext::default(),
    )
}

fn create_test_actor() -> AuditActor {
    AuditActor::System {
        component_name: "queue-keeper".to_string(),
        instance_id: "instance-1".to_string(),
        version: "1.0.0".to_string(),
    }
}

fn create_test_resource() -> AuditResource {
    AuditResource::WebhookEvent {
        event_id: EventId::new(),
        session_id: SessionId::from_parts("owner", "repo", "pull_request", "123"),
        repository: create_test_repository(),
        event_type: "push".to_string(),
    }
}

fn create_test_repository() -> Repository {
    Repository {
        id: RepositoryId::new(123),
        owner: User {
            id: UserId::new(456),
            login: "owner".to_string(),
            user_type: UserType::User,
        },
        name: "repo".to_string(),
        full_name: "owner/repo".to_string(),
        private: false,
    }
}

fn create_test_action() -> AuditAction {
    AuditAction::Process {
        operation: "webhook_processing".to_string(),
    }
}

fn create_test_result() -> AuditResult {
    AuditResult::Success {
        duration: Some(Duration::from_millis(100)),
        details: Some("Success".to_string()),
    }
}
