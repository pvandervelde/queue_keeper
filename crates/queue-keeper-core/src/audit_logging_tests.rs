//! Tests for authentication module.

use super::*;

#[test]
fn test_audit_log_id_creation() {
    let id1 = AuditLogId::new();
    let id2 = AuditLogId::new();

    assert_ne!(id1, id2);
    assert!(!id1.as_str().is_empty());
}

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
fn test_audit_actor_description() {
    let user_actor = AuditActor::User {
        user_id: "user123".to_string(),
        username: "john_doe".to_string(),
        email: Some("john@example.com".to_string()),
        role: Some("Admin".to_string()),
    };

    assert_eq!(user_actor.get_description(), "john_doe (Admin)");
    assert!(user_actor.is_privileged());

    let system_actor = AuditActor::System {
        component_name: "queue-keeper".to_string(),
        instance_id: "instance-1".to_string(),
        version: "1.0.0".to_string(),
    };

    assert_eq!(system_actor.get_description(), "queue-keeper v1.0.0");
    assert!(system_actor.is_privileged());
}

#[test]
fn test_audit_result_checks() {
    let success = AuditResult::Success {
        duration: Some(Duration::from_millis(100)),
        details: None,
    };
    assert!(success.is_successful());
    assert!(!success.is_error());

    let failure = AuditResult::Failure {
        error_code: "ERR001".to_string(),
        error_message: "Something went wrong".to_string(),
        retryable: true,
    };
    assert!(!failure.is_successful());
    assert!(failure.is_error());
    assert_eq!(failure.get_error_code(), Some("ERR001"));
}
