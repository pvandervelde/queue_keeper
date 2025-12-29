//! Tests for metrics collection module.

use super::*;
use std::sync::Arc;

/// Test that NoOpMetricsCollector can be created and used.
#[test]
fn test_noop_collector_creation() {
    let collector = NoOpMetricsCollector;
    let _arc: Arc<dyn MetricsCollector> = Arc::new(collector);
}

/// Test that NoOpMetricsCollector is thread-safe.
#[test]
fn test_noop_collector_thread_safety() {
    let collector: Arc<dyn MetricsCollector> = Arc::new(NoOpMetricsCollector);

    // Clone for another thread
    let collector_clone = Arc::clone(&collector);

    // Verify both can be used
    collector.record_webhook_request(Duration::from_secs(1), true);
    collector_clone.record_webhook_request(Duration::from_secs(1), false);
}

/// Test that NoOpMetricsCollector handles all webhook metrics.
#[test]
fn test_noop_collector_webhook_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_webhook_request(Duration::from_millis(150), true);
    collector.record_webhook_request(Duration::from_millis(250), false);
    collector.record_webhook_validation_failure();
}

/// Test that NoOpMetricsCollector handles queue routing metrics.
#[test]
fn test_noop_collector_queue_routing_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_queue_routing(Duration::from_millis(50), 3);
    collector.record_queue_routing(Duration::from_millis(100), 5);
}

/// Test that NoOpMetricsCollector handles queue delivery metrics.
#[test]
fn test_noop_collector_queue_delivery_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_queue_delivery_attempt(true);
    collector.record_queue_delivery_attempt(false);
}

/// Test that NoOpMetricsCollector handles error metrics.
#[test]
fn test_noop_collector_error_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_error("4xx", false);
    collector.record_error("5xx", true);
    collector.record_error("network", true);
}

/// Test that NoOpMetricsCollector handles circuit breaker metrics.
#[test]
fn test_noop_collector_circuit_breaker_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_circuit_breaker_state("github", 0); // closed
    collector.record_circuit_breaker_state("queue", 1); // open
    collector.record_circuit_breaker_state("storage", 2); // half-open
}

/// Test that NoOpMetricsCollector handles retry metrics.
#[test]
fn test_noop_collector_retry_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_retry_attempt("queue");
    collector.record_retry_attempt("storage");
    collector.record_retry_attempt("github");
}

/// Test that NoOpMetricsCollector handles blob storage metrics.
#[test]
fn test_noop_collector_blob_storage_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_blob_storage_failure();
    collector.record_blob_storage_failure();
}

/// Test that NoOpMetricsCollector handles queue depth metrics.
#[test]
fn test_noop_collector_queue_depth_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_queue_depth("bot1-queue", 100);
    collector.record_queue_depth("bot2-queue", 50);
    collector.record_dead_letter_queue_depth(10);
}

/// Test that NoOpMetricsCollector handles session metrics.
#[test]
fn test_noop_collector_session_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_session_ordering_violation();
    collector.record_session_ordering_violation();
}

/// Test that NoOpMetricsCollector handles processing rate metrics.
#[test]
fn test_noop_collector_processing_rate_metrics() {
    let collector = NoOpMetricsCollector;

    // Should not panic or fail
    collector.record_queue_processing_rate(100.5);
    collector.record_queue_processing_rate(250.0);
}

/// Test that NoOpMetricsCollector implements Default.
#[test]
fn test_noop_collector_default() {
    let collector = NoOpMetricsCollector::default();

    // Should work with default instance
    collector.record_webhook_request(Duration::from_secs(1), true);
}

/// Test that NoOpMetricsCollector can be cloned.
#[test]
fn test_noop_collector_clone() {
    let collector = NoOpMetricsCollector;
    let cloned = collector.clone();

    // Both should work
    collector.record_webhook_request(Duration::from_secs(1), true);
    cloned.record_webhook_request(Duration::from_secs(1), false);
}

/// Test that NoOpMetricsCollector can be used in async context.
#[tokio::test]
async fn test_noop_collector_async_usage() {
    let collector: Arc<dyn MetricsCollector> = Arc::new(NoOpMetricsCollector);

    // Simulate async webhook processing
    let start = std::time::Instant::now();
    tokio::time::sleep(Duration::from_millis(10)).await;
    let duration = start.elapsed();

    // Should work in async context
    collector.record_webhook_request(duration, true);
}

/// Test that NoOpMetricsCollector handles concurrent access.
#[tokio::test]
async fn test_noop_collector_concurrent_access() {
    let collector: Arc<dyn MetricsCollector> = Arc::new(NoOpMetricsCollector);

    // Spawn multiple tasks recording metrics
    let mut handles = vec![];

    for i in 0..10 {
        let collector_clone = Arc::clone(&collector);
        let handle = tokio::spawn(async move {
            collector_clone.record_webhook_request(Duration::from_millis(i * 10), true);
            collector_clone.record_error("test", false);
            collector_clone.record_retry_attempt("test");
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task should complete");
    }

    // Should not panic or deadlock
}

/// Test that MetricsCollector trait is object-safe.
#[test]
fn test_metrics_collector_object_safety() {
    let _collector: Box<dyn MetricsCollector> = Box::new(NoOpMetricsCollector);
    let _collector: Arc<dyn MetricsCollector> = Arc::new(NoOpMetricsCollector);
}

/// Test webhook request recording with various durations.
#[test]
fn test_webhook_request_various_durations() {
    let collector = NoOpMetricsCollector;

    // Test various duration ranges
    collector.record_webhook_request(Duration::from_nanos(500), true);
    collector.record_webhook_request(Duration::from_micros(100), true);
    collector.record_webhook_request(Duration::from_millis(50), true);
    collector.record_webhook_request(Duration::from_secs(1), false);
    collector.record_webhook_request(Duration::from_secs(10), false);
}

/// Test error recording with various categories.
#[test]
fn test_error_various_categories() {
    let collector = NoOpMetricsCollector;

    // Test various error categories
    collector.record_error("400", false);
    collector.record_error("401", false);
    collector.record_error("403", false);
    collector.record_error("404", false);
    collector.record_error("500", true);
    collector.record_error("502", true);
    collector.record_error("503", true);
    collector.record_error("timeout", true);
    collector.record_error("connection", true);
}

/// Test circuit breaker state transitions.
#[test]
fn test_circuit_breaker_state_transitions() {
    let collector = NoOpMetricsCollector;

    // Simulate state transitions
    collector.record_circuit_breaker_state("test-service", 0); // closed
    collector.record_circuit_breaker_state("test-service", 1); // open
    collector.record_circuit_breaker_state("test-service", 2); // half-open
    collector.record_circuit_breaker_state("test-service", 0); // closed again
}

/// Test queue depth tracking.
#[test]
fn test_queue_depth_tracking() {
    let collector = NoOpMetricsCollector;

    // Test various queue depths
    collector.record_queue_depth("queue1", 0);
    collector.record_queue_depth("queue2", 100);
    collector.record_queue_depth("queue3", 10000);
    collector.record_dead_letter_queue_depth(0);
    collector.record_dead_letter_queue_depth(50);
    collector.record_dead_letter_queue_depth(1000);
}

/// Test processing rate tracking.
#[test]
fn test_processing_rate_tracking() {
    let collector = NoOpMetricsCollector;

    // Test various processing rates
    collector.record_queue_processing_rate(0.0);
    collector.record_queue_processing_rate(10.5);
    collector.record_queue_processing_rate(100.0);
    collector.record_queue_processing_rate(1000.75);
}
