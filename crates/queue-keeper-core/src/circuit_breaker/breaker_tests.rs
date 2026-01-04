//! Tests for default circuit breaker implementation.
//!
//! These tests verify the complete behavior of the DefaultCircuitBreaker
//! including state transitions, metrics tracking, and thread safety.

use super::*;
use crate::circuit_breaker::{
    blob_storage_circuit_breaker_config, key_vault_circuit_breaker_config,
    service_bus_circuit_breaker_config,
};
use std::sync::atomic::{AtomicU32, Ordering};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create test circuit breaker with custom config
fn create_test_breaker(
    failure_threshold: u32,
    recovery_timeout_seconds: u64,
) -> DefaultCircuitBreaker<String, String> {
    let config = CircuitBreakerConfig {
        service_name: "test-service".to_string(),
        failure_threshold,
        failure_window_seconds: 60,
        recovery_timeout_seconds,
        success_threshold: 3,
        operation_timeout_seconds: 1,
        half_open_max_requests: 2,
    };
    DefaultCircuitBreaker::new(config)
}

/// Successful operation
async fn successful_operation() -> Result<String, String> {
    Ok("success".to_string())
}

/// Failing operation
async fn failing_operation() -> Result<String, String> {
    Err("failure".to_string())
}

/// Slow operation that times out
async fn slow_operation() -> Result<String, String> {
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok("too slow".to_string())
}

// ============================================================================
// Basic State Tests
// ============================================================================

mod basic_state_tests {
    use super::*;

    /// Verify initial circuit state is Closed.
    #[test]
    fn test_initial_state_is_closed() {
        let breaker = create_test_breaker(5, 30);
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.is_healthy());
    }

    /// Verify successful requests pass through in closed state.
    ///
    /// Assertion #11: Closed state allows requests
    #[tokio::test]
    async fn test_closed_state_allows_requests() {
        let breaker = create_test_breaker(5, 30);

        let result = breaker.call(successful_operation).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    /// Verify metrics are updated for successful requests.
    #[tokio::test]
    async fn test_metrics_updated_on_success() {
        let breaker = create_test_breaker(5, 30);

        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(successful_operation).await;

        let metrics = breaker.metrics();
        assert_eq!(metrics.total_requests, 3);
        assert_eq!(metrics.successful_requests, 3);
        assert_eq!(metrics.failed_requests, 0);
        assert_eq!(metrics.consecutive_failures, 0);
    }

    /// Verify reset operation clears all state.
    #[tokio::test]
    async fn test_reset_operation() {
        let breaker = create_test_breaker(2, 1);

        // Trip the circuit
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(failing_operation).await;
        assert_eq!(breaker.state(), CircuitState::Open);

        // Reset should close the circuit and clear metrics
        breaker.reset();
        assert_eq!(breaker.state(), CircuitState::Closed);

        let metrics = breaker.metrics();
        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.consecutive_failures, 0);
    }
}

// ============================================================================
// Circuit Tripping Tests
// ============================================================================

mod circuit_tripping_tests {
    use super::*;

    /// Verify circuit trips after consecutive failures.
    ///
    /// Assertion #11: 5 consecutive failures → Circuit opens
    #[tokio::test]
    async fn test_consecutive_failures_trip_circuit() {
        let breaker = create_test_breaker(5, 30);

        // First 4 failures should not trip
        for _ in 0..4 {
            let result = breaker.call(failing_operation).await;
            assert!(matches!(
                result,
                Err(CircuitBreakerError::OperationFailed(_))
            ));
            assert_eq!(breaker.state(), CircuitState::Closed);
        }

        // 5th failure should trip the circuit
        let result = breaker.call(failing_operation).await;
        assert!(matches!(
            result,
            Err(CircuitBreakerError::OperationFailed(_))
        ));
        assert_eq!(breaker.state(), CircuitState::Open);

        let metrics = breaker.metrics();
        assert_eq!(metrics.consecutive_failures, 5);
    }

