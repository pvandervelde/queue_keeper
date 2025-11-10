//! Tests for event envelope and core types.

use super::*;
use serde_json::json;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_repository() -> Repository {
    Repository {
        id: 123456,
        name: "test-repo".to_string(),
        full_name: "owner/test-repo".to_string(),
        owner: crate::client::RepositoryOwner {
            login: "owner".to_string(),
            id: 999,
            avatar_url: "https://example.com/avatar.png".to_string(),
            owner_type: crate::client::OwnerType::User,
        },
        private: false,
        description: Some("Test repository".to_string()),
        default_branch: "main".to_string(),
        html_url: "https://github.com/owner/test-repo".to_string(),
        clone_url: "https://github.com/owner/test-repo.git".to_string(),
        ssh_url: "git@github.com:owner/test-repo.git".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

// ============================================================================
// EventEnvelope Tests
// ============================================================================

/// Verify EventEnvelope can be constructed with required fields.
#[test]
fn test_event_envelope_creation() {
    let repository = create_test_repository();
    let payload = EventPayload::new(json!({"action": "opened"}));

    let envelope = EventEnvelope::new("pull_request".to_string(), repository.clone(), payload);

    assert_eq!(envelope.event_type, "pull_request");
    assert_eq!(envelope.repository.full_name, "owner/test-repo");
    assert_eq!(envelope.entity_type, EntityType::PullRequest);
}

/// Verify session ID can be added using builder pattern.
#[test]
fn test_event_envelope_with_session_id() {
    let repository = create_test_repository();
    let payload = EventPayload::new(json!({"action": "opened"}));

    let envelope = EventEnvelope::new("pull_request".to_string(), repository, payload)
        .with_session_id("session-123".to_string());

    assert_eq!(envelope.session_id, Some("session-123".to_string()));
}

/// Verify trace context can be added using builder pattern.
#[test]
fn test_event_envelope_with_trace_context() {
    let repository = create_test_repository();
    let payload = EventPayload::new(json!({"action": "opened"}));

    let trace_context = TraceContext {
        trace_id: "trace-123".to_string(),
        span_id: "span-456".to_string(),
        parent_span_id: None,
    };

    let envelope = EventEnvelope::new("pull_request".to_string(), repository, payload)
        .with_trace_context(trace_context.clone());

    assert!(envelope.trace_context.is_some());
    let ctx = envelope.trace_context.unwrap();
    assert_eq!(ctx.trace_id, "trace-123");
    assert_eq!(ctx.span_id, "span-456");
}

/// Verify entity_key returns correct format for PR events.
#[test]
fn test_event_envelope_entity_key_pull_request() {
    let repository = create_test_repository();
    let payload = EventPayload::new(json!({"action": "opened", "number": 42}));

    let mut envelope = EventEnvelope::new("pull_request".to_string(), repository, payload);
    envelope.entity_id = Some("42".to_string());

    let key = envelope.entity_key();
    assert!(key.contains("owner/test-repo"));
    assert!(key.contains("PullRequest"));
    assert!(key.contains("42"));
}

/// Verify entity_key returns repository-level key when entity_id is None.
#[test]
fn test_event_envelope_entity_key_repository_level() {
    let repository = create_test_repository();
    let payload = EventPayload::new(json!({"ref": "refs/heads/main"}));

    let envelope = EventEnvelope::new("push".to_string(), repository, payload);

    let key = envelope.entity_key();
    assert!(key.contains("owner/test-repo"));
}

/// Verify correlation_id returns the event ID as a string.
#[test]
fn test_event_envelope_correlation_id() {
    let repository = create_test_repository();
    let payload = EventPayload::new(json!({"action": "opened"}));

    let envelope = EventEnvelope::new("pull_request".to_string(), repository, payload);
    let correlation_id = envelope.correlation_id();

    assert!(!correlation_id.is_empty());
    assert_eq!(correlation_id, envelope.event_id.as_str());
}

/// Verify EventEnvelope serialization and deserialization.
#[test]
fn test_event_envelope_serde() {
    let repository = create_test_repository();
    let payload = EventPayload::new(json!({"action": "opened"}));

    let envelope = EventEnvelope::new("pull_request".to_string(), repository, payload)
        .with_session_id("session-123".to_string());

    let json = serde_json::to_value(&envelope).expect("Failed to serialize");
    let deserialized: EventEnvelope = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized.event_type, envelope.event_type);
    assert_eq!(deserialized.session_id, envelope.session_id);
}

// ============================================================================
// EventId Tests
// ============================================================================

/// Verify EventId can be created as a new UUID.
#[test]
fn test_event_id_new() {
    let id1 = EventId::new();
    let id2 = EventId::new();

    assert_ne!(id1, id2);
    assert!(!id1.as_str().is_empty());
}

/// Verify EventId can be created from GitHub delivery ID.
#[test]
fn test_event_id_from_github_delivery() {
    let id = EventId::from_github_delivery("12345-67890-abcdef");
    assert_eq!(id.as_str(), "gh-12345-67890-abcdef");
}

/// Verify EventId Display trait implementation.
#[test]
fn test_event_id_display() {
    let id = EventId::from_github_delivery("test-id");
    assert_eq!(format!("{}", id), "gh-test-id");
}

/// Verify EventId implements Default trait.
#[test]
fn test_event_id_default() {
    let id = EventId::default();
    assert!(!id.as_str().is_empty());
}

/// Verify EventId serialization and deserialization.
#[test]
fn test_event_id_serde() {
    let id = EventId::from_github_delivery("delivery-123");
    let json = serde_json::to_value(&id).expect("Failed to serialize");
    let deserialized: EventId = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized, id);
}

