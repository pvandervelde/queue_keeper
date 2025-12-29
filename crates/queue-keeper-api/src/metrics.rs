//! Metrics collection and observability types for the API service.

use prometheus::{Gauge, Histogram, IntCounter, IntCounterVec, IntGauge, IntGaugeVec};
use std::sync::Arc;

/// Service metrics for observability
#[derive(Debug)]
pub struct ServiceMetrics {
    // HTTP request metrics
    pub http_requests_total: IntCounter,
    pub http_request_duration: Histogram,
    pub http_request_size: Histogram,
    pub http_response_size: Histogram,

    // Webhook processing metrics
    pub webhook_requests_total: IntCounter,
    pub webhook_duration_seconds: Histogram,
    pub webhook_validation_failures: IntCounter,
    pub webhook_queue_routing_duration: Histogram,

    // Queue management metrics
    pub queue_depth_messages: IntGaugeVec,
    pub queue_processing_rate: Gauge,
    pub dead_letter_queue_depth: IntGauge,
    pub session_ordering_violations: IntCounter,

    // Bot-specific metrics
    pub events_processed_per_bot: IntCounterVec,
    pub queue_send_errors_total: IntCounter,
    pub active_sessions: IntGauge,

    // Replay and administrative operations
    pub replay_operations_total: IntCounter,
    pub replay_events_processed: IntCounter,
    pub replay_failures_total: IntCounter,

    // Error and security metrics
    pub error_rate_by_category: IntCounterVec,
    pub circuit_breaker_state: IntGaugeVec,
    pub retry_attempts_total: IntCounterVec,
    pub blob_storage_failures: IntCounter,
    pub signature_validation_failures: IntCounter,
    pub authentication_failures_total: IntCounter,
}

impl ServiceMetrics {
    pub fn new() -> Result<Arc<Self>, prometheus::Error> {
        use prometheus::{
            register_gauge, register_histogram, register_int_counter, register_int_counter_vec,
            register_int_gauge, register_int_gauge_vec,
        };

        Ok(Arc::new(Self {
            http_requests_total: register_int_counter!(
                "http_requests_total",
                "Total number of HTTP requests",
            )?,
            http_request_duration: register_histogram!(
                "http_request_duration_seconds",
                "HTTP request processing time",
                vec![0.001, 0.01, 0.1, 1.0, 10.0]
            )?,
            http_request_size: register_histogram!(
                "http_request_size_bytes",
                "HTTP request size in bytes",
                vec![100.0, 1000.0, 10000.0, 100000.0, 1000000.0]
            )?,
            http_response_size: register_histogram!(
                "http_response_size_bytes",
                "HTTP response size in bytes",
                vec![100.0, 1000.0, 10000.0, 100000.0, 1000000.0]
            )?,

            webhook_requests_total: register_int_counter!(
                "webhook_requests_total",
                "Total webhook requests received"
            )?,
            webhook_duration_seconds: register_histogram!(
                "webhook_duration_seconds",
                "Webhook processing time distribution",
                vec![0.001, 0.01, 0.1, 0.5, 1.0, 2.0, 5.0]
            )?,
            webhook_validation_failures: register_int_counter!(
                "webhook_validation_failures",
                "Invalid signature/payload count"
            )?,
            webhook_queue_routing_duration: register_histogram!(
                "webhook_queue_routing_duration_seconds",
                "Time to route to all bot queues",
                vec![0.001, 0.01, 0.1, 0.2, 0.5, 1.0]
            )?,

            queue_depth_messages: register_int_gauge_vec!(
                "queue_depth_messages",
                "Messages waiting in each bot queue",
                &["queue_name"]
            )?,
            queue_processing_rate: register_gauge!(
                "queue_processing_rate",
                "Messages processed per minute"
            )?,
            dead_letter_queue_depth: register_int_gauge!(
                "dead_letter_queue_depth",
                "Failed messages requiring attention"
            )?,
            session_ordering_violations: register_int_counter!(
                "session_ordering_violations",
                "Events processed out of order"
            )?,

            events_processed_per_bot: register_int_counter_vec!(
                "events_processed_per_bot",
                "Events routed to each bot queue",
                &["bot_name"]
            )?,
            queue_send_errors_total: register_int_counter!(
                "queue_send_errors_total",
                "Failed queue send operations"
            )?,
            active_sessions: register_int_gauge!(
                "active_sessions",
                "Number of active message sessions"
            )?,

            replay_operations_total: register_int_counter!(
                "replay_operations_total",
                "Total replay operations initiated"
            )?,
            replay_events_processed: register_int_counter!(
                "replay_events_processed",
                "Events processed during replay operations"
            )?,
            replay_failures_total: register_int_counter!(
                "replay_failures_total",
                "Failed replay operations"
            )?,

            error_rate_by_category: register_int_counter_vec!(
                "error_rate_by_category",
                "Errors grouped by category and transience",
                &["category", "transient"]
            )?,
            circuit_breaker_state: register_int_gauge_vec!(
                "circuit_breaker_state",
                "Service circuit breaker status",
                &["service"]
            )?,
            retry_attempts_total: register_int_counter_vec!(
                "retry_attempts_total",
                "Retry operations by service",
                &["service"]
            )?,
            blob_storage_failures: register_int_counter!(
                "blob_storage_failures",
                "Audit trail storage failures"
            )?,
            signature_validation_failures: register_int_counter!(
                "signature_validation_failures",
                "Failed webhook signature validations"
            )?,
            authentication_failures_total: register_int_counter!(
                "authentication_failures_total",
                "Failed authentication attempts"
            )?,
        }))
    }

