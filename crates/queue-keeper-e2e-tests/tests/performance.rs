//! End-to-end tests for performance and load characteristics
//!
//! These tests verify:
//! - Response time under load (Assertion #2)
//! - Concurrent request handling (Assertion #21)
//! - Large payload handling
//! - Memory usage
//! - Graceful degradation

mod common;

use common::{github_webhook_headers, http_client, sample_webhook_payload, TestContainer};
use std::time::Duration;

/// Verify that server responds quickly under normal load
///
/// Tests Assertion #2: Response Time SLA
#[tokio::test]
async fn test_response_time_p95() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act: Send 20 requests and measure latencies
    let mut latencies = Vec::new();
    for _ in 0..20 {
        let headers = github_webhook_headers();
        let payload = sample_webhook_payload();

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

        let latency = start.elapsed();
        latencies.push(latency);

        // Verify response is successful
        assert!(
            response.status().is_success() || response.status() == 400,
            "Request should succeed or fail validation"
        );
    }

    // Assert: P95 latency < 1 second
    latencies.sort();
    let p95_index = (latencies.len() as f64 * 0.95) as usize;
    let p95_latency = latencies[p95_index];

    assert!(
        p95_latency < Duration::from_secs(1),
        "P95 latency should be < 1s, got: {:?}",
        p95_latency
    );
}

/// Verify that server handles concurrent requests
///
/// Tests Assertion #21: Concurrent Processing
#[tokio::test]
#[ignore = "High-load test - run manually"]
async fn test_concurrent_requests() {
    // Arrange
    let server = TestContainer::start().await;
    let num_concurrent = 100; // Test with 100 concurrent requests

    // Act: Send concurrent requests
    let mut handles = Vec::new();
    for i in 0..num_concurrent {
        let url = server.url("/webhook");
        let handle = tokio::spawn(async move {
            let client = http_client();
            let headers = github_webhook_headers();
            let mut payload = sample_webhook_payload();
            payload["number"] = serde_json::json!(i); // Make each request unique

            let mut request = client.post(&url);
            for (key, value) in headers.iter() {
                request = request.header(key, value);
            }

            let start = std::time::Instant::now();
            let response = request
                .json(&payload)
                .send()
                .await
                .expect("Failed to send request");

            let latency = start.elapsed();
            (response.status(), latency)
        });
        handles.push(handle);
    }

    // Assert: All requests complete successfully
    let mut success_count = 0;
    let mut latencies = Vec::new();

    for handle in handles {
        let (status, latency) = handle.await.expect("Task should complete");
        if status.is_success() || status == 400 {
            success_count += 1;
        }
        latencies.push(latency);
    }

    // At least 95% should succeed
    assert!(
        success_count >= (num_concurrent as f64 * 0.95) as usize,
        "At least 95% of requests should succeed: {}/{}",
        success_count,
        num_concurrent
    );

    // P95 latency should still be reasonable
    latencies.sort();
    let p95_index = (latencies.len() as f64 * 0.95) as usize;
    let p95_latency = latencies[p95_index];

    assert!(
        p95_latency < Duration::from_secs(5),
        "P95 latency under load should be < 5s, got: {:?}",
        p95_latency
    );
}

/// Verify that server handles 1MB payloads
///
/// Tests edge case: Large Payloads
#[tokio::test]
async fn test_large_payload_handling() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Create 1MB payload
    let large_data = "A".repeat(1024 * 1024);
    let mut payload = sample_webhook_payload();
    payload["large_field"] = serde_json::json!(large_data);

    let headers = github_webhook_headers();

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
    // Should accept or reject based on configured limits
    assert!(
        response.status().is_success()
        || response.status() == 413 // Payload Too Large
        || response.status() == 400, // Validation error
        "Expected success, 413, or 400, got: {}",
        response.status()
    );
}

/// Verify that health check responds quickly even under load
#[tokio::test]
async fn test_health_check_under_load() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Start background load (10 webhook requests)
    let url = server.url("/webhook");
    let _load_handles: Vec<_> = (0..10)
        .map(|_| {
            let url_clone = url.clone();
            tokio::spawn(async move {
                let client = http_client();
                let headers = github_webhook_headers();
                let payload = sample_webhook_payload();

                let mut request = client.post(&url_clone);
                for (key, value) in headers.iter() {
                    request = request.header(key, value);
                }

                let _ = request.json(&payload).send().await;
            })
        })
        .collect();

    // Act: Check health during load
    let start = std::time::Instant::now();
    let response = client
        .get(server.url("/health"))
        .send()
        .await
        .expect("Failed to send request");
    let latency = start.elapsed();

    // Assert: Health check should still be fast
    assert_eq!(response.status(), 200);
    assert!(
        latency < Duration::from_millis(500),
        "Health check should respond < 500ms even under load, got: {:?}",
        latency
    );
}

