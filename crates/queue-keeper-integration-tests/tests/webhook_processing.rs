//! Integration tests for webhook processing
//!
//! These tests verify the webhook handler's immediate response pattern
//! by calling the API code directly (no HTTP layer).

mod common;

use axum::extract::State;
use bytes::Bytes;
use common::{
    create_test_app_state_with_processor, create_valid_webhook_headers, MockWebhookProcessor,
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
    let result = queue_keeper_api::handle_webhook(State(state), headers, body).await;
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
    let result = queue_keeper_api::handle_webhook(State(state), headers, body).await;

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
    let result = queue_keeper_api::handle_webhook(State(state), headers, body).await;

    // Assert
    assert!(result.is_ok(), "Expected successful response");
    let response = result.unwrap().0;

    assert!(
        !response.event_id.to_string().is_empty(),
        "Event ID should not be empty"
    );
    assert!(
        !response.session_id.to_string().is_empty(),
        "Session ID should not be empty"
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
    let result = queue_keeper_api::handle_webhook(State(state), headers, body).await;

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
    let result = queue_keeper_api::handle_webhook(State(state), headers, body).await;
    let response_time = start.elapsed();

    // Assert: Response returned very quickly for ping event
    assert!(result.is_ok(), "Expected successful ping response");
    assert!(
        response_time < Duration::from_millis(200),
        "Ping response took {}ms, expected <200ms",
        response_time.as_millis()
    );
}
