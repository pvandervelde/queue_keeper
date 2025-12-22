//! Circuit breaker resilience patterns for preventing cascading failures.
//!
//! This module implements the circuit breaker pattern to protect against
//! cascading failures in external service dependencies.
//!
//! # Circuit Breaker States
//!
//! - **Closed**: Normal operation, requests pass through
//! - **Open**: Service is failing, requests are rejected immediately
//! - **Half-Open**: Testing recovery, limited requests allowed
//!
//! # Example
//!
//! ```rust
//! use queue_keeper_core::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = CircuitBreakerConfig::default();
//! // Circuit breaker would wrap external service calls
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::future::Future;
use thiserror::Error;

use crate::Timestamp;

// Re-export implementation
mod breaker;
pub use breaker::{DefaultCircuitBreaker, DefaultCircuitBreakerFactory};

// ============================================================================
// Circuit Breaker Trait
// ============================================================================

/// Circuit breaker protection for external service operations.
///
/// Implements the circuit breaker pattern to protect against cascading
/// failures by failing fast when a service is experiencing issues.
///
/// # Type Parameters
///
/// - `T`: Success result type
/// - `E`: Operation error type
///
/// # States
///
/// - **Closed**: Normal operation, tracking failures
/// - **Open**: Fast-fail mode after consecutive failures
/// - **Half-Open**: Testing service recovery with limited requests
#[async_trait]
pub trait CircuitBreaker<T, E>: Send + Sync {
    /// Execute operation with circuit breaker protection.
    ///
    /// # Arguments
    ///
    /// - `operation`: Async closure that performs the protected operation
    ///
    /// # Returns
    ///
    /// - `Ok(T)`: Operation succeeded
    /// - `Err(CircuitBreakerError)`: Circuit protection or operation failure
    ///
    /// # Behavior
    ///
    /// - **Closed State**: Execute operation, track failures
    /// - **Open State**: Reject immediately with CircuitOpen error
    /// - **Half-Open State**: Allow limited concurrent requests
    async fn call<F, Fut>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, E>> + Send;

    /// Get current circuit breaker state.
    fn state(&self) -> CircuitState;

    /// Get circuit breaker metrics and statistics.
    fn metrics(&self) -> CircuitMetrics;

    /// Reset circuit breaker to closed state (admin operation).
    ///
    /// This is an administrative operation that forces the circuit
    /// back to closed state, clearing all failure counters.
    fn reset(&self);

    /// Check if circuit breaker is healthy (allowing requests).
    fn is_healthy(&self) -> bool {
        self.state().allows_requests()
    }
}

// ============================================================================
// Circuit Breaker Factory
// ============================================================================

/// Factory for creating circuit breakers with specific configurations.
pub trait CircuitBreakerFactory: Send + Sync {
    /// Create circuit breaker with configuration.
    ///
    /// # Arguments
    ///
    /// - `config`: Circuit breaker configuration
    ///
    /// # Returns
    ///
    /// Concrete circuit breaker instance
    fn create_circuit_breaker(
        &self,
        config: CircuitBreakerConfig,
    ) -> DefaultCircuitBreaker<Vec<u8>, Box<dyn std::error::Error + Send + Sync>>;

    /// Create typed circuit breaker.
    ///
    /// # Type Parameters
    ///
    /// - `T`: Success result type
    /// - `E`: Error type
    ///
    /// # Arguments
    ///
    /// - `config`: Circuit breaker configuration
    ///
    /// # Returns
    ///
    /// Concrete typed circuit breaker instance
    fn create_typed_circuit_breaker<T, E>(
        &self,
        config: CircuitBreakerConfig,
    ) -> DefaultCircuitBreaker<T, E>
    where
        T: Send + Sync + 'static,
        E: Send + Sync + 'static;
}

// ============================================================================
// Circuit State
// ============================================================================

/// Current state of the circuit breaker.
///
/// Circuit breakers transition between these states based on
/// success and failure patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed, allowing requests through.
    ///
    /// Normal operation mode, tracking failures to detect issues.
    Closed,

    /// Circuit is open, rejecting all requests.
    ///
    /// Fast-fail mode after consecutive failures exceeded threshold.
    Open,

    /// Circuit is half-open, allowing limited test requests.
    ///
    /// Testing recovery with limited concurrent requests.
    HalfOpen,
}

