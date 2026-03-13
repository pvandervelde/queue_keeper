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

/// Verify that readiness endpoint returns 200 when the service is ready
///
/// The readiness endpoint lives at `/ready` (not `/health/ready`).
/// Kubernetes readiness probes use this to decide whether to route traffic
/// to the pod.  With the default [`DefaultHealthChecker`] the service is
/// always considered ready once the HTTP server is listening.
#[tokio::test]
async fn test_readiness_endpoint_returns_200() {
    // Arrange
    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/ready")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Readiness endpoint should return 200"
    );
}

/// Verify that the readiness response body is valid JSON
#[tokio::test]
async fn test_readiness_endpoint_returns_json() {
    // Arrange
    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/ready")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: status 200 and JSON content-type
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .expect("content-type header should be present");
    assert!(
        content_type.to_str().unwrap().contains("application/json"),
        "Readiness response should be application/json, got: {:?}",
        content_type
    );
}

/// Verify that liveness endpoint returns 200
///
/// The liveness endpoint lives at `/health/live`.
/// A Kubernetes liveness probe uses this to decide whether to restart the
/// pod.  If the process can handle any HTTP request it is considered alive.
#[tokio::test]
async fn test_liveness_endpoint_returns_200() {
    // Arrange
    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/health/live")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Liveness endpoint should return 200"
    );
}

/// Verify that the liveness response body is valid JSON with status "alive"
#[tokio::test]
async fn test_liveness_endpoint_returns_alive_status() {
    // Arrange
    let state = create_test_app_state();
    let app = create_router(state);

    let request = Request::builder()
        .uri("/health/live")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Extract body
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Assert: status field is "alive" (not "healthy" or "unhealthy")
    assert_eq!(
        body["status"].as_str().unwrap_or(""),
        "alive",
        "Liveness response 'status' field should be 'alive'"
    );
}