    pub fn record_http_request(
        &self,
        duration: std::time::Duration,
        request_size: u64,
        response_size: u64,
    ) {
        self.http_requests_total.inc();
        self.http_request_duration.observe(duration.as_secs_f64());
        self.http_request_size.observe(request_size as f64);
        self.http_response_size.observe(response_size as f64);
    }

    pub fn record_webhook_request(&self, duration: std::time::Duration, success: bool) {
        self.webhook_requests_total.inc();
        self.webhook_duration_seconds
            .observe(duration.as_secs_f64());
        if !success {
            self.webhook_validation_failures.inc();
        }
    }
}

// Implement MetricsCollector trait from queue-keeper-core
impl queue_keeper_core::monitoring::MetricsCollector for ServiceMetrics {
    fn record_webhook_request(&self, duration: std::time::Duration, success: bool) {
        self.webhook_requests_total.inc();
        self.webhook_duration_seconds
            .observe(duration.as_secs_f64());
        if !success {
            self.webhook_validation_failures.inc();
        }
    }

    fn record_webhook_validation_failure(&self) {
        self.webhook_validation_failures.inc();
        self.signature_validation_failures.inc();
    }

    fn record_queue_routing(&self, duration: std::time::Duration, queue_count: usize) {
        self.webhook_queue_routing_duration
            .observe(duration.as_secs_f64());
        // Increment counter for each bot queue routed to
        // Use "multiple" label when routing to multiple queues
        let bot_label = if queue_count <= 1 {
            "single"
        } else {
            "multiple"
        };
        self.events_processed_per_bot
            .with_label_values(&[bot_label])
            .inc_by(queue_count as u64);
    }

    fn record_queue_delivery_attempt(&self, success: bool) {
        // Track via error_rate_by_category for failed deliveries
        if !success {
            self.error_rate_by_category
                .with_label_values(&["queue_delivery", "false"])
                .inc();
            self.queue_send_errors_total.inc();
        }
    }

    fn record_error(&self, category: &str, is_transient: bool) {
        let transient_label = if is_transient { "true" } else { "false" };
        self.error_rate_by_category
            .with_label_values(&[category, transient_label])
            .inc();
    }

    fn record_circuit_breaker_state(&self, service: &str, state: i64) {
        self.circuit_breaker_state
            .with_label_values(&[service])
            .set(state);
    }

    fn record_retry_attempt(&self, service: &str) {
        self.retry_attempts_total
            .with_label_values(&[service])
            .inc();
    }

    fn record_blob_storage_failure(&self) {
        self.blob_storage_failures.inc();
    }

    fn record_queue_depth(&self, queue_name: &str, depth: i64) {
        self.queue_depth_messages
            .with_label_values(&[queue_name])
            .set(depth);
    }

    fn record_dead_letter_queue_depth(&self, depth: i64) {
        self.dead_letter_queue_depth.set(depth);
    }

    fn record_session_ordering_violation(&self) {
        self.session_ordering_violations.inc();
    }

    fn record_queue_processing_rate(&self, rate: f64) {
        self.queue_processing_rate.set(rate);
    }
}