impl CircuitState {
    /// Check if requests are allowed in current state.
    ///
    /// # Returns
    ///
    /// - `true`: Closed or HalfOpen states allow requests
    /// - `false`: Open state rejects all requests
    pub fn allows_requests(&self) -> bool {
        matches!(self, Self::Closed | Self::HalfOpen)
    }

    /// Check if circuit is in failure state.
    ///
    /// # Returns
    ///
    /// - `true`: Open or HalfOpen states indicate issues
    /// - `false`: Closed state is healthy
    pub fn is_failure_state(&self) -> bool {
        matches!(self, Self::Open | Self::HalfOpen)
    }
}

// ============================================================================
// Circuit Breaker Configuration
// ============================================================================

/// Configuration for circuit breaker behavior.
///
/// Controls when the circuit trips, recovery timing, and request limits.
///
/// # Default Configuration
///
/// - Failure threshold: 5 consecutive failures
/// - Failure window: 60 seconds
/// - Recovery timeout: 30 seconds (REQ-009)
/// - Success threshold: 3 successes to close
/// - Operation timeout: 10 seconds
/// - Half-open max requests: 5 concurrent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Service name for identification.
    pub service_name: String,

    /// Number of consecutive failures to trip circuit.
    pub failure_threshold: u32,

    /// Time window for counting failures (seconds).
    pub failure_window_seconds: u64,

    /// Time circuit stays open before allowing test requests (seconds).
    ///
    /// REQ-009: Default 30 seconds for recovery timeout.
    pub recovery_timeout_seconds: u64,

    /// Number of successful requests needed to close circuit from half-open.
    pub success_threshold: u32,

    /// Timeout for individual operations (seconds).
    pub operation_timeout_seconds: u64,

    /// Maximum number of concurrent requests in half-open state.
    pub half_open_max_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            service_name: "unknown".to_string(),
            failure_threshold: 5,          // REQ-009: 5 consecutive failures
            failure_window_seconds: 60,    // 1 minute window
            recovery_timeout_seconds: 30,  // REQ-009: 30-second cooldown
            success_threshold: 3,          // 3 successes to close
            operation_timeout_seconds: 10, // 10 second operation timeout
            half_open_max_requests: 5,     // Limited testing
        }
    }
}

// ============================================================================
// Circuit Metrics
// ============================================================================

/// Metrics and statistics for circuit breaker.
///
/// Tracks operational statistics for monitoring and alerting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitMetrics {
    /// Current circuit state.
    pub state: CircuitState,

    /// Total number of requests processed.
    pub total_requests: u64,

    /// Number of successful requests.
    pub successful_requests: u64,

    /// Number of failed requests.
    pub failed_requests: u64,

    /// Number of requests rejected by open circuit.
    pub rejected_requests: u64,

    /// Consecutive failures in current window.
    pub consecutive_failures: u32,

    /// Time when circuit last changed state.
    pub last_state_change: Timestamp,

    /// Time when circuit will next attempt recovery (if open).
    pub next_recovery_attempt: Option<Timestamp>,

    /// Current failure rate (0.0 to 1.0).
    pub failure_rate: f64,

    /// Average response time in milliseconds.
    pub avg_response_time_ms: f64,
}

impl CircuitMetrics {
    /// Calculate success rate.
    ///
    /// # Returns
    ///
    /// Success rate from 0.0 to 1.0, or 1.0 if no requests processed.
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }

    /// Check if circuit should trip based on failure rate.
    ///
    /// # Arguments
    ///
    /// - `threshold`: Number of consecutive failures required
    ///
    /// # Returns
    ///
    /// `true` if consecutive failures meet or exceed threshold
    pub fn should_trip(&self, threshold: u32) -> bool {
        self.consecutive_failures >= threshold
    }
}

// ============================================================================
// Circuit Breaker Error
// ============================================================================

/// Errors that can occur with circuit breaker operations.
///
/// Wraps operation errors and adds circuit breaker-specific failures.
#[derive(Debug, Error)]
pub enum CircuitBreakerError<E> {
    /// Circuit breaker is open - requests rejected.
    ///
    /// Fast-fail mode, service is experiencing issues.
    #[error("Circuit breaker is open - requests rejected")]
    CircuitOpen,

