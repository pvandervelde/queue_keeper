//! Tests for the queue-keeper-core library module.

use super::*;

#[test]
fn test_event_id_generation() {
    let id1 = EventId::new();
    let id2 = EventId::new();

    assert_ne!(id1, id2);
    assert!(!id1.as_str().is_empty());
}

#[test]
fn test_session_id_validation() {
    // Valid session ID
    let valid = SessionId::new("owner/repo/pull_request/123".to_string());
    assert!(valid.is_ok());

    // Too long
    let too_long = "a".repeat(129);
    let invalid = SessionId::new(too_long);
    assert!(matches!(invalid, Err(ValidationError::TooLong { .. })));

    // Invalid characters
    let invalid = SessionId::new("owner/repo with spaces/issue/456".to_string());
    assert!(matches!(
        invalid,
        Err(ValidationError::InvalidCharacters { .. })
    ));
}

#[test]
fn test_session_id_from_parts() {
    let session_id = SessionId::from_parts("microsoft", "vscode", "pull_request", "1234");
    assert_eq!(session_id.as_str(), "microsoft/vscode/pull_request/1234");
}

#[test]
fn test_retry_policy_delay_calculation() {
    let policy = RetryPolicy::exponential();

    let delay1 = policy.calculate_delay(1);
    let delay2 = policy.calculate_delay(2);
    let delay3 = policy.calculate_delay(3);

    assert!(delay1 > Duration::ZERO);
    assert!(delay2 > delay1);
    assert!(delay3 > delay2);
    assert!(delay3 <= policy.max_delay);
}

// ============================================================================
// CorrelationId – string-backed tests
// ============================================================================

mod correlation_id_tests {
    use super::*;

    /// Verify that a freshly generated CorrelationId is non-empty.
    #[test]
    fn test_correlation_id_new_is_non_empty() {
        let id = CorrelationId::new();
        assert!(!id.as_str().is_empty());
    }

    /// Verify that two generated CorrelationIds are distinct.
    #[test]
    fn test_correlation_id_new_generates_unique_ids() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();
        assert_ne!(id1, id2);
    }

    /// Verify that `as_str()` returns the same value as `Display`.
    #[test]
    fn test_correlation_id_as_str_matches_display() {
        let id = CorrelationId::new();
        assert_eq!(id.as_str(), id.to_string());
    }

    /// Verify that `FromStr` accepts any non-empty string, including non-UUID values.
    #[test]
    fn test_correlation_id_from_str_accepts_non_uuid_string() {
        let result = "00-4bf92f3577b34da6a-00f067aa0ba902b7-01".parse::<CorrelationId>();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().as_str(),
            "00-4bf92f3577b34da6a-00f067aa0ba902b7-01"
        );
    }

    /// Verify that `FromStr` rejects an empty string.
    #[test]
    fn test_correlation_id_from_str_rejects_empty_string() {
        let result = "".parse::<CorrelationId>();
        assert!(result.is_err());
    }

    /// Verify that a CorrelationId preserves a non-UUID value verbatim.
    #[test]
    fn test_correlation_id_preserves_non_uuid_value() {
        let id = "my-custom-trace-id".parse::<CorrelationId>().unwrap();
        assert_eq!(id.as_str(), "my-custom-trace-id");
    }
}

// ============================================================================
// TraceContext tests
// ============================================================================

mod trace_context_tests {
    use super::*;
    use std::collections::HashMap;

    fn make_headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Verify that `traceparent` header is extracted correctly.
    #[test]
    fn test_trace_context_from_headers_traceparent() {
        let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let headers = make_headers(&[("traceparent", tp)]);
        let ctx = TraceContext::from_headers(&headers);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().as_str(), tp);
    }

    /// Verify that `x-correlation-id` header is extracted when present.
    #[test]
    fn test_trace_context_from_headers_correlation_id() {
        let headers = make_headers(&[("x-correlation-id", "my-correlation-123")]);
        let ctx = TraceContext::from_headers(&headers);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().as_str(), "my-correlation-123");
    }

    /// Verify that `x-request-id` header is extracted when present.
    #[test]
    fn test_trace_context_from_headers_request_id() {
        let headers = make_headers(&[("x-request-id", "req-id-456")]);
        let ctx = TraceContext::from_headers(&headers);
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().as_str(), "req-id-456");
    }

    /// Verify that `traceparent` takes priority over `x-correlation-id` and `x-request-id`.
    #[test]
    fn test_trace_context_from_headers_priority_traceparent_wins() {
        let headers = make_headers(&[
            ("traceparent", "trace-val"),
            ("x-correlation-id", "corr-val"),
            ("x-request-id", "req-val"),
        ]);
        let ctx = TraceContext::from_headers(&headers).unwrap();
        assert_eq!(ctx.as_str(), "trace-val");
    }

    /// Verify that `x-correlation-id` takes priority over `x-request-id` when both present.
    #[test]
    fn test_trace_context_from_headers_priority_correlation_over_request() {
        let headers = make_headers(&[
            ("x-correlation-id", "corr-val"),
            ("x-request-id", "req-val"),
        ]);
        let ctx = TraceContext::from_headers(&headers).unwrap();
        assert_eq!(ctx.as_str(), "corr-val");
    }

    /// Verify that `None` is returned when no recognised trace headers are present.
    #[test]
    fn test_trace_context_no_matching_headers_returns_none() {
        let headers = make_headers(&[
            ("content-type", "application/json"),
            ("x-github-event", "push"),
        ]);
        let ctx = TraceContext::from_headers(&headers);
        assert!(ctx.is_none());
    }

    /// Verify that `None` is returned for an empty header map.
    #[test]
    fn test_trace_context_empty_headers_returns_none() {
        let headers = HashMap::new();
        let ctx = TraceContext::from_headers(&headers);
        assert!(ctx.is_none());
    }

    /// Verify that converting a TraceContext to CorrelationId preserves the value verbatim.
    #[test]
    fn test_trace_context_into_correlation_id_preserves_value() {
        let headers = make_headers(&[("x-correlation-id", "my-id-123")]);
        let ctx = TraceContext::from_headers(&headers).unwrap();
        let corr_id: CorrelationId = ctx.into();
        assert_eq!(corr_id.as_str(), "my-id-123");
    }

    /// Verify that a W3C traceparent value is preserved verbatim as the CorrelationId.
    #[test]
    fn test_trace_context_into_correlation_id_from_traceparent() {
        let tp = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let headers = make_headers(&[("traceparent", tp)]);
        let ctx = TraceContext::from_headers(&headers).unwrap();
        let corr_id: CorrelationId = ctx.into();
        assert_eq!(corr_id.as_str(), tp);
    }

    /// Verify that `Display` outputs the raw trace context string.
    #[test]
    fn test_trace_context_display() {
        let headers = make_headers(&[("x-correlation-id", "some-trace-value")]);
        let ctx = TraceContext::from_headers(&headers).unwrap();
        assert_eq!(ctx.to_string(), "some-trace-value");
    }

    /// Verify that `as_str()` returns the raw trace context string.
    #[test]
    fn test_trace_context_as_str() {
        let headers = make_headers(&[("x-request-id", "req-789")]);
        let ctx = TraceContext::from_headers(&headers).unwrap();
        assert_eq!(ctx.as_str(), "req-789");
    }
}