impl Default for ServiceMetrics {
    fn default() -> Self {
        // This is a stub implementation for testing
        // In production, use ServiceMetrics::new() instead
        use prometheus::{
            register_gauge, register_histogram, register_int_counter, register_int_counter_vec,
            register_int_gauge, register_int_gauge_vec,
        };

        // Use unique names with timestamp to avoid registration conflicts in tests
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        Self {
            http_requests_total: register_int_counter!(
                format!("http_requests_total_test_{}", suffix),
                "Test HTTP requests"
            )
            .unwrap(),
            http_request_duration: register_histogram!(
                format!("http_request_duration_seconds_test_{}", suffix),
                "Test HTTP duration",
                vec![]
            )
            .unwrap(),
            http_request_size: register_histogram!(
                format!("http_request_size_bytes_test_{}", suffix),
                "Test HTTP request size",
                vec![]
            )
            .unwrap(),
            http_response_size: register_histogram!(
                format!("http_response_size_bytes_test_{}", suffix),
                "Test HTTP response size",
                vec![]
            )
            .unwrap(),
            webhook_requests_total: register_int_counter!(
                format!("webhook_requests_total_test_{}", suffix),
                "Test webhook requests"
            )
            .unwrap(),
            webhook_duration_seconds: register_histogram!(
                format!("webhook_duration_seconds_test_{}", suffix),
                "Test webhook duration",
                vec![]
            )
            .unwrap(),
            webhook_validation_failures: register_int_counter!(
                format!("webhook_validation_failures_test_{}", suffix),
                "Test webhook validation failures"
            )
            .unwrap(),
            webhook_queue_routing_duration: register_histogram!(
                format!("webhook_queue_routing_duration_seconds_test_{}", suffix),
                "Test webhook queue routing duration",
                vec![]
            )
            .unwrap(),
            queue_depth_messages: register_int_gauge_vec!(
                format!("queue_depth_messages_test_{}", suffix),
                "Test queue depth",
                &["queue_name"]
            )
            .unwrap(),
            queue_processing_rate: register_gauge!(
                format!("queue_processing_rate_test_{}", suffix),
                "Test queue processing rate"
            )
            .unwrap(),
            dead_letter_queue_depth: register_int_gauge!(
                format!("dead_letter_queue_depth_test_{}", suffix),
                "Test DLQ depth"
            )
            .unwrap(),
            session_ordering_violations: register_int_counter!(
                format!("session_ordering_violations_test_{}", suffix),
                "Test session ordering violations"
            )
            .unwrap(),
            error_rate_by_category: register_int_counter_vec!(
                format!("error_rate_by_category_test_{}", suffix),
                "Test error rate",
                &["category", "transient"]
            )
            .unwrap(),
            circuit_breaker_state: register_int_gauge_vec!(
                format!("circuit_breaker_state_test_{}", suffix),
                "Test circuit breaker state",
                &["service"]
            )
            .unwrap(),
            retry_attempts_total: register_int_counter_vec!(
                format!("retry_attempts_total_test_{}", suffix),
                "Test retry attempts",
                &["service"]
            )
            .unwrap(),
            blob_storage_failures: register_int_counter!(
                format!("blob_storage_failures_test_{}", suffix),
                "Test blob storage failures"
            )
            .unwrap(),
            events_processed_per_bot: register_int_counter_vec!(
                format!("events_processed_per_bot_test_{}", suffix),
                "Test events per bot",
                &["bot_name"]
            )
            .unwrap(),
            queue_send_errors_total: register_int_counter!(
                format!("queue_send_errors_total_test_{}", suffix),
                "Test queue send errors"
            )
            .unwrap(),
            active_sessions: register_int_gauge!(
                format!("active_sessions_test_{}", suffix),
                "Test active sessions"
            )
            .unwrap(),
            replay_operations_total: register_int_counter!(
                format!("replay_operations_total_test_{}", suffix),
                "Test replay operations"
            )
            .unwrap(),
            replay_events_processed: register_int_counter!(
                format!("replay_events_processed_test_{}", suffix),
                "Test replay events processed"
            )
            .unwrap(),
            replay_failures_total: register_int_counter!(
                format!("replay_failures_total_test_{}", suffix),
                "Test replay failures"
            )
            .unwrap(),
            signature_validation_failures: register_int_counter!(
                format!("signature_validation_failures_test_{}", suffix),
                "Test signature validation failures"
            )
            .unwrap(),
            authentication_failures_total: register_int_counter!(
                format!("authentication_failures_total_test_{}", suffix),
                "Test authentication failures"
            )
            .unwrap(),
        }
    }
}

/// OpenTelemetry configuration for distributed tracing
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Service name for tracing
    pub service_name: String,

    /// Service version
    pub service_version: String,

    /// Environment (dev, staging, prod)
    pub environment: String,

    /// Trace sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,

    /// Enable JSON logging
    pub json_logging: bool,

    /// Current log level
    pub log_level: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "queue-keeper".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: "development".to_string(),
            sampling_ratio: 1.0, // 100% sampling in development
            json_logging: false,
            log_level: "info".to_string(),
        }
    }
}

impl TelemetryConfig {
    pub fn new(service_name: String, environment: String) -> Self {
        let is_production = environment == "production";
        Self {
            service_name,
            sampling_ratio: if is_production { 0.1 } else { 1.0 },
            json_logging: is_production,
            environment,
            ..Default::default()
        }
    }

    pub fn set_log_level(&mut self, level: String) -> Result<(), String> {
        match level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {
                self.log_level = level;
                Ok(())
            }
            _ => Err(format!("Invalid log level: {}", level)),
        }
    }

    pub fn set_sampling_ratio(&mut self, ratio: f64) -> Result<(), String> {
        if (0.0..=1.0).contains(&ratio) {
            self.sampling_ratio = ratio;
            Ok(())
        } else {
            Err("Sampling ratio must be between 0.0 and 1.0".to_string())
        }
    }
}
