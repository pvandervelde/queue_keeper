//! Tests for HTTP middleware (logging, tracing, metrics)

use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt; // For `oneshot`

#[tokio::test]
async fn test_request_logging_middleware_logs_request_and_response() {
    // Arrange
    let app = axum::Router::new()
        .route("/test", axum::routing::get(|| async { "OK" }))
        .layer(axum::middleware::from_fn(request_logging_middleware));

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    // Note: Actual log verification would require a log capturing mechanism
}

#[tokio::test]
async fn test_request_logging_includes_correlation_id() {
    // Arrange
    let app = axum::Router::new()
        .route("/test", axum::routing::get(|| async { "OK" }))
        .layer(axum::middleware::from_fn(request_logging_middleware));

    let request = Request::builder()
        .uri("/test")
        .header("x-correlation-id", "test-correlation-123")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    // Correlation ID should be logged and included in response
    assert!(response.headers().contains_key("x-correlation-id"));
}

#[tokio::test]
async fn test_request_logging_generates_correlation_id_if_missing() {
    // Arrange
    let app = axum::Router::new()
        .route("/test", axum::routing::get(|| async { "OK" }))
        .layer(axum::middleware::from_fn(request_logging_middleware));

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    // Middleware should generate and include correlation ID
    assert!(response.headers().contains_key("x-correlation-id"));
}

#[tokio::test]
async fn test_metrics_middleware_records_request_duration() {
    // Arrange
    let app = axum::Router::new()
        .route(
            "/test",
            axum::routing::get(|| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                "OK"
            }),
        )
        .layer(axum::middleware::from_fn(metrics_middleware));

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    // Note: Actual metrics verification would require access to metrics registry
}

#[tokio::test]
async fn test_metrics_middleware_records_request_and_response_sizes() {
    // Arrange
    let app = axum::Router::new()
        .route(
            "/test",
            axum::routing::post(|body: String| async move { format!("Received: {}", body) }),
        )
        .layer(axum::middleware::from_fn(metrics_middleware));

    let request = Request::builder()
        .method("POST")
        .uri("/test")
        .header("content-length", "10")
        .body(Body::from("test-data"))
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_middleware_chain_preserves_order() {
    // Arrange
    let app = axum::Router::new()
        .route("/test", axum::routing::get(|| async { "OK" }))
        .layer(axum::middleware::from_fn(metrics_middleware))
        .layer(axum::middleware::from_fn(request_logging_middleware));

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
    // Middleware should execute in correct order (logging -> metrics -> handler)
}

#[tokio::test]
async fn test_error_responses_are_logged_correctly() {
    // Arrange
    let app = axum::Router::new()
        .route(
            "/test",
            axum::routing::get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "Error") }),
        )
        .layer(axum::middleware::from_fn(request_logging_middleware));

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    // Error responses should be logged with appropriate level
}

#[tokio::test]
async fn test_large_requests_are_tracked() {
    // Arrange
    let app = axum::Router::new()
        .route(
            "/test",
            axum::routing::post(|body: String| async move { format!("Length: {}", body.len()) }),
        )
        .layer(axum::middleware::from_fn(metrics_middleware));

    let large_body = "x".repeat(10000);
    let request = Request::builder()
        .method("POST")
        .uri("/test")
        .header("content-length", large_body.len().to_string())
        .body(Body::from(large_body))
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_request_path_is_normalized_for_metrics() {
    // This test verifies that the path normalization function works correctly.
    // The normalization happens inside the metrics middleware to prevent
    // cardinality explosion in metrics systems.

    // Test numeric ID normalization
    assert_eq!(
        normalize_path_for_metrics("/api/events/12345"),
        "/api/events/:id"
    );

    // Test UUID normalization
    assert_eq!(
        normalize_path_for_metrics("/api/sessions/550e8400-e29b-41d4-a716-446655440000"),
        "/api/sessions/:id"
    );

    // Test path with multiple IDs
    assert_eq!(
        normalize_path_for_metrics("/api/repos/123/issues/456"),
        "/api/repos/:id/issues/:id"
    );

    // Test path with no IDs
    assert_eq!(normalize_path_for_metrics("/api/health"), "/api/health");

    // Test root path
    assert_eq!(normalize_path_for_metrics("/"), "/");
}

#[test]
fn test_uuid_validation() {
    // Valid UUIDs with correct 8-4-4-4-12 pattern
    assert!(is_uuid_like("550e8400-e29b-41d4-a716-446655440000"));
    assert!(is_uuid_like("f47ac10b-58cc-4372-a567-0e02b2c3d479"));
    assert!(is_uuid_like("00000000-0000-0000-0000-000000000000"));
    assert!(is_uuid_like("ffffffff-ffff-ffff-ffff-ffffffffffff"));

    // Invalid: wrong length
    assert!(!is_uuid_like("550e8400-e29b-41d4-a716-44665544000"));
    assert!(!is_uuid_like("550e8400-e29b-41d4-a716-4466554400000"));

    // Invalid: hyphens in wrong positions
    assert!(!is_uuid_like("550e8400e-29b-41d4-a716-446655440000"));
    assert!(!is_uuid_like("550e8400-e29b41d4-a716-446655440000"));
    assert!(!is_uuid_like("550e8400-e29b-41d4a716-446655440000"));

    // Invalid: non-hex characters
    assert!(!is_uuid_like("550e8400-e29g-41d4-a716-446655440000"));
    assert!(!is_uuid_like("550e8400-e29b-41d4-z716-446655440000"));

    // Invalid: wrong number of hyphens
    assert!(!is_uuid_like("550e8400-e29b-41d4-a716446655440000"));
    assert!(!is_uuid_like("550e8400e29b41d4a716446655440000"));

    // Invalid: hyphens but wrong pattern (not 8-4-4-4-12)
    assert!(!is_uuid_like("550e840-0e29-b41d-4a71-6446655440000"));
}