    /// Operation timeout after specified duration.
    #[error("Operation timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Operation failed with error.
    #[error("Operation failed: {0}")]
    OperationFailed(E),

    /// Circuit breaker internal error.
    #[error("Circuit breaker internal error: {message}")]
    InternalError { message: String },

    /// Too many concurrent requests in half-open state.
    #[error("Too many concurrent requests in half-open state")]
    TooManyConcurrentRequests,
}

impl<E> CircuitBreakerError<E> {
    /// Check if error should count as failure for circuit breaker.
    ///
    /// # Returns
    ///
    /// `true` for errors that indicate service issues
    pub fn counts_as_failure(&self) -> bool {
        matches!(
            self,
            Self::OperationFailed(_) | Self::Timeout { .. } | Self::InternalError { .. }
        )
    }

    /// Check if error is due to circuit breaker protection.
    ///
    /// # Returns
    ///
    /// `true` for circuit breaker protection errors (not operation errors)
    pub fn is_circuit_protection(&self) -> bool {
        matches!(self, Self::CircuitOpen | Self::TooManyConcurrentRequests)
    }
}

// ============================================================================
// Service-Specific Configurations
// ============================================================================

/// Circuit breaker configuration for Azure Service Bus.
///
/// Tuned for queue operation patterns:
/// - 5 consecutive failures to trip
/// - 30 second recovery timeout
/// - 3 successes to close
/// - 5 second operation timeout
pub fn service_bus_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        service_name: "azure-service-bus".to_string(),
        failure_threshold: 5,         // REQ-009: 5 consecutive failures
        failure_window_seconds: 60,   // 1 minute window
        recovery_timeout_seconds: 30, // REQ-009: 30 second cooldown
        success_threshold: 3,         // 3 successes to close
        operation_timeout_seconds: 5, // 5 second timeout for queue operations
        half_open_max_requests: 3,    // Conservative testing
    }
}

/// Circuit breaker configuration for Azure Blob Storage.
///
/// Tuned for blob storage operation patterns:
/// - 5 consecutive failures to trip
/// - 30 second recovery timeout
/// - 2 successes to close
/// - 10 second operation timeout
pub fn blob_storage_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        service_name: "azure-blob-storage".to_string(),
        failure_threshold: 5,          // REQ-009: 5 consecutive failures
        failure_window_seconds: 60,    // 1 minute window
        recovery_timeout_seconds: 30,  // REQ-009: 30 second cooldown
        success_threshold: 2,          // 2 successes to close
        operation_timeout_seconds: 10, // 10 second timeout for blob operations
        half_open_max_requests: 5,     // Allow more testing for storage
    }
}

/// Circuit breaker configuration for Azure Key Vault.
///
/// More sensitive due to security impact:
/// - 3 consecutive failures to trip (more sensitive)
/// - 60 second recovery timeout (longer for security)
/// - 2 successes to close
/// - 5 second operation timeout
pub fn key_vault_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        service_name: "azure-key-vault".to_string(),
        failure_threshold: 3,         // More sensitive due to security impact
        failure_window_seconds: 60,   // 1 minute window
        recovery_timeout_seconds: 60, // Longer cooldown for security
        success_threshold: 2,         // 2 successes to close
        operation_timeout_seconds: 5, // 5 second timeout for secret operations
        half_open_max_requests: 2,    // Very conservative testing
    }
}

/// Circuit breaker configuration for GitHub API.
///
/// Tuned for GitHub API operation patterns:
/// - 5 consecutive failures to trip
/// - 60 second recovery timeout (respect GitHub rate limits)
/// - 3 successes to close
/// - 10 second operation timeout (network + processing)
pub fn github_api_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        service_name: "github-api".to_string(),
        failure_threshold: 5,          // REQ-009: 5 consecutive failures
        failure_window_seconds: 120,   // 2 minute window (GitHub rate limits)
        recovery_timeout_seconds: 60,  // 60 second cooldown to respect rate limits
        success_threshold: 3,          // 3 successes to close
        operation_timeout_seconds: 10, // 10 second timeout for API operations
        half_open_max_requests: 3,     // Conservative testing
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
