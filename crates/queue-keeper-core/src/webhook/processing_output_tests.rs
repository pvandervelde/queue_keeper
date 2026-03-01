//! Tests for [`ProcessingOutput`] and [`DirectQueueMetadata`].

use super::*;
use crate::Timestamp;
use bytes::Bytes;

// ============================================================================
// Test helpers
// ============================================================================

/// Build a minimal [`WrappedEvent`] for testing purposes.
fn test_wrapped_event() -> WrappedEvent {
    WrappedEvent::new(
        "test-provider".to_string(),
        "push".to_string(),
        None,
        None,
        serde_json::json!({}),
    )
}

/// Build a [`WrappedEvent`] with an action and session ID for testing.
fn test_wrapped_event_with_session() -> WrappedEvent {
    use crate::SessionId;
    let session = SessionId::from_parts("owner", "repo", "pull_request", "42");
    WrappedEvent::new(
        "github".to_string(),
        "pull_request".to_string(),
        Some("opened".to_string()),
        Some(session),
        serde_json::json!({"action": "opened"}),
    )
}

// ============================================================================
// ProcessingOutput tests
// ============================================================================

mod processing_output_variants {
    use super::*;

    /// Verify that `ProcessingOutput::Wrapped` correctly holds a [`WrappedEvent`]
    /// and the `is_wrapped()` / `is_direct()` predicates report correctly.
    #[test]
    fn test_wrapped_variant_holds_event_and_predicates_are_correct() {
        let event = test_wrapped_event();
        let output = ProcessingOutput::Wrapped(event.clone());

        assert!(output.is_wrapped(), "expected is_wrapped() == true");
        assert!(!output.is_direct(), "expected is_direct() == false");
    }

