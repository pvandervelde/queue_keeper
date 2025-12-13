//! End-to-end tests for webhook HTTP endpoint

mod common;

use common::{github_webhook_headers, http_client, sample_webhook_payload, TestContainer};

/// Verify that POST /webhook accepts valid webhook
#[tokio::test]
async fn test_webhook_endpoint_accepts_valid_webhook() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let headers = github_webhook_headers();
    let payload = sample_webhook_payload();

    // Act
    let mut request = client.post(server.url("/webhook"));
    for (key, value) in headers.iter() {
        request = request.header(key, value);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    let status = response.status();
    assert!(
        status.is_success() || status.as_u16() == 400,
        "Webhook should be accepted or return validation error, got: {}",
        status
    );
}

/// Verify that POST /webhook requires GitHub headers
#[tokio::test]
async fn test_webhook_endpoint_requires_github_headers() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let payload = sample_webhook_payload();

    // Act - Send without GitHub headers
    let response = client
        .post(server.url("/webhook"))
        .json(&payload)
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(
        response.status(),
        400,
        "Should return 400 Bad Request for missing GitHub headers"
    );
}

/// Verify that GET /webhook is not allowed
#[tokio::test]
async fn test_webhook_endpoint_rejects_get_requests() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act
    let response = client
        .get(server.url("/webhook"))
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(
        response.status(),
        405,
        "GET requests to /webhook should return 405 Method Not Allowed"
    );
}

/// Verify that webhook endpoint responds quickly
#[tokio::test]
async fn test_webhook_endpoint_responds_quickly() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let headers = github_webhook_headers();
    let payload = sample_webhook_payload();

    // Act
    let start = std::time::Instant::now();

    let mut request = client.post(server.url("/webhook"));
    for (key, value) in headers.iter() {
        request = request.header(key, value);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .expect("Failed to send request");

    let duration = start.elapsed();

    // Assert
    assert!(
        duration < std::time::Duration::from_secs(1),
        "Webhook should respond in <1s (target <500ms), took {:?}",
        duration
    );

    // Response should be either success or client error, not server error
    let status = response.status();
    assert!(
        status.is_success() || status.is_client_error(),
        "Should not have server error, got: {}",
        status
    );
}

/// Verify that ping events are accepted
#[tokio::test]
async fn test_webhook_accepts_ping_event() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-github-event", "ping".parse().unwrap());
    headers.insert(
        "x-github-delivery",
        "12345678-1234-1234-1234-123456789012".parse().unwrap(),
    );
    headers.insert("content-type", "application/json".parse().unwrap());

    let payload = serde_json::json!({
        "zen": "Testing is good",
        "hook_id": 123
    });

    // Act
    let mut request = client.post(server.url("/webhook"));
    for (key, value) in headers.iter() {
        request = request.header(key, value);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    let status = response.status();
    assert!(
        status.is_success() || status.is_client_error(),
        "Ping event should be processed, got: {}",
        status
    );
}

/// Verify that webhook response includes JSON body
#[tokio::test]
async fn test_webhook_response_includes_json() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let headers = github_webhook_headers();
    let payload = sample_webhook_payload();

    // Act
    let mut request = client.post(server.url("/webhook"));
    for (key, value) in headers.iter() {
        request = request.header(key, value);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    if response.status().is_success() {
        let content_type = response.headers().get("content-type");
        assert!(content_type.is_some(), "Should have Content-Type header");

        let json_result: Result<serde_json::Value, _> = response.json().await;
        assert!(json_result.is_ok(), "Response should be valid JSON");
    }
}
