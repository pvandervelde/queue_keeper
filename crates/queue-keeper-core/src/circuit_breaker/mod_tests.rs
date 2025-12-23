//! Tests for circuit breaker types and traits.
//!
//! These are contract tests that verify the behavior of circuit breaker
//! types and configurations independent of any specific implementation.

use super::*;

// ============================================================================
// CircuitState Tests
// ============================================================================

mod circuit_state_tests {
    use super::*;

    /// Verify that Closed and HalfOpen states allow requests.
    ///
    /// Assertion #11: Circuit states control request flow
    #[test]
    fn test_circuit_state_allows_requests() {
        assert!(CircuitState::Closed.allows_requests());
        assert!(CircuitState::HalfOpen.allows_requests());
        assert!(!CircuitState::Open.allows_requests());
    }

    /// Verify that Open and HalfOpen are considered failure states.
    #[test]
    fn test_circuit_state_is_failure_state() {
        assert!(!CircuitState::Closed.is_failure_state());
        assert!(CircuitState::Open.is_failure_state());
        assert!(CircuitState::HalfOpen.is_failure_state());
    }

    /// Verify CircuitState serialization for observability.
    #[test]
    fn test_circuit_state_serialization() {
        let closed = CircuitState::Closed;
        let json = serde_json::to_string(&closed).expect("Failed to serialize");
        let deserialized: CircuitState =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(closed, deserialized);
    }

    /// Verify CircuitState equality comparisons.
    #[test]
    fn test_circuit_state_equality() {
        assert_eq!(CircuitState::Closed, CircuitState::Closed);
        assert_ne!(CircuitState::Closed, CircuitState::Open);
        assert_ne!(CircuitState::Open, CircuitState::HalfOpen);
    }
}

// ============================================================================
// CircuitBreakerConfig Tests
// ============================================================================

mod circuit_breaker_config_tests {
    use super::*;

    /// Verify default configuration values match REQ-009.
    ///
    /// REQ-009: 5 failures, 30-second cooldown
    #[test]
    fn test_circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();

        assert_eq!(config.service_name, "unknown");
        assert_eq!(config.failure_threshold, 5); // REQ-009
        assert_eq!(config.failure_window_seconds, 60);
        assert_eq!(config.recovery_timeout_seconds, 30); // REQ-009
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.operation_timeout_seconds, 10);
        assert_eq!(config.half_open_max_requests, 5);
    }

    /// Verify service-specific configurations are properly tuned.
    #[test]
    fn test_service_bus_circuit_breaker_config() {
        let config = service_bus_circuit_breaker_config();

        assert_eq!(config.service_name, "azure-service-bus");
        assert_eq!(config.failure_threshold, 5); // REQ-009
        assert_eq!(config.recovery_timeout_seconds, 30); // REQ-009
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.operation_timeout_seconds, 5);
        assert_eq!(config.half_open_max_requests, 3); // Conservative
    }

    /// Verify blob storage configuration.
    #[test]
    fn test_blob_storage_circuit_breaker_config() {
        let config = blob_storage_circuit_breaker_config();

        assert_eq!(config.service_name, "azure-blob-storage");
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout_seconds, 30);
        assert_eq!(config.success_threshold, 2); // Lower for storage
        assert_eq!(config.operation_timeout_seconds, 10); // Longer timeout
        assert_eq!(config.half_open_max_requests, 5); // More testing allowed
    }

    /// Verify key vault configuration is more sensitive.
    #[test]
    fn test_key_vault_circuit_breaker_config() {
        let config = key_vault_circuit_breaker_config();

        assert_eq!(config.service_name, "azure-key-vault");
        assert_eq!(config.failure_threshold, 3); // More sensitive for security
        assert_eq!(config.recovery_timeout_seconds, 60); // Longer cooldown
        assert_eq!(config.success_threshold, 2);
        assert_eq!(config.operation_timeout_seconds, 5);
        assert_eq!(config.half_open_max_requests, 2); // Very conservative
    }

    /// Verify configuration serialization for persistence.
    #[test]
    fn test_circuit_breaker_config_serialization() {
        let config = CircuitBreakerConfig::default();
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: CircuitBreakerConfig =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(config.service_name, deserialized.service_name);
        assert_eq!(config.failure_threshold, deserialized.failure_threshold);
        assert_eq!(
            config.recovery_timeout_seconds,
            deserialized.recovery_timeout_seconds
        );
    }
}