    /// Verify that `ProcessingOutput::Direct` correctly holds payload and
    /// metadata, and the predicates report correctly.
    #[test]
    fn test_direct_variant_holds_payload_and_metadata() {
        let payload = Bytes::from(r#"{"key":"value"}"#);
        let metadata = DirectQueueMetadata::new("jira", "application/json");

        let output = ProcessingOutput::Direct {
            payload: payload.clone(),
            metadata,
        };

        assert!(output.is_direct(), "expected is_direct() == true");
        assert!(!output.is_wrapped(), "expected is_wrapped() == false");

        if let ProcessingOutput::Direct {
            payload: p,
            metadata: m,
        } = &output
        {
            assert_eq!(p.as_ref(), payload.as_ref());
            assert_eq!(m.provider_id(), "jira");
            assert_eq!(m.content_type(), "application/json");
        }
    }

    /// Verify `event_id()` returns the wrapped event's event ID for wrapped outputs.
    #[test]
    fn test_event_id_returns_wrapped_event_id_for_wrapped() {
        let event = test_wrapped_event();
        let expected_id = event.event_id;
        let output = ProcessingOutput::Wrapped(event);

        assert_eq!(output.event_id(), expected_id);
    }

    /// Verify `event_id()` returns the metadata's event ID for direct outputs.
    #[test]
    fn test_event_id_returns_metadata_id_for_direct() {
        let metadata = DirectQueueMetadata::new("gitlab", "application/json");
        let expected_id = metadata.event_id();
        let output = ProcessingOutput::Direct {
            payload: Bytes::new(),
            metadata,
        };

        assert_eq!(output.event_id(), expected_id);
    }

    /// Verify `correlation_id()` returns the wrapped event's value for wrapped outputs.
    #[test]
    fn test_correlation_id_returns_wrapped_event_id_for_wrapped() {
        let event = test_wrapped_event();
        let expected = event.correlation_id.clone();
        let output = ProcessingOutput::Wrapped(event);

        assert_eq!(output.correlation_id(), &expected);
    }

    /// Verify `correlation_id()` returns the metadata's value for direct outputs.
    #[test]
    fn test_correlation_id_returns_metadata_id_for_direct() {
        let metadata = DirectQueueMetadata::new("slack", "application/json");
        let expected = metadata.correlation_id().clone();
        let output = ProcessingOutput::Direct {
            payload: Bytes::new(),
            metadata,
        };

        assert_eq!(output.correlation_id(), &expected);
    }

    /// Verify `session_id()` returns the session from a wrapped event that has one.
    #[test]
    fn test_session_id_returns_some_for_wrapped_with_session() {
        let event = test_wrapped_event_with_session();
        let output = ProcessingOutput::Wrapped(event);

        assert!(
            output.session_id().is_some(),
            "session_id() should be Some for an event with a session"
        );
    }

    /// Verify `session_id()` returns `None` for a wrapped event without a session.
    #[test]
    fn test_session_id_returns_none_for_wrapped_without_session() {
        let event = test_wrapped_event(); // no session
        let output = ProcessingOutput::Wrapped(event);

        assert!(
            output.session_id().is_none(),
            "session_id() should be None when event has no session"
        );
    }

    /// Verify `session_id()` returns `None` for direct outputs.
    #[test]
    fn test_session_id_returns_none_for_direct() {
        let output = ProcessingOutput::Direct {
            payload: Bytes::new(),
            metadata: DirectQueueMetadata::new("jira", "application/json"),
        };

        assert!(
            output.session_id().is_none(),
            "session_id() should always be None for Direct output"
        );
    }

    /// Verify `event_type()` returns the event type for wrapped outputs.
    #[test]
    fn test_event_type_returns_type_for_wrapped() {
        let event = test_wrapped_event();
        let output = ProcessingOutput::Wrapped(event);

        assert_eq!(
            output.event_type(),
            Some("push"),
            "event_type() should return the wrapped event's type"
        );
    }

    /// Verify `event_type()` returns `None` for direct outputs.
    #[test]
    fn test_event_type_returns_none_for_direct() {
        let output = ProcessingOutput::Direct {
            payload: Bytes::new(),
            metadata: DirectQueueMetadata::new("jira", "application/json"),
        };

        assert!(
            output.event_type().is_none(),
            "event_type() should be None for Direct output"
        );
    }

    /// Verify `as_wrapped()` returns `Some(&WrappedEvent)` for wrapped outputs.
    #[test]
    fn test_as_wrapped_returns_some_for_wrapped() {
        let event = test_wrapped_event();
        let expected_id = event.event_id;
        let output = ProcessingOutput::Wrapped(event);

        let wrapped = output.as_wrapped();
        assert!(wrapped.is_some(), "as_wrapped() should return Some");
        assert_eq!(
            wrapped.unwrap().event_id,
            expected_id,
            "inner event should match"
        );
    }

    /// Verify `as_wrapped()` returns `None` for direct outputs.
    #[test]
    fn test_as_wrapped_returns_none_for_direct() {
        let output = ProcessingOutput::Direct {
            payload: Bytes::new(),
            metadata: DirectQueueMetadata::new("jira", "application/json"),
        };

        assert!(
            output.as_wrapped().is_none(),
            "as_wrapped() should be None for Direct output"
        );
    }
}

// ============================================================================
// WrappedEvent tests
// ============================================================================

mod wrapped_event_tests {
    use super::*;
    use crate::SessionId;

    /// Verify `new()` generates a non-empty event_id.
    #[test]
    fn test_new_generates_non_empty_event_id() {
        let event = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        assert!(
            !event.event_id.as_str().is_empty(),
            "event_id must be non-empty"
        );
    }

    /// Verify `new()` generates a non-empty correlation_id.
    #[test]
    fn test_new_generates_non_empty_correlation_id() {
        let event = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        assert!(
            !event.correlation_id.as_str().is_empty(),
            "correlation_id must be non-empty"
        );
    }

    /// Verify `provider`, `event_type`, `action` and `session_id` are stored correctly.
    #[test]
    fn test_fields_stored_correctly() {
        let session = SessionId::from_parts("owner", "repo", "issues", "7");
        let payload = serde_json::json!({"action": "opened"});

        let event = WrappedEvent::new(
            "github".to_string(),
            "issues".to_string(),
            Some("opened".to_string()),
            Some(session.clone()),
            payload.clone(),
        );

        assert_eq!(event.provider, "github");
        assert_eq!(event.event_type, "issues");
        assert_eq!(event.action.as_deref(), Some("opened"));
        assert_eq!(event.session_id.as_ref(), Some(&session));
        assert_eq!(event.payload, payload);
    }

