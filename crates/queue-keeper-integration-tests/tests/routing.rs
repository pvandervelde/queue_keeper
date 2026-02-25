//! Integration tests for router creation and routing logic
//!
//! These tests verify that the API routes are configured correctly.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::create_test_app_state;
use tower::ServiceExt; // For `oneshot`

/// Verify that the router includes all expected routes
#[tokio::test]
async fn test_router_has_health_endpoint() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Route exists (not 404)
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Health endpoint should exist"
    );
}

/// Verify that the router includes webhook endpoint
#[tokio::test]
async fn test_router_has_webhook_endpoint() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .method("POST")
        .uri("/webhook/github")
        .header("x-github-event", "ping")
        .header("x-github-delivery", "12345678-1234-1234-1234-123456789012")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"zen":"test"}"#))
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Route exists (not 404)
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Webhook endpoint should exist for registered provider"
    );
}

/// Verify that the router includes metrics endpoint
#[tokio::test]
async fn test_router_has_metrics_endpoint() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Route exists (not 404)
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Metrics endpoint should exist"
    );
}

/// Verify that unknown routes return 404
#[tokio::test]
async fn test_router_returns_404_for_unknown_routes() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .uri("/nonexistent")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Verify that GET requests to webhook endpoint are rejected
#[tokio::test]
async fn test_webhook_endpoint_rejects_get_requests() {
    // Arrange
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    // The route is POST /webhook/{provider} — a GET to a known provider
    // path should be rejected with 405 Method Not Allowed.
    let request = Request::builder()
        .method("GET")
        .uri("/webhook/github")
        .body(Body::empty())
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: Should not allow GET
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