// ============================================================================
// CircuitMetrics Tests
// ============================================================================

mod circuit_metrics_tests {
    use super::*;

    /// Verify success rate calculation with no requests.
    #[test]
    fn test_circuit_metrics_success_rate_no_requests() {
        let metrics = CircuitMetrics {
            state: CircuitState::Closed,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            rejected_requests: 0,
            consecutive_failures: 0,
            last_state_change: Timestamp::now(),
            next_recovery_attempt: None,
            failure_rate: 0.0,
            avg_response_time_ms: 0.0,
        };

        assert_eq!(metrics.success_rate(), 1.0);
    }

    /// Verify success rate calculation with requests.
    #[test]
    fn test_circuit_metrics_success_rate_with_requests() {
        let metrics = CircuitMetrics {
            state: CircuitState::Closed,
            total_requests: 100,
            successful_requests: 75,
            failed_requests: 25,
            rejected_requests: 0,
            consecutive_failures: 2,
            last_state_change: Timestamp::now(),
            next_recovery_attempt: None,
            failure_rate: 0.25,
            avg_response_time_ms: 150.0,
        };

        assert_eq!(metrics.success_rate(), 0.75);
    }

    /// Verify should_trip threshold logic.
    ///
    /// Assertion #11: 5 consecutive failures trigger circuit trip
    #[test]
    fn test_circuit_metrics_should_trip() {
        let mut metrics = CircuitMetrics {
            state: CircuitState::Closed,
            total_requests: 10,
            successful_requests: 5,
            failed_requests: 5,
            rejected_requests: 0,
            consecutive_failures: 4,
            last_state_change: Timestamp::now(),
            next_recovery_attempt: None,
            failure_rate: 0.5,
            avg_response_time_ms: 200.0,
        };

        // Below threshold
        assert!(!metrics.should_trip(5));

        // At threshold
        metrics.consecutive_failures = 5;
        assert!(metrics.should_trip(5));

        // Above threshold
        metrics.consecutive_failures = 10;
        assert!(metrics.should_trip(5));
    }

    /// Verify metrics serialization for monitoring systems.
    #[test]
    fn test_circuit_metrics_serialization() {
        let metrics = CircuitMetrics {
            state: CircuitState::Open,
            total_requests: 100,
            successful_requests: 50,
            failed_requests: 50,
            rejected_requests: 10,
            consecutive_failures: 5,
            last_state_change: Timestamp::now(),
            next_recovery_attempt: Some(Timestamp::now()),
            failure_rate: 0.5,
            avg_response_time_ms: 250.0,
        };

        let json = serde_json::to_string(&metrics).expect("Failed to serialize");
        let deserialized: CircuitMetrics =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(metrics.state, deserialized.state);
        assert_eq!(metrics.total_requests, deserialized.total_requests);
        assert_eq!(
            metrics.consecutive_failures,
            deserialized.consecutive_failures
        );
    }
}

// ============================================================================
// CircuitBreakerError Tests
// ============================================================================

mod circuit_breaker_error_tests {
    use super::*;

    /// Verify error classification for failure counting.
    #[test]
    fn test_circuit_breaker_error_counts_as_failure() {
        // Errors that count as failures
        assert!(
            CircuitBreakerError::<String>::OperationFailed("error".to_string()).counts_as_failure()
        );
        assert!(CircuitBreakerError::<String>::Timeout { timeout_ms: 1000 }.counts_as_failure());
        assert!(CircuitBreakerError::<String>::InternalError {
            message: "error".to_string()
        }
        .counts_as_failure());

        // Errors that don't count as failures (circuit protection)
        assert!(!CircuitBreakerError::<String>::CircuitOpen.counts_as_failure());
        assert!(!CircuitBreakerError::<String>::TooManyConcurrentRequests.counts_as_failure());
    }

