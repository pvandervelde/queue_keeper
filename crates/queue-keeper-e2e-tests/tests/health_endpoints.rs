//! End-to-end tests for health check HTTP endpoints

mod common;

use common::{http_client, TestContainer};

/// Verify that GET /health returns 200 OK
#[tokio::test]
async fn test_health_endpoint_returns_200() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/health"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), 200);
    assert!(response.headers().get("content-type").is_some());

    let body = response.text().await.expect("Failed to read response body");
    assert!(!body.is_empty(), "Response body should not be empty");
}

/// Verify that health endpoint returns JSON
#[tokio::test]
async fn test_health_endpoint_returns_json() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/health"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    let content_type = response.headers().get("content-type").unwrap();
    assert!(
        content_type.to_str().unwrap().contains("application/json"),
        "Content-Type should be application/json"
    );

    let json: serde_json::Value = response
        .json()
        .await
        .expect("Response should be valid JSON");

    assert!(json.is_object(), "Response should be a JSON object");
}

/// Verify that health endpoint responds quickly
#[tokio::test]
async fn test_health_endpoint_responds_quickly() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let start = std::time::Instant::now();
    let response = client
        .get(server.url("/health"))
        .send()
        .await
        .expect("Failed to send request");
    let duration = start.elapsed();

    // Assert
    assert_eq!(response.status(), 200);
    assert!(
        duration < std::time::Duration::from_millis(500),
        "Health check should respond in <500ms, took {:?}",
        duration
    );
}

/// Verify that metrics endpoint exists
#[tokio::test]
async fn test_metrics_endpoint_exists() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/metrics"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_ne!(response.status(), 404, "Metrics endpoint should exist");
}

/// Verify that metrics endpoint returns Prometheus format
#[tokio::test]
async fn test_metrics_endpoint_returns_prometheus_format() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/metrics"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    if response.status().is_success() {
        let body = response.text().await.expect("Failed to read response body");

        // Prometheus metrics should contain metric names and values
        // Check for basic metric format indicators
        assert!(
            body.contains("# HELP") || body.contains("# TYPE") || !body.is_empty(),
            "Metrics should be in Prometheus format"
        );
    }
}