    /// Verify `received_at` and `processed_at` are set to near-current time.
    #[test]
    fn test_timestamps_set_to_current_time() {
        let before = Timestamp::now();
        let event = WrappedEvent::new(
            "test".to_string(),
            "ping".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        let after = Timestamp::now();

        assert!(event.received_at >= before && event.received_at <= after);
        assert!(event.processed_at >= before && event.processed_at <= after);
    }

    /// Verify two independently created events have distinct event_ids.
    #[test]
    fn test_two_events_have_distinct_event_ids() {
        let e1 = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        let e2 = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        assert_ne!(e1.event_id, e2.event_id);
    }

    /// Verify `action` is `None` when not provided.
    #[test]
    fn test_action_is_none_when_not_provided() {
        let event = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        assert!(event.action.is_none());
    }

    /// Verify `session_id` is `None` when not provided.
    #[test]
    fn test_session_id_is_none_when_not_provided() {
        let event = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        assert!(event.session_id.is_none());
    }

    /// Verify serialisation round-trip for [`WrappedEvent`].
    #[test]
    fn test_serialization_roundtrip() {
        let session = SessionId::from_parts("owner", "repo", "push", "0");
        let event = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            Some(session),
            serde_json::json!({"ref": "refs/heads/main"}),
        );

        let json = serde_json::to_string(&event).expect("serialisation should succeed");
        let deser: WrappedEvent =
            serde_json::from_str(&json).expect("deserialisation should succeed");

        assert_eq!(deser.event_id, event.event_id);
        assert_eq!(deser.provider, event.provider);
        assert_eq!(deser.event_type, event.event_type);
        assert_eq!(deser.session_id, event.session_id);
        assert_eq!(deser.payload, event.payload);
    }
}

// ============================================================================
// DirectQueueMetadata tests
// ============================================================================

mod direct_queue_metadata_tests {
    use super::*;

    /// Verify `new()` auto-generates a non-empty `EventId`.
    #[test]
    fn test_new_generates_event_id() {
        let meta = DirectQueueMetadata::new("test-provider", "application/json");
        assert!(
            !meta.event_id().as_str().is_empty(),
            "event_id must be non-empty"
        );
    }

    /// Verify `new()` auto-generates a non-empty `CorrelationId`.
    #[test]
    fn test_new_generates_correlation_id() {
        let meta = DirectQueueMetadata::new("test-provider", "application/json");
        assert!(
            !meta.correlation_id().as_str().is_empty(),
            "correlation_id must be non-empty"
        );
    }

    /// Verify `received_at()` returns a timestamp close to "now".
    #[test]
    fn test_new_sets_received_at_to_current_time() {
        let before = Timestamp::now();
        let meta = DirectQueueMetadata::new("test-provider", "application/json");
        let after = Timestamp::now();

        assert!(
            meta.received_at() >= before && meta.received_at() <= after,
            "received_at should be between 'before' and 'after' timestamps"
        );
    }

    /// Verify `provider_id()` stores the value given at construction.
    #[test]
    fn test_provider_id_stored_correctly() {
        let meta = DirectQueueMetadata::new("my-cool-app", "text/xml");
        assert_eq!(meta.provider_id(), "my-cool-app");
    }

    /// Verify `content_type()` stores the value given at construction.
    #[test]
    fn test_content_type_stored_correctly() {
        let meta = DirectQueueMetadata::new("my-cool-app", "text/xml");
        assert_eq!(meta.content_type(), "text/xml");
    }

    /// Verify that two independently created metadata instances have distinct
    /// event IDs (global uniqueness).
    #[test]
    fn test_two_instances_have_distinct_event_ids() {
        let meta1 = DirectQueueMetadata::new("a", "application/json");
        let meta2 = DirectQueueMetadata::new("a", "application/json");

        assert_ne!(
            meta1.event_id(),
            meta2.event_id(),
            "each instance must have a unique event_id"
        );
    }

    /// Verify serialisation round-trip for `DirectQueueMetadata`.
    #[test]
    fn test_serialization_roundtrip() {
        let meta = DirectQueueMetadata::new("roundtrip-provider", "application/json");

        let json = serde_json::to_string(&meta).expect("serialisation should succeed");
        let deser: DirectQueueMetadata =
            serde_json::from_str(&json).expect("deserialisation should succeed");

        assert_eq!(deser.provider_id(), meta.provider_id());
        assert_eq!(deser.content_type(), meta.content_type());
        assert_eq!(deser.event_id(), meta.event_id());
    }
}