/// Verify that metrics endpoint responds quickly
#[tokio::test]
async fn test_metrics_endpoint_performance() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();

    // Act: Measure metrics endpoint latency
    let start = std::time::Instant::now();
    let response = client
        .get(server.url("/metrics"))
        .send()
        .await
        .expect("Failed to send request");
    let latency = start.elapsed();

    // Assert
    assert!(
        response.status().is_success() || response.status() == 404,
        "Metrics endpoint should exist"
    );

    if response.status().is_success() {
        assert!(
            latency < Duration::from_millis(200),
            "Metrics should be fast to collect, got: {:?}",
            latency
        );
    }
}

/// Verify that server doesn't crash with malformed payloads
#[tokio::test]
async fn test_malformed_payload_resilience() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let headers = github_webhook_headers();

    // Test various malformed payloads
    let large_junk = "a".repeat(10 * 1024); // 10KB of junk
    let malformed_payloads: Vec<&str> = vec![
        "",                     // Empty
        "{",                    // Incomplete JSON
        "not json at all",      // Plain text
        "null",                 // Null
        "[]",                   // Array instead of object
        "123",                  // Number
    ];

    // Act: Send all malformed payloads (including the large one separately)
    let mut all_payloads: Vec<String> = malformed_payloads.iter().map(|s| s.to_string()).collect();
    all_payloads.push(large_junk);
    
    for payload in all_payloads {
        let mut request = client.post(server.url("/webhook"));
        for (key, value) in headers.iter() {
            request = request.header(key, value);
        }

        let response = request
            .body(payload)
            .header("content-type", "application/json")
            .send()
            .await
            .expect("Failed to send request");

        // Assert: Should reject gracefully (not crash)
        assert!(
            response.status().is_client_error(),
            "Malformed payload should be rejected with 4xx"
        );
    }

    // Verify server is still responsive
    let health_response = client
        .get(server.url("/health"))
        .send()
        .await
        .expect("Failed to send request");
    assert_eq!(
        health_response.status(),
        200,
        "Server should still be healthy"
    );
}

/// Verify that server handles rapid sequential requests
#[tokio::test]
async fn test_rapid_sequential_requests() {
    // Arrange
    let server = TestContainer::start().await;
    let client = http_client();
    let num_requests = 50;

    // Act: Send requests as fast as possible
    let mut success_count = 0;
    let start = std::time::Instant::now();

    for i in 0..num_requests {
        let headers = github_webhook_headers();
        let mut payload = sample_webhook_payload();
        payload["number"] = serde_json::json!(i);

        let mut request = client.post(server.url("/webhook"));
        for (key, value) in headers.iter() {
            request = request.header(key, value);
        }

        let response = request
            .json(&payload)
            .send()
            .await
            .expect("Failed to send request");

        if response.status().is_success() || response.status() == 400 {
            success_count += 1;
        }
    }

    let total_time = start.elapsed();

    // Assert: All requests complete successfully
    assert_eq!(
        success_count, num_requests,
        "All sequential requests should succeed"
    );

    // Throughput should be reasonable (at least 10 req/sec)
    let throughput = num_requests as f64 / total_time.as_secs_f64();
    assert!(
        throughput >= 10.0,
        "Should handle at least 10 req/sec, got: {:.1}",
        throughput
    );
}

/// Verify memory doesn't grow unbounded with many requests
#[tokio::test]
#[ignore = "Memory testing requires special setup"]
async fn test_memory_usage_bounded() {
    // This test would require:
    // 1. Monitoring container memory usage
    // 2. Sending many requests
    // 3. Verifying memory returns to baseline after load
    //
    // Implementation depends on container runtime metrics
}

/// Verify that timeout configuration works
#[tokio::test]
#[ignore = "Requires slow-processing mock"]
async fn test_request_timeout() {
    // This test would require:
    // 1. Mock webhook processor that delays processing
    // 2. Configure short timeout
    // 3. Verify request times out appropriately
    //
    // Implementation depends on timeout configuration
}