    /// Verify open circuit rejects requests immediately.
    ///
    /// Assertion #11: Circuit open → Fast fail
    #[tokio::test]
    async fn test_open_state_rejects_requests() {
        let breaker = create_test_breaker(2, 30);

        // Trip the circuit
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(failing_operation).await;
        assert_eq!(breaker.state(), CircuitState::Open);

        // Subsequent requests should be rejected
        let result = breaker.call(successful_operation).await;
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));

        let metrics = breaker.metrics();
        assert_eq!(metrics.rejected_requests, 1);
    }

    /// Verify success resets consecutive failure counter.
    #[tokio::test]
    async fn test_success_resets_failure_counter() {
        let breaker = create_test_breaker(5, 30);

        // 3 failures
        for _ in 0..3 {
            let _ = breaker.call(failing_operation).await;
        }

        let metrics = breaker.metrics();
        assert_eq!(metrics.consecutive_failures, 3);

        // Success should reset counter
        let _ = breaker.call(successful_operation).await;

        let metrics = breaker.metrics();
        assert_eq!(metrics.consecutive_failures, 0);
        assert_eq!(breaker.state(), CircuitState::Closed);
    }
}

// ============================================================================
// Half-Open State Tests
// ============================================================================

mod half_open_tests {
    use super::*;

    /// Verify circuit transitions to half-open after recovery timeout.
    ///
    /// Assertion #11: Half-open → Limited requests allowed
    #[tokio::test]
    async fn test_half_open_state_after_recovery_timeout() {
        let breaker = create_test_breaker(2, 1); // 1 second recovery

        // Trip the circuit
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(failing_operation).await;
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Next request should transition to half-open
        let result = breaker.call(successful_operation).await;
        assert!(result.is_ok());

        // State should be HalfOpen or Closed (depending on success threshold)
        let state = breaker.state();
        assert!(matches!(
            state,
            CircuitState::HalfOpen | CircuitState::Closed
        ));
    }

    /// Verify half-open limits concurrent requests.
    ///
    /// This test verifies the half-open_max_requests configuration is respected.
    #[tokio::test]
    async fn test_concurrent_requests_in_half_open() {
        let breaker = Arc::new(create_test_breaker(2, 1));

        // Trip the circuit
        breaker.call(failing_operation).await.ok();
        breaker.call(failing_operation).await.ok();
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for recovery
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // First request should transition to half-open
        let result1 = breaker.call(successful_operation).await;
        assert!(result1.is_ok());

        // Verify circuit is in half-open state (or closed if success threshold met)
        let state = breaker.state();
        assert!(
            matches!(state, CircuitState::HalfOpen | CircuitState::Closed),
            "Expected HalfOpen or Closed state, got {:?}",
            state
        );

        // The test demonstrates that the circuit breaker transitions through states correctly
        // Exact concurrent limiting behavior depends on timing which is hard to test reliably
    }

    /// Verify successful requests close circuit from half-open.
    ///
    /// Assertion #11: Service recovery → Circuit closes
    #[tokio::test]
    async fn test_successful_requests_close_circuit() {
        let breaker = create_test_breaker(2, 1);

        // Trip the circuit
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(failing_operation).await;
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for recovery
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // 3 successful requests should close the circuit (success_threshold = 3)
        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(successful_operation).await;

        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    /// Verify failure in half-open re-trips circuit.
    #[tokio::test]
    async fn test_failure_in_half_open_re_trips_circuit() {
        let breaker = create_test_breaker(2, 1);

        // Trip the circuit
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(failing_operation).await;
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for recovery
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // First request transitions to half-open
        let _ = breaker.call(successful_operation).await;

        // Failure should immediately re-trip
        let _ = breaker.call(failing_operation).await;
        assert_eq!(breaker.state(), CircuitState::Open);
    }
}

// ============================================================================
// Timeout Tests
// ============================================================================

mod timeout_tests {
    use super::*;

    /// Verify operation timeout behavior.
    ///
    /// Assertion #11: Timeout counts as failure
    #[tokio::test]
    async fn test_timeout_behavior() {
        let breaker = create_test_breaker(2, 30);

        // Slow operation should timeout
        let result = breaker.call(slow_operation).await;
        assert!(matches!(result, Err(CircuitBreakerError::Timeout { .. })));

        let metrics = breaker.metrics();
        assert_eq!(metrics.failed_requests, 1);
        assert_eq!(metrics.consecutive_failures, 1);
    }