    /// Verify circuit protection error detection.
    #[test]
    fn test_circuit_breaker_error_is_circuit_protection() {
        // Circuit protection errors
        assert!(CircuitBreakerError::<String>::CircuitOpen.is_circuit_protection());
        assert!(CircuitBreakerError::<String>::TooManyConcurrentRequests.is_circuit_protection());

        // Not circuit protection errors
        assert!(
            !CircuitBreakerError::<String>::OperationFailed("error".to_string())
                .is_circuit_protection()
        );
        assert!(
            !CircuitBreakerError::<String>::Timeout { timeout_ms: 1000 }.is_circuit_protection()
        );
        assert!(!CircuitBreakerError::<String>::InternalError {
            message: "error".to_string()
        }
        .is_circuit_protection());
    }

    /// Verify error display formatting for logging.
    #[test]
    fn test_circuit_breaker_error_display() {
        let error = CircuitBreakerError::<String>::CircuitOpen;
        assert_eq!(
            error.to_string(),
            "Circuit breaker is open - requests rejected"
        );

        let error = CircuitBreakerError::<String>::Timeout { timeout_ms: 5000 };
        assert_eq!(error.to_string(), "Operation timeout after 5000ms");

        let error =
            CircuitBreakerError::<String>::OperationFailed("service unavailable".to_string());
        assert_eq!(error.to_string(), "Operation failed: service unavailable");
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

mod integration_tests {
    use super::*;

    /// Verify all service configurations have sensible values.
    #[test]
    fn test_all_service_configurations_valid() {
        let configs = vec![
            service_bus_circuit_breaker_config(),
            blob_storage_circuit_breaker_config(),
            key_vault_circuit_breaker_config(),
        ];

        for config in configs {
            // All thresholds should be > 0
            assert!(config.failure_threshold > 0);
            assert!(config.success_threshold > 0);
            assert!(config.half_open_max_requests > 0);

            // All timeouts should be reasonable
            assert!(config.failure_window_seconds > 0);
            assert!(config.recovery_timeout_seconds > 0);
            assert!(config.operation_timeout_seconds > 0);

            // Service name should be set
            assert!(!config.service_name.is_empty());
            assert_ne!(config.service_name, "unknown");
        }
    }

    /// Verify REQ-009 compliance across all service configurations.
    ///
    /// REQ-009: Circuit breaker must trip after 5 consecutive failures
    /// and allow testing after 30-second cooldown.
    #[test]
    fn test_req_009_compliance() {
        // Service Bus and Blob Storage should follow REQ-009 exactly
        let service_bus = service_bus_circuit_breaker_config();
        assert_eq!(
            service_bus.failure_threshold, 5,
            "Service Bus should trip after 5 failures (REQ-009)"
        );
        assert_eq!(
            service_bus.recovery_timeout_seconds, 30,
            "Service Bus should have 30-second cooldown (REQ-009)"
        );

        let blob_storage = blob_storage_circuit_breaker_config();
        assert_eq!(
            blob_storage.failure_threshold, 5,
            "Blob Storage should trip after 5 failures (REQ-009)"
        );
        assert_eq!(
            blob_storage.recovery_timeout_seconds, 30,
            "Blob Storage should have 30-second cooldown (REQ-009)"
        );

        // Key Vault is more sensitive but still reasonable
        let key_vault = key_vault_circuit_breaker_config();
        assert!(
            key_vault.failure_threshold >= 3,
            "Key Vault should have reasonable failure threshold"
        );
        assert!(
            key_vault.recovery_timeout_seconds >= 30,
            "Key Vault should have adequate cooldown period"
        );
    }
}