// ============================================================================
// EntityType Tests
// ============================================================================

/// Verify EntityType::from_event_type maps pull_request correctly.
#[test]
fn test_entity_type_from_event_type_pull_request() {
    let entity_type = EntityType::from_event_type("pull_request");
    assert_eq!(entity_type, EntityType::PullRequest);
}

/// Verify EntityType::from_event_type maps issues correctly.
#[test]
fn test_entity_type_from_event_type_issues() {
    let entity_type = EntityType::from_event_type("issues");
    assert_eq!(entity_type, EntityType::Issue);
}

/// Verify EntityType::from_event_type maps push to Branch.
#[test]
fn test_entity_type_from_event_type_push() {
    let entity_type = EntityType::from_event_type("push");
    assert_eq!(entity_type, EntityType::Branch);
}

/// Verify EntityType::from_event_type returns Unknown for unsupported types.
#[test]
fn test_entity_type_from_event_type_unknown() {
    let entity_type = EntityType::from_event_type("unsupported_event");
    assert_eq!(entity_type, EntityType::Unknown);
}

/// Verify EntityType::supports_ordering returns true for ordered entities.
#[test]
fn test_entity_type_supports_ordering_true() {
    assert!(EntityType::PullRequest.supports_ordering());
    assert!(EntityType::Issue.supports_ordering());
    assert!(EntityType::Branch.supports_ordering());
}

/// Verify EntityType::supports_ordering returns false for unordered entities.
#[test]
fn test_entity_type_supports_ordering_false() {
    assert!(!EntityType::Repository.supports_ordering());
    assert!(!EntityType::CheckRun.supports_ordering());
    assert!(!EntityType::Unknown.supports_ordering());
}

/// Verify EntityType serialization and deserialization.
#[test]
fn test_entity_type_serde() {
    let entity_type = EntityType::PullRequest;
    let json = serde_json::to_value(&entity_type).expect("Failed to serialize");
    let deserialized: EntityType = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized, entity_type);
}

// ============================================================================
// EventPayload Tests
// ============================================================================

/// Verify EventPayload can be created from JSON value.
#[test]
fn test_event_payload_new() {
    let value = json!({"action": "opened", "number": 42});
    let payload = EventPayload::new(value.clone());

    assert_eq!(payload.raw(), &value);
}

