//! Integration tests for webhook processing
//!
//! These tests verify the webhook handler's immediate response pattern
//! by calling the API code directly (no HTTP layer).

mod common;

use axum::extract::{Path, State};
use bytes::Bytes;
use common::{
    create_test_app_state_with_processor, create_valid_webhook_headers,
    AlwaysFailingSignatureValidator, MockWebhookProcessor,
};
use std::sync::Arc;
use std::time::Duration;

/// Verify that webhook processing returns immediately after validation and normalization
///
/// This test validates Assertion #2: Response Time SLA
/// - Webhook processing completes within fast path (validation + normalization + storage)
/// - Response returned quickly (target <500ms)
#[tokio::test]
async fn test_webhook_processing_returns_immediately() {
    // Arrange: Create mock processor that simulates fast processing (50ms)
    let processor = MockWebhookProcessor::new();
    processor.set_delay(Duration::from_millis(50));

    let state = create_test_app_state_with_processor(Arc::new(processor.clone()));

    let headers = create_valid_webhook_headers();
    let body = Bytes::from(r#"{"action":"opened","number":123}"#);

    // Act: Measure response time
    let start = std::time::Instant::now();
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;
    let response_time = start.elapsed();

    // Assert: Response returned quickly (within 1 second, ideally <500ms)
    assert!(result.is_ok(), "Expected successful response");
    assert!(
        response_time < Duration::from_millis(1000),
        "Response took {}ms, expected <1000ms for fast path",
        response_time.as_millis()
    );

    // Assert: Webhook processor was called during fast path
    assert_eq!(
        processor.call_count(),
        1,
        "Expected one processor call during fast path"
    );
}

/// Verify that webhook processing returns error if validation fails
///
/// This test validates that validation errors are returned immediately
/// without spawning background tasks.
#[tokio::test]
async fn test_webhook_processing_returns_error_on_validation_failure() {
    // Arrange: Configure processor to return validation error
    let processor = MockWebhookProcessor::new();
    processor.set_error("Invalid signature".to_string());

    let state = create_test_app_state_with_processor(Arc::new(processor));

    let headers = create_valid_webhook_headers();
    let body = Bytes::from(r#"{"action":"opened"}"#);

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert: Error response returned immediately
    assert!(result.is_err(), "Expected error response");
}

/// Verify that webhook response includes event_id and session_id
///
/// This test validates that the response contains tracking identifiers
/// for correlation and monitoring.
#[tokio::test]
async fn test_webhook_response_includes_event_metadata() {
    // Arrange
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor));

    let headers = create_valid_webhook_headers();
    let body = Bytes::from(r#"{"action":"opened","number":123}"#);

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert
    assert!(result.is_ok(), "Expected successful response");
    let response = result.unwrap().0;

    assert!(
        !response.event_id.to_string().is_empty(),
        "Event ID should not be empty"
    );
    assert!(
        response.session_id.is_some(),
        "Session ID should be present"
    );
    assert_eq!(response.status, "processed", "Status should be 'processed'");
    assert!(
        response.message.contains("successfully"),
        "Message should indicate success"
    );
}

/// Verify that malformed headers result in immediate error response
///
/// This test validates input validation at the HTTP layer.
#[tokio::test]
async fn test_webhook_rejects_malformed_headers() {
    // Arrange
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor));

    // Create headers missing required GitHub webhook headers
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "content-type",
        axum::http::HeaderValue::from_static("application/json"),
    );
    // Missing X-GitHub-Event, X-GitHub-Delivery

    let body = Bytes::from(r#"{"action":"opened"}"#);

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert: Validation error returned immediately
    assert!(result.is_err(), "Expected error for malformed headers");
}

/// Verify that ping events are processed quickly
///
/// This test validates that ping events return immediately.
#[tokio::test]
async fn test_webhook_handles_ping_event_immediately() {
    // Arrange
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor.clone()));

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "x-github-event",
        axum::http::HeaderValue::from_static("ping"),
    );
    headers.insert(
        "x-github-delivery",
        axum::http::HeaderValue::from_static("12345678-1234-1234-1234-123456789012"),
    );
    headers.insert(
        "content-type",
        axum::http::HeaderValue::from_static("application/json"),
    );

    let body = Bytes::from(r#"{"zen":"Testing is good","hook_id":123}"#);

    // Act
    let start = std::time::Instant::now();
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;
    let response_time = start.elapsed();

    // Assert: Response returned very quickly for ping event
    assert!(result.is_ok(), "Expected successful ping response");
    assert!(
        response_time < Duration::from_millis(200),
        "Ping response took {}ms, expected <200ms",
        response_time.as_millis()
    );
}

// ============================================================================
// Audit Logging Integration Tests
// ============================================================================

/// Verify that webhook processing logs audit events on successful processing
///
/// Uses the real `WebhookProcessorImpl` (no signature validator, no payload
/// storer) with a `MockAuditLogger` so that `log_webhook_processing` is
/// called by the production code path.  The request body includes a valid
/// GitHub repository structure so that event normalisation succeeds and the
/// audit call is triggered.
#[tokio::test]
async fn test_webhook_processing_logs_audit_events() {
    // Arrange: real processor with audit logger, no signature validator or storer
    let audit_logger = Arc::new(common::MockAuditLogger::new());
    let processor = queue_keeper_core::webhook::WebhookProcessorImpl::new(
        None,
        None,
        Some(audit_logger.clone()),
    );
    let state = create_test_app_state_with_processor(Arc::new(processor));

    let headers = create_valid_webhook_headers();
    // Body must contain a repository so that normalisation can extract it
    // and trigger the audit call inside WebhookProcessorImpl::process_webhook.
    let body = Bytes::from(
        r#"{"action":"opened","number":123,"pull_request":{"number":123},"repository":{"id":1,"name":"repo","full_name":"owner/repo","private":false,"owner":{"login":"owner","id":1,"type":"User"}}}"#,
    );

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert: processing succeeded
    assert!(result.is_ok(), "Expected successful webhook response");

    // Assert: audit logger was called exactly once via log_webhook_processing
    assert_eq!(
        audit_logger.webhook_processing_call_count(),
        1,
        "Expected exactly one log_webhook_processing call for successful processing"
    );
}

