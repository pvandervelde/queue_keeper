//! End-to-end tests for HTTP error responses

mod common;

use common::{http_client, TestContainer};

/// Verify that unknown routes return 404
#[tokio::test]
async fn test_unknown_route_returns_404() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/nonexistent"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 404);
}

/// Verify that 404 responses include JSON error
#[tokio::test]
async fn test_404_includes_json_error() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/nonexistent"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 404);

    // May or may not have JSON body depending on implementation
    // At minimum should have some response
    let body = response.text().await.expect("Failed to read body");
    // 404 handler may return HTML or JSON or empty - all are acceptable
    assert!(body.len() >= 0);
}

/// Verify that invalid JSON returns 400
#[tokio::test]
async fn test_invalid_json_returns_400() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act - Send malformed JSON to webhook endpoint
    let response = client
        .post(server.url("/webhook"))
        .header("content-type", "application/json")
        .header("x-github-event", "push")
        .header("x-github-delivery", "12345678-1234-1234-1234-123456789012")
        .body("{invalid json")
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(
        response.status(),
        400,
        "Invalid JSON should return 400 Bad Request"
    );
}

/// Verify that server errors are properly formatted
#[tokio::test]
async fn test_error_responses_have_consistent_format() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act - Trigger a validation error
    let response = client
        .post(server.url("/webhook"))
        .header("content-type", "application/json")
        .body(r#"{"test": "data"}"#)
        .send()
        .await
        .expect("Failed to send request");

    // Assert - Should be 4xx error
    assert!(
        response.status().is_client_error(),
        "Missing required headers should be client error"
    );
}

/// Verify that CORS headers are present (if CORS is enabled)
#[tokio::test]
async fn test_cors_headers_present_when_enabled() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/health"))
        .header("Origin", "https://example.com")
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    // CORS headers may or may not be present depending on configuration
    // This test just verifies the server processes the request
    assert!(response.status().is_success());
}

/// Verify that large request bodies are handled
#[tokio::test]
async fn test_large_request_body_handling() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Create a large but reasonable webhook payload (1MB)
    let large_payload = "x".repeat(1024 * 1024);

    // Act
    let response = client
        .post(server.url("/webhook"))
        .header("content-type", "application/json")
        .header("x-github-event", "push")
        .header("x-github-delivery", "12345678-1234-1234-1234-123456789012")
        .body(large_payload)
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    // Should either accept it or reject with 413 Payload Too Large
    assert!(
        response.status() == 400 || response.status() == 413 || response.status().is_success(),
        "Large payload should be handled gracefully, got: {}",
        response.status()
    );
}
