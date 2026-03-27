//! End-to-end tests for admin API endpoints
//!
//! These tests verify:
//! - Configuration endpoints (GET /admin/config)
//! - Log level management (GET/PUT /admin/logging/level)
//! - Trace sampling (GET/PUT /admin/tracing/sampling)
//! - Metrics reset (POST /admin/metrics/reset)
//! - Event replay (POST /admin/events/:id/replay)
//! - Session reset (POST /admin/sessions/:id/reset)

mod common;

use common::{http_client, TestContainer};
use serde_json::json;

/// Verify that GET /admin/config returns current configuration
#[tokio::test]
async fn test_get_configuration() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/admin/config"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 200);

    let config: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    // Verify config structure
    assert!(config.get("server").is_some());
    assert!(config.get("webhooks").is_some());
    assert!(config.get("security").is_some());
    assert!(config.get("logging").is_some());
}

/// Verify that GET /admin/logging/level returns current log level
#[tokio::test]
async fn test_get_log_level() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/admin/logging/level"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 200);

    let level: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    assert!(level.get("level").is_some());
}

/// Verify that PUT /admin/logging/level updates log level
#[tokio::test]
async fn test_set_log_level() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .put(server.url("/admin/logging/level"))
        .json(&json!({"level": "debug"}))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 200);

    let level: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(level["level"], "debug");
}

/// Verify that PUT /admin/logging/level rejects invalid levels
#[tokio::test]
async fn test_set_invalid_log_level() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .put(server.url("/admin/logging/level"))
        .json(&json!({"level": "invalid"}))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 400);
}

/// Verify that GET /admin/tracing/sampling returns sampling configuration
#[tokio::test]
async fn test_get_trace_sampling() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/admin/tracing/sampling"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 200);

    let sampling: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    assert!(sampling.get("sampling_ratio").is_some());
    assert!(sampling.get("service_name").is_some());
}

/// Verify that PUT /admin/tracing/sampling updates sampling ratio
#[tokio::test]
async fn test_set_trace_sampling() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .put(server.url("/admin/tracing/sampling"))
        .json(&json!({"sampling_ratio": 0.5}))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 200);

    let sampling: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(sampling["sampling_ratio"], 0.5);
}

/// Verify that PUT /admin/tracing/sampling rejects invalid ratios
#[tokio::test]
async fn test_set_invalid_trace_sampling() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act - Try ratio > 1.0
    let response = client
        .put(server.url("/admin/tracing/sampling"))
        .json(&json!({"sampling_ratio": 1.5}))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 400);
}

/// Verify that POST /admin/metrics/reset works
#[tokio::test]
async fn test_reset_metrics() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .post(server.url("/admin/metrics/reset"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 200);

    let result: serde_json::Value = response.json().await.expect("Failed to parse JSON");

    assert_eq!(result["status"], "success");
}

/// Verify that POST /admin/events/:id/replay returns proper response
#[tokio::test]
#[ignore = "Requires event replay implementation"]
async fn test_replay_event() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let event_id = "01HQXYZ123456789ABCDEFG";

    // Act
    let response = client
        .post(server.url(&format!("/admin/events/{}/replay", event_id)))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    // Should return 200 if event exists, 404 if not
    assert!(response.status().is_success() || response.status() == 404);
}

/// Verify that POST /admin/sessions/:id/reset returns proper response
#[tokio::test]
#[ignore = "Requires session reset implementation"]
async fn test_reset_session() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let session_id = "pr-123";

    // Act
    let response = client
        .post(server.url(&format!("/admin/sessions/{}/reset", session_id)))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    // Should return 200 if session exists, 404 if not
    assert!(response.status().is_success() || response.status() == 404);
}

/// Verify that admin endpoints require authentication when an admin API key
/// is configured.
///
/// Starts the container with `QK__SECURITY__ADMIN_API_KEY` set so that the
/// admin auth middleware is active, then verifies:
/// 1. An unauthenticated request returns 401.
/// 2. A request with the correct bearer token succeeds.
#[tokio::test]
async fn test_admin_endpoints_require_auth() {
    // Arrange: start container with admin auth enabled
    let server =
        TestContainer::start_with_env(vec![("QK__SECURITY__ADMIN_API_KEY", "e2e-test-admin-key")])
            .await;
    let client = http_client();

    // Act 1 – unauthenticated request
    let unauth_response = client
        .get(server.url("/admin/config"))
        .send()
        .await
        .expect("Failed to send unauthenticated request");

    // Assert 1 – must be rejected
    assert_eq!(
        unauth_response.status(),
        401,
        "Unauthenticated admin request must return 401"
    );

    // Act 2 – authenticated request
    let auth_response = client
        .get(server.url("/admin/config"))
        .header("Authorization", "Bearer e2e-test-admin-key")
        .send()
        .await
        .expect("Failed to send authenticated request");

    // Assert 2 – must succeed
    assert!(
        auth_response.status().is_success(),
        "Authenticated admin request must succeed, got {}",
        auth_response.status()
    );
}

/// Verify that admin API returns consistent JSON error format
#[tokio::test]
async fn test_admin_api_error_format() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act - Trigger an error (invalid log level)
    let response = client
        .put(server.url("/admin/logging/level"))
        .json(&json!({"level": "invalid"}))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 400);

    // Error should be in JSON format (not HTML or plain text)
    let content_type = response.headers().get("content-type");
    if let Some(ct) = content_type {
        let ct_str = ct.to_str().unwrap_or("");
        assert!(
            ct_str.contains("application/json"),
            "Expected JSON content type, got: {}",
            ct_str
        );
    }
}