/// Verify that failed signature validation logs a security audit event
///
/// Uses the real `WebhookProcessorImpl` with an `AlwaysFailingSignatureValidator`
/// and a `MockAuditLogger`.  The validator always rejects the signature, which
/// causes `WebhookProcessorImpl` to emit a security event via `log_event`
/// before returning an error.
#[tokio::test]
async fn test_failed_signature_validation_logs_security_event() {
    // Arrange: real processor with a failing validator and audit logger
    let audit_logger = Arc::new(common::MockAuditLogger::new());
    let validator = Arc::new(AlwaysFailingSignatureValidator);
    let processor = queue_keeper_core::webhook::WebhookProcessorImpl::new(
        Some(validator),
        None,
        Some(audit_logger.clone()),
    );
    let state = create_test_app_state_with_processor(Arc::new(processor));

    let headers = create_valid_webhook_headers(); // includes x-hub-signature-256 header
    let body = Bytes::from(r#"{"action":"opened"}"#);

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert: request was rejected as expected
    assert!(
        result.is_err(),
        "Expected error response for invalid signature"
    );

    // Assert: a security audit event was recorded via log_event
    assert_eq!(
        audit_logger.event_count(),
        1,
        "Expected exactly one security audit event for signature validation failure"
    );

    let events = audit_logger.get_logged_events();
    let security_event = &events[0];
    assert_eq!(
        security_event.event_type,
        queue_keeper_core::audit_logging::AuditEventType::Security,
        "Audit event type must be Security for signature validation failure"
    );
    assert!(
        security_event.result.is_error(),
        "Audit event result must indicate failure"
    );
    assert_eq!(
        security_event.result.get_error_code(),
        Some("webhook_signature_failure"),
        "Audit event must carry the canonical error code for signature failures"
    );
}

// ============================================================================
// Trace Context Propagation Integration Tests
// ============================================================================

/// Verify that a `traceparent` header is extracted from the HTTP request
/// and stored on the `WebhookRequest` received by the processor.
///
/// This validates that the API layer passes all headers to
/// `WebhookRequest::with_raw_headers`, which calls `TraceContext::from_headers`
/// internally, so the trace value is available downstream.
#[tokio::test]
async fn test_traceparent_header_is_propagated_to_webhook_request() {
    // Arrange: processor that captures the incoming WebhookRequest
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor.clone()));

    let mut headers = create_valid_webhook_headers();
    let trace_value = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    headers.insert(
        "traceparent",
        axum::http::HeaderValue::from_static(trace_value),
    );
    let body = Bytes::from(r#"{"action":"opened","number":123}"#);

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert: processing succeeded
    assert!(result.is_ok(), "Expected successful webhook response");

    // Assert: the processor received the WebhookRequest with trace_context set
    let calls = processor.get_calls();
    assert_eq!(calls.len(), 1, "Expected exactly one processor call");
    let request = &calls[0];
    assert!(
        request.trace_context.is_some(),
        "trace_context must be populated when traceparent header is present"
    );
    assert_eq!(
        request.trace_context.as_ref().map(|tc| tc.as_str()),
        Some(trace_value),
        "trace_context value must match the incoming traceparent header"
    );
}

/// Verify that when no trace headers are present the `trace_context` field
/// on the `WebhookRequest` is `None`.
#[tokio::test]
async fn test_no_trace_header_leaves_trace_context_none() {
    // Arrange: standard headers without any trace header
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor.clone()));

    let headers = create_valid_webhook_headers();
    let body = Bytes::from(r#"{"action":"opened","number":123}"#);

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert
    assert!(result.is_ok(), "Expected successful webhook response");

    let calls = processor.get_calls();
    assert_eq!(calls.len(), 1, "Expected exactly one processor call");
    assert!(
        calls[0].trace_context.is_none(),
        "trace_context must be None when no trace headers are present"
    );
}

/// Verify that `x-correlation-id` is used as trace context when no
/// `traceparent` header is present.
#[tokio::test]
async fn test_x_correlation_id_header_is_propagated_when_no_traceparent() {
    // Arrange
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor.clone()));

    let mut headers = create_valid_webhook_headers();
    let correlation_value = "my-correlation-id-12345";
    headers.insert(
        "x-correlation-id",
        axum::http::HeaderValue::from_static(correlation_value),
    );
    let body = Bytes::from(r#"{"action":"opened","number":123}"#);

    // Act
    let result = queue_keeper_api::handle_provider_webhook(
        State(state),
        Path("github".to_string()),
        headers,
        body,
    )
    .await;

    // Assert
    assert!(result.is_ok(), "Expected successful webhook response");

    let calls = processor.get_calls();
    assert_eq!(calls.len(), 1, "Expected exactly one processor call");
    assert_eq!(
        calls[0].trace_context.as_ref().map(|tc| tc.as_str()),
        Some(correlation_value),
        "trace_context must equal x-correlation-id when no traceparent is present"
    );
}