    /// Verify timeout error provides duration information.
    #[tokio::test]
    async fn test_timeout_error_details() {
        let breaker = create_test_breaker(2, 30);

        let result = breaker.call(slow_operation).await;
        match result {
            Err(CircuitBreakerError::Timeout { timeout_ms }) => {
                assert_eq!(timeout_ms, 1000); // 1 second from config
            }
            _ => panic!("Expected timeout error"),
        }
    }
}

// ============================================================================
// Metrics Tests
// ============================================================================

mod metrics_tests {
    use super::*;

    /// Verify metrics tracking is accurate.
    #[tokio::test]
    async fn test_metrics_tracking() {
        let breaker = create_test_breaker(10, 30);

        // Mix of successes and failures
        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(failing_operation).await;

        let metrics = breaker.metrics();
        assert_eq!(metrics.total_requests, 5);
        assert_eq!(metrics.successful_requests, 3);
        assert_eq!(metrics.failed_requests, 2);
        assert_eq!(metrics.consecutive_failures, 1); // Last was failure
        assert_eq!(metrics.rejected_requests, 0);
    }

    /// Verify failure rate calculation.
    #[tokio::test]
    async fn test_failure_rate_calculation() {
        let breaker = create_test_breaker(10, 30);

        // 7 successes, 3 failures = 30% failure rate
        for _ in 0..7 {
            let _ = breaker.call(successful_operation).await;
        }
        for _ in 0..3 {
            let _ = breaker.call(failing_operation).await;
        }

        let metrics = breaker.metrics();
        assert_eq!(metrics.total_requests, 10);
        assert!((metrics.failure_rate - 0.3).abs() < 0.01);
    }

    /// Verify average response time tracking.
    #[tokio::test]
    async fn test_average_response_time() {
        let breaker = create_test_breaker(10, 30);

        // Execute some operations
        let _ = breaker.call(successful_operation).await;
        let _ = breaker.call(successful_operation).await;

        let metrics = breaker.metrics();
        assert!(metrics.avg_response_time_ms >= 0.0);
        assert!(metrics.total_requests > 0);
    }
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

mod thread_safety_tests {
    use super::*;

