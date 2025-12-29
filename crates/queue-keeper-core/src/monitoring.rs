//! Metrics collection and monitoring.
//!
//! This module provides traits and types for collecting metrics from core business operations.
//! The traits are implemented by infrastructure layers (e.g., queue-keeper-api with Prometheus)
//! to maintain clean architecture boundaries.
//!
//! # Architecture
//!
//! - **Domain Layer** (this module): Defines what metrics to collect via traits
//! - **Infrastructure Layer** (queue-keeper-api): Implements traits with Prometheus
//! - **Best-Effort Pattern**: Metric failures never block business operations
//!
//! # Examples
//!
//! ```rust
//! use queue_keeper_core::monitoring::{MetricsCollector, NoOpMetricsCollector};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! // For production: inject Prometheus-backed collector
//! // For tests: use NoOpMetricsCollector
//! let metrics: Arc<dyn MetricsCollector> = Arc::new(NoOpMetricsCollector);
//!
//! // Record webhook processing
//! metrics.record_webhook_request(Duration::from_millis(150), true);
//!
//! // Record validation failure
//! metrics.record_webhook_validation_failure();
//!
//! // Record queue routing
//! metrics.record_queue_routing(Duration::from_millis(50), 3);
//! ```

use async_trait::async_trait;
use std::time::Duration;

/// Metrics collector for domain operations.
///
/// This trait defines metrics collection interface for core business operations.
/// Implementations should never fail - use best-effort recording where metrics
/// failures don't impact business logic.
///
/// # Thread Safety
///
/// All methods take `&self` to support `Arc<dyn MetricsCollector>` sharing
/// across async tasks. Implementations must be thread-safe.
///
/// # Best-Effort Pattern
///
/// Metric recording failures should be logged but never propagate errors
/// to business operations. The system must remain operational even if
/// metrics collection fails.
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Record a webhook request processing.
    ///
    /// # Parameters
    ///
    /// - `duration`: Time spent processing the webhook
    /// - `success`: Whether the webhook was processed successfully
    ///
    /// # Metrics Updated
    ///
    /// - `webhook_requests_total`: Incremented by 1
    /// - `webhook_duration_seconds`: Histogram observation
    /// - `webhook_validation_failures`: Incremented if !success
    fn record_webhook_request(&self, duration: Duration, success: bool);

    /// Record a webhook signature validation failure.
    ///
    /// Called when HMAC-SHA256 signature validation fails.
    ///
    /// # Metrics Updated
    ///
    /// - `webhook_validation_failures`: Incremented by 1
    fn record_webhook_validation_failure(&self);

    /// Record queue routing operation.
    ///
    /// # Parameters
    ///
    /// - `duration`: Time spent routing to all queues
    /// - `queue_count`: Number of queues routed to
    ///
    /// # Metrics Updated
    ///
    /// - `webhook_queue_routing_duration`: Histogram observation
    fn record_queue_routing(&self, duration: Duration, queue_count: usize);

    /// Record a queue delivery attempt.
    ///
    /// # Parameters
    ///
    /// - `success`: Whether delivery succeeded to all target queues
    ///
    /// # Metrics Updated
    ///
    /// - Counter for successful/failed deliveries
    fn record_queue_delivery_attempt(&self, success: bool);

    /// Record an error occurrence.
    ///
    /// # Parameters
    ///
    /// - `category`: Error category (e.g., "4xx", "5xx", "network")
    /// - `is_transient`: Whether the error is transient (retriable)
    ///
    /// # Metrics Updated
    ///
    /// - `error_rate_by_category`: Incremented with category label
    fn record_error(&self, category: &str, is_transient: bool);

    /// Record circuit breaker state.
    ///
    /// # Parameters
    ///
    /// - `service`: Service name (e.g., "github", "queue", "storage")
    /// - `state`: Circuit state (0=closed, 1=open, 2=half-open)
    ///
    /// # Metrics Updated
    ///
    /// - `circuit_breaker_state`: Gauge set to state value
    fn record_circuit_breaker_state(&self, service: &str, state: i64);

    /// Record a retry attempt.
    ///
    /// # Parameters
    ///
    /// - `service`: Service being retried (e.g., "queue", "storage")
    ///
    /// # Metrics Updated
    ///
    /// - `retry_attempts_total`: Incremented by 1
    fn record_retry_attempt(&self, service: &str);

    /// Record a blob storage failure.
    ///
    /// Called when webhook payload storage fails.
    ///
    /// # Metrics Updated
    ///
    /// - `blob_storage_failures`: Incremented by 1
    fn record_blob_storage_failure(&self);

    /// Record queue depth.
    ///
    /// # Parameters
    ///
    /// - `queue_name`: Name of the queue
    /// - `depth`: Current number of messages in queue
    ///
    /// # Metrics Updated
    ///
    /// - `queue_depth_messages`: Gauge set to depth value
    fn record_queue_depth(&self, queue_name: &str, depth: i64);

    /// Record dead letter queue depth.
    ///
    /// # Parameters
    ///
    /// - `depth`: Current number of messages in DLQ
    ///
    /// # Metrics Updated
    ///
    /// - `dead_letter_queue_depth`: Gauge set to depth value
    fn record_dead_letter_queue_depth(&self, depth: i64);

    /// Record a session ordering violation.
    ///
    /// Called when events with the same session ID are processed out of order.
    ///
    /// # Metrics Updated
    ///
    /// - `session_ordering_violations`: Incremented by 1
    fn record_session_ordering_violation(&self);

    /// Record queue processing rate.
    ///
    /// # Parameters
    ///
    /// - `rate`: Messages processed per minute
    ///
    /// # Metrics Updated
    ///
    /// - `queue_processing_rate`: Gauge set to rate value
    fn record_queue_processing_rate(&self, rate: f64);
}

/// No-op metrics collector for testing.
///
/// This implementation silently ignores all metric recording calls,
/// making it suitable for unit tests that don't need metrics validation.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::monitoring::{MetricsCollector, NoOpMetricsCollector};
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// let metrics: Arc<dyn MetricsCollector> = Arc::new(NoOpMetricsCollector);
///
/// // All calls are no-ops, never fail
/// metrics.record_webhook_request(Duration::from_secs(1), true);
/// metrics.record_error("test", true);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpMetricsCollector;

#[async_trait]
impl MetricsCollector for NoOpMetricsCollector {
    fn record_webhook_request(&self, _duration: Duration, _success: bool) {
        // No-op
    }

    fn record_webhook_validation_failure(&self) {
        // No-op
    }

    fn record_queue_routing(&self, _duration: Duration, _queue_count: usize) {
        // No-op
    }

    fn record_queue_delivery_attempt(&self, _success: bool) {
        // No-op
    }

    fn record_error(&self, _category: &str, _is_transient: bool) {
        // No-op
    }

    fn record_circuit_breaker_state(&self, _service: &str, _state: i64) {
        // No-op
    }

    fn record_retry_attempt(&self, _service: &str) {
        // No-op
    }

    fn record_blob_storage_failure(&self) {
        // No-op
    }

    fn record_queue_depth(&self, _queue_name: &str, _depth: i64) {
        // No-op
    }

    fn record_dead_letter_queue_depth(&self, _depth: i64) {
        // No-op
    }

    fn record_session_ordering_violation(&self) {
        // No-op
    }

    fn record_queue_processing_rate(&self, _rate: f64) {
        // No-op
    }
}

#[cfg(test)]
#[path = "monitoring_tests.rs"]
mod tests;
