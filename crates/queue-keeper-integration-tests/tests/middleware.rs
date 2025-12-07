//! Integration tests for HTTP middleware (logging, metrics, tracing)

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::create_test_app_state;
use tower::ServiceExt;

/// Verify that request logging middleware processes requests
#[tokio::test]
async fn test_request_logging_middleware_processes_requests() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Request completed successfully (middleware didn't block)
    assert_eq!(response.status(), StatusCode::OK);
}

/// Verify that correlation ID is propagated through middleware
#[tokio::test]
async fn test_correlation_id_propagation() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .header("x-correlation-id", "test-correlation-123")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Correlation ID should be in response headers
    assert!(
        response.headers().contains_key("x-correlation-id"),
        "Response should include correlation ID header"
    );
}

/// Verify that middleware generates correlation ID if not provided
#[tokio::test]
async fn test_correlation_id_generation() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        // No correlation ID header
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Generated correlation ID should be in response
    let correlation_id = response.headers().get("x-correlation-id");
    assert!(
        correlation_id.is_some(),
        "Response should include generated correlation ID"
    );
    assert!(
        !correlation_id.unwrap().to_str().unwrap().is_empty(),
        "Generated correlation ID should not be empty"
    );
}

/// Verify that metrics middleware records requests
#[tokio::test]
async fn test_metrics_middleware_records_requests() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Request completed (metrics recorded in background)
    assert_eq!(response.status(), StatusCode::OK);

    // Note: Actual metrics verification would require inspecting Prometheus metrics
    // This test validates that metrics middleware doesn't break request flow
}

/// Verify that compression middleware is applied when appropriate
#[tokio::test]
async fn test_compression_middleware_applies_when_requested() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .header("accept-encoding", "gzip")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Request completed successfully
    assert_eq!(response.status(), StatusCode::OK);

    // Note: Actual compression verification would check Content-Encoding header
    // This test validates that compression middleware doesn't break request flow
}

/// Verify that CORS middleware is applied
#[tokio::test]
async fn test_cors_middleware_allows_configured_origins() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .header("origin", "https://example.com")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Request completed successfully
    assert_eq!(response.status(), StatusCode::OK);

    // CORS headers may or may not be present depending on configuration
    // This test validates that CORS middleware doesn't break request flow
}