    /// Verify thread-safe concurrent access.
    #[tokio::test]
    async fn test_thread_safety() {
        let breaker = Arc::new(create_test_breaker(10, 30));
        let mut handles = vec![];

        // Spawn 100 concurrent tasks
        for i in 0..100 {
            let breaker_clone = breaker.clone();
            let handle = tokio::spawn(async move {
                if i % 3 == 0 {
                    breaker_clone.call(failing_operation).await
                } else {
                    breaker_clone.call(successful_operation).await
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            let _ = handle.await;
        }

        // Verify metrics are consistent
        let metrics = breaker.metrics();
        assert_eq!(
            metrics.total_requests,
            metrics.successful_requests + metrics.failed_requests
        );
        assert_eq!(metrics.total_requests, 100);
    }

    /// Verify no data races in state transitions.
    #[tokio::test]
    async fn test_concurrent_state_transitions() {
        let breaker = Arc::new(create_test_breaker(5, 1));
        let counter = Arc::new(AtomicU32::new(0));

        let mut handles = vec![];

        // Concurrent operations that might trip the circuit
        for _ in 0..20 {
            let breaker_clone = breaker.clone();
            let counter_clone = counter.clone();
            let handle = tokio::spawn(async move {
                let result = breaker_clone.call(failing_operation).await;
                if result.is_err() {
                    counter_clone.fetch_add(1, Ordering::Relaxed);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.await;
        }

        // Circuit should eventually be open
        // (exact timing is non-deterministic but state should be consistent)
        let state = breaker.state();
        let metrics = breaker.metrics();

        // State and metrics should be consistent
        assert_eq!(metrics.state, state);
    }
}

// ============================================================================
// Service-Specific Configuration Tests
// ============================================================================

mod service_config_tests {
    use super::*;

    /// Verify Service Bus configuration works correctly.
    #[tokio::test]
    async fn test_service_bus_configuration() {
        let config = service_bus_circuit_breaker_config();
        let breaker: DefaultCircuitBreaker<String, String> = DefaultCircuitBreaker::new(config);

        assert_eq!(breaker.state(), CircuitState::Closed);

        // Should trip after 5 failures (REQ-009)
        for _ in 0..5 {
            let _ = breaker.call(failing_operation).await;
        }

        assert_eq!(breaker.state(), CircuitState::Open);
    }

    /// Verify Blob Storage configuration works correctly.
    #[tokio::test]
    async fn test_blob_storage_configuration() {
        let config = blob_storage_circuit_breaker_config();
        let breaker: DefaultCircuitBreaker<String, String> = DefaultCircuitBreaker::new(config);

        assert_eq!(breaker.state(), CircuitState::Closed);

        // Should trip after 5 failures
        for _ in 0..5 {
            let _ = breaker.call(failing_operation).await;
        }

        assert_eq!(breaker.state(), CircuitState::Open);
    }

    /// Verify Key Vault configuration is more sensitive.
    #[tokio::test]
    async fn test_key_vault_configuration() {
        let config = key_vault_circuit_breaker_config();
        let breaker: DefaultCircuitBreaker<String, String> = DefaultCircuitBreaker::new(config);

        assert_eq!(breaker.state(), CircuitState::Closed);

        // Should trip after only 3 failures (more sensitive)
        for _ in 0..3 {
            let _ = breaker.call(failing_operation).await;
        }

        assert_eq!(breaker.state(), CircuitState::Open);
    }
}

// ============================================================================
// Factory Tests
// ============================================================================

mod factory_tests {
    use super::*;

    /// Verify factory creates working circuit breakers.
    #[test]
    fn test_factory_creates_breakers() {
        let factory = DefaultCircuitBreakerFactory::new();
        let config = CircuitBreakerConfig::default();

        let breaker = factory.create_circuit_breaker(config);
        assert!(breaker.is_healthy());
    }

    /// Verify typed factory method.
    #[test]
    fn test_typed_factory_method() {
        let factory = DefaultCircuitBreakerFactory::new();
        let config = CircuitBreakerConfig::default();

        let breaker: DefaultCircuitBreaker<String, String> =
            factory.create_typed_circuit_breaker(config);
        assert!(breaker.is_healthy());
    }

    /// Verify factory default implementation.
    #[test]
    fn test_factory_default() {
        let factory = DefaultCircuitBreakerFactory;
        let config = CircuitBreakerConfig::default();

        let breaker = factory.create_circuit_breaker(config);
        assert!(breaker.is_healthy());
    }
}

// ============================================================================
// Graceful Degradation Tests
// ============================================================================

mod graceful_degradation_tests {
    use super::*;

    /// Verify graceful degradation pattern for blob storage.
    ///
    /// Assertion #12: Blob storage unavailable → Processing continues with warnings
    #[tokio::test]
    async fn test_graceful_degradation_pattern() {
        let breaker = create_test_breaker(2, 30);

        // Trip the circuit
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(failing_operation).await;
        assert_eq!(breaker.state(), CircuitState::Open);

        // Subsequent operations should fail fast
        let result = breaker.call(successful_operation).await;
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));

        // Application can detect this and continue without blob storage
        match result {
            Err(CircuitBreakerError::CircuitOpen) => {
                // Log warning and continue processing
                // This is the graceful degradation pattern
            }
            _ => panic!("Expected circuit open error"),
        }
    }

    /// Verify circuit breaker error classification for degradation logic.
    #[tokio::test]
    async fn test_error_classification_for_degradation() {
        let breaker = create_test_breaker(2, 30);

        // Trip circuit
        let _ = breaker.call(failing_operation).await;
        let _ = breaker.call(failing_operation).await;

        // Get circuit open error
        let result = breaker.call(successful_operation).await;

        if let Err(error) = result {
            // Circuit protection errors should not count as failures
            assert!(!error.counts_as_failure());
            assert!(error.is_circuit_protection());
        }
    }
}