/// Verify EventPayload::parse_pull_request returns typed event.
#[test]
#[ignore] // Will pass after implementation
fn test_event_payload_parse_pull_request() {
    let value = json!({
        "action": "opened",
        "number": 42,
        "pull_request": {
            "id": 1,
            "number": 42,
            "title": "Test",
            "state": "open",
            "locked": false,
            "user": {"login": "test", "id": 123},
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "head": {"ref": "feature", "sha": "abc", "repo": null},
            "base": {"ref": "main", "sha": "def", "repo": null},
            "draft": false
        },
        "repository": {},
        "sender": {"id": 1, "login": "test", "type": "User"}
    });

    let payload = EventPayload::new(value);
    let pr_event = payload.parse_pull_request().expect("Failed to parse");

    assert_eq!(pr_event.number, 42);
    assert_eq!(pr_event.action, PullRequestAction::Opened);
}

/// Verify EventPayload::parse_issue returns typed event.
#[test]
#[ignore] // Will pass after implementation
fn test_event_payload_parse_issue() {
    let value = json!({
        "action": "opened",
        "issue": {
            "id": 1,
            "number": 10,
            "title": "Test Issue",
            "state": "open",
            "locked": false,
            "user": {"login": "test", "id": 123},
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        },
        "repository": {},
        "sender": {"id": 1, "login": "test", "type": "User"}
    });

    let payload = EventPayload::new(value);
    let issue_event = payload.parse_issue().expect("Failed to parse");

    assert_eq!(issue_event.issue.number, 10);
    assert_eq!(issue_event.action, IssueAction::Opened);
}

/// Verify EventPayload serialization and deserialization.
#[test]
fn test_event_payload_serde() {
    let value = json!({"action": "opened", "data": "test"});
    let payload = EventPayload::new(value.clone());

    let json = serde_json::to_value(&payload).expect("Failed to serialize");
    let deserialized: EventPayload = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized.raw(), &value);
}

// ============================================================================
// EventMetadata Tests
// ============================================================================

/// Verify EventMetadata default values.
#[test]
fn test_event_metadata_default() {
    let metadata = EventMetadata::default();

    assert_eq!(metadata.source, EventSource::GitHub);
    assert!(!metadata.signature_valid);
    assert_eq!(metadata.retry_count, 0);
    assert!(metadata.routing_rules.is_empty());
    assert!(metadata.delivery_id.is_none());
    assert!(metadata.processed_at.is_none());
}

/// Verify EventMetadata received_at is set to current time.
#[test]
fn test_event_metadata_received_at() {
    let before = Utc::now();
    let metadata = EventMetadata::default();
    let after = Utc::now();

    assert!(metadata.received_at >= before);
    assert!(metadata.received_at <= after);
}

/// Verify EventMetadata can be customized.
#[test]
fn test_event_metadata_customization() {
    let mut metadata = EventMetadata::default();
    metadata.source = EventSource::Replay;
    metadata.signature_valid = true;
    metadata.delivery_id = Some("delivery-123".to_string());
    metadata.retry_count = 3;
    metadata.routing_rules.push("rule1".to_string());

    assert_eq!(metadata.source, EventSource::Replay);
    assert!(metadata.signature_valid);
    assert_eq!(metadata.delivery_id, Some("delivery-123".to_string()));
    assert_eq!(metadata.retry_count, 3);
    assert_eq!(metadata.routing_rules.len(), 1);
}

// ============================================================================
// EventSource Tests
// ============================================================================

/// Verify EventSource enum variants.
#[test]
fn test_event_source_variants() {
    assert_eq!(EventSource::GitHub, EventSource::GitHub);
    assert_ne!(EventSource::GitHub, EventSource::Replay);
    assert_ne!(EventSource::Replay, EventSource::Test);
}

/// Verify EventSource serialization and deserialization.
#[test]
fn test_event_source_serde() {
    let source = EventSource::GitHub;
    let json = serde_json::to_value(&source).expect("Failed to serialize");
    let deserialized: EventSource = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized, source);
}

// ============================================================================
// TraceContext Tests
// ============================================================================

