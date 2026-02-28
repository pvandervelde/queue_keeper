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

/// Verify that an unregistered provider path returns 404
///
/// When a POST arrives at `/webhook/{provider}` and no processor is registered
/// under that provider ID, the handler must return HTTP 404 so callers know the
/// endpoint does not exist rather than receiving a misleading error.
#[tokio::test]
async fn test_unregistered_provider_returns_not_found() {
    // Arrange: default app state only registers "github"
    let state = create_test_app_state();
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .method("POST")
        .uri("/webhook/jira") // not registered
        .header("x-github-event", "push")
        .header("x-github-delivery", "12345678-1234-1234-1234-123456789012")
        .header("x-hub-signature-256", "sha256=abc123")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"test":"data"}"#))
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: unregistered provider → 404 NOT FOUND
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Unregistered provider should return 404"
    );
}

/// Verify that a non-GitHub provider endpoint accepts webhooks when registered
///
/// Registering a mock processor under "slack" must make `/webhook/slack`
/// reachable and return a success status, proving the routing layer is
/// provider-agnostic.
#[tokio::test]
async fn test_registered_generic_provider_accepts_webhook() {
    use common::{create_test_app_state_with_providers, MockWebhookProcessor};
    use std::sync::Arc;

    // Arrange: register both "github" and "slack"
    let github_processor = Arc::new(MockWebhookProcessor::new());
    let slack_processor = Arc::new(MockWebhookProcessor::new());

    let state = create_test_app_state_with_providers(vec![
        (
            "github".to_string(),
            github_processor as Arc<dyn queue_keeper_core::webhook::WebhookProcessor>,
        ),
        (
            "slack".to_string(),
            slack_processor as Arc<dyn queue_keeper_core::webhook::WebhookProcessor>,
        ),
    ]);
    let app = queue_keeper_api::create_router(state);

    let request = Request::builder()
        .method("POST")
        .uri("/webhook/slack")
        .header("x-github-event", "push")
        .header("x-github-delivery", "12345678-1234-1234-1234-123456789012")
        .header("x-hub-signature-256", "sha256=abc123")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"text":"Hello from Slack"}"#))
        .unwrap();

    // Act
    let response = app.oneshot(request).await.unwrap();

    // Assert: registered "slack" provider should respond successfully (not 404/405)
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Registered slack provider endpoint should exist"
    );
    assert_ne!(
        response.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "POST to webhook endpoint should be allowed"
    );
}

/// Verify that routing dispatches to the correct processor per provider
///
/// When multiple provider processors are registered, a request to
/// `/webhook/slack` must invoke only the Slack processor, and a request to
/// `/webhook/github` must invoke only the GitHub processor.
#[tokio::test]
async fn test_provider_routing_dispatches_to_correct_processor() {
    use common::{create_test_app_state_with_providers, MockWebhookProcessor};
    use std::sync::Arc;

    // Arrange: two processors with distinct call counters
    let github_processor = Arc::new(MockWebhookProcessor::new());
    let slack_processor = Arc::new(MockWebhookProcessor::new());
    let github_clone = github_processor.clone();
    let slack_clone = slack_processor.clone();

    let state = create_test_app_state_with_providers(vec![
        (
            "github".to_string(),
            github_processor as Arc<dyn queue_keeper_core::webhook::WebhookProcessor>,
        ),
        (
            "slack".to_string(),
            slack_processor as Arc<dyn queue_keeper_core::webhook::WebhookProcessor>,
        ),
    ]);
    let app = queue_keeper_api::create_router(state);

    let slack_request = Request::builder()
        .method("POST")
        .uri("/webhook/slack")
        .header("x-github-event", "push")
        .header("x-github-delivery", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        .header("x-hub-signature-256", "sha256=abc123")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"text":"Hello"}"#))
        .unwrap();

    // Act: send to /webhook/slack
    let response = app.oneshot(slack_request).await.unwrap();

    // Assert: Slack processor was called; GitHub was not
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Slack endpoint should resolve"
    );
    assert_eq!(
        slack_clone.call_count(),
        1,
        "Slack processor should have been called exactly once"
    );
    assert_eq!(
        github_clone.call_count(),
        0,
        "GitHub processor must not be called when routing to slack endpoint"
    );
}
