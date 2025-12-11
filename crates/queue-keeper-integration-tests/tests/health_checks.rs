//! Integration tests for health check functionality

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{create_test_app_state, MockHealthChecker};
use queue_keeper_api::create_router;
use std::sync::Arc;
use tower::ServiceExt;

/// Verify that health endpoint returns 200 when system is healthy
#[tokio::test]
async fn test_health_endpoint_returns_200_when_healthy() {
    // Arrange
    let health_checker = Arc::new(MockHealthChecker::new());
    health_checker.set_healthy(true);

    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
}

/// Verify that health endpoint returns proper response structure
#[tokio::test]
async fn test_health_endpoint_response_structure() {
    // Arrange
    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    // Response body should be JSON
    let content_type = response.headers().get("content-type");
    assert!(content_type.is_some());
    let content_type_str = content_type.unwrap().to_str().unwrap();
    assert!(
        content_type_str.contains("application/json"),
        "Content-Type should be application/json, got: {}",
        content_type_str
    );
}

/// Verify that readiness endpoint exists
#[tokio::test]
async fn test_readiness_endpoint_exists() {
    // Arrange
    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/health/ready")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Route exists (may return different status codes based on readiness)
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Readiness endpoint should exist"
    );
}

/// Verify that liveness endpoint exists
#[tokio::test]
async fn test_liveness_endpoint_exists() {
    // Arrange
    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/health/live")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Route exists
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Liveness endpoint should exist"
    );
}