/// Verify TraceContext can be created with all fields.
#[test]
fn test_trace_context_creation() {
    let context = TraceContext {
        trace_id: "trace-123".to_string(),
        span_id: "span-456".to_string(),
        parent_span_id: Some("parent-789".to_string()),
    };

    assert_eq!(context.trace_id, "trace-123");
    assert_eq!(context.span_id, "span-456");
    assert_eq!(context.parent_span_id, Some("parent-789".to_string()));
}

/// Verify TraceContext can be created without parent span.
#[test]
fn test_trace_context_no_parent() {
    let context = TraceContext {
        trace_id: "trace-abc".to_string(),
        span_id: "span-def".to_string(),
        parent_span_id: None,
    };

    assert!(context.parent_span_id.is_none());
}

/// Verify TraceContext serialization and deserialization.
#[test]
fn test_trace_context_serde() {
    let context = TraceContext {
        trace_id: "trace-123".to_string(),
        span_id: "span-456".to_string(),
        parent_span_id: Some("parent-789".to_string()),
    };

    let json = serde_json::to_value(&context).expect("Failed to serialize");
    let deserialized: TraceContext = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized.trace_id, context.trace_id);
    assert_eq!(deserialized.span_id, context.span_id);
    assert_eq!(deserialized.parent_span_id, context.parent_span_id);
}

use super::*;

#[test]
fn test_module_exports() {
    // Verify that main types are accessible through the module
    let _ = EventId::new();
    let _ = EntityType::PullRequest;
    let _ = EventSource::GitHub;
    let _ = ProcessorConfig::default();
}

#[test]
fn test_session_id_strategy_variants() {
    // Test that all strategy variants can be created
    let _ = SessionIdStrategy::None;
    let _ = SessionIdStrategy::Entity;
    let _ = SessionIdStrategy::Repository;
}

#[test]
fn test_pull_request_action_variants() {
    // Ensure all PR action variants exist
    let actions = [
        PullRequestAction::Opened,
        PullRequestAction::Closed,
        PullRequestAction::Reopened,
        PullRequestAction::Synchronize,
        PullRequestAction::Edited,
        PullRequestAction::Assigned,
        PullRequestAction::Unassigned,
        PullRequestAction::ReviewRequested,
        PullRequestAction::ReviewRequestRemoved,
        PullRequestAction::Labeled,
        PullRequestAction::Unlabeled,
        PullRequestAction::ReadyForReview,
        PullRequestAction::ConvertedToDraft,
    ];

    assert_eq!(actions.len(), 13);
}

#[test]
fn test_issue_action_variants() {
    // Ensure all issue action variants exist
    let actions = [
        IssueAction::Opened,
        IssueAction::Closed,
        IssueAction::Reopened,
        IssueAction::Edited,
        IssueAction::Assigned,
        IssueAction::Unassigned,
        IssueAction::Labeled,
        IssueAction::Unlabeled,
        IssueAction::Transferred,
        IssueAction::Pinned,
        IssueAction::Unpinned,
    ];

    assert_eq!(actions.len(), 11);
}

#[test]
fn test_check_run_action_variants() {
    let actions = [
        CheckRunAction::Created,
        CheckRunAction::Completed,
        CheckRunAction::Rerequested,
        CheckRunAction::RequestedAction,
    ];

    assert_eq!(actions.len(), 4);
}

#[test]
fn test_check_suite_action_variants() {
    let actions = [
        CheckSuiteAction::Completed,
        CheckSuiteAction::Requested,
        CheckSuiteAction::Rerequested,
    ];

    assert_eq!(actions.len(), 3);
}

#[test]
fn test_entity_type_variants() {
    let types = [
        EntityType::Repository,
        EntityType::PullRequest,
        EntityType::Issue,
        EntityType::Branch,
        EntityType::Release,
        EntityType::User,
        EntityType::Organization,
        EntityType::CheckRun,
        EntityType::CheckSuite,
        EntityType::Deployment,
        EntityType::Unknown,
    ];

    assert_eq!(types.len(), 11);
}

#[test]
fn test_event_source_enum_variants() {
    let sources = [EventSource::GitHub, EventSource::Replay, EventSource::Test];

    assert_eq!(sources.len(), 3);
}
