//! Default circuit breaker implementation.
//!
//! Provides a thread-safe, production-ready circuit breaker implementation
//! using Arc<RwLock<>> for state management.

use async_trait::async_trait;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::timeout;

use super::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitBreakerFactory,
    CircuitMetrics, CircuitState,
};
use crate::Timestamp;

// ============================================================================
// Internal State
// ============================================================================

/// Internal state for circuit breaker.
///
/// Protected by RwLock for thread-safe access.
#[derive(Debug)]
struct InternalState {
    /// Current circuit state
    current_state: CircuitState,

    /// Consecutive failures in current window
    consecutive_failures: u32,

    /// Consecutive successes in half-open state
    consecutive_successes: u32,

    /// Current concurrent requests in half-open state
    half_open_concurrent: u32,

    /// Timestamp of last state change
    last_state_change: Timestamp,

    /// Timestamp when circuit can transition from open to half-open
    next_recovery_attempt: Option<Timestamp>,

    /// Total requests processed
    total_requests: u64,

    /// Successful requests
    successful_requests: u64,

    /// Failed requests
    failed_requests: u64,

    /// Rejected requests (circuit open)
    rejected_requests: u64,

    /// Total response time for average calculation
    total_response_time_ms: f64,
}

impl InternalState {
    fn new() -> Self {
        Self {
            current_state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            half_open_concurrent: 0,
            last_state_change: Timestamp::now(),
            next_recovery_attempt: None,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            rejected_requests: 0,
            total_response_time_ms: 0.0,
        }
    }

    /// Calculate current failure rate
    fn failure_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.failed_requests as f64 / self.total_requests as f64
        }
    }

    /// Calculate average response time
    fn avg_response_time_ms(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_response_time_ms / self.total_requests as f64
        }
    }
}

// ============================================================================
// Default Circuit Breaker
// ============================================================================

/// Default circuit breaker implementation.
///
/// Thread-safe implementation using Arc<RwLock<>> for state management.
/// Implements the circuit breaker pattern with configurable thresholds.
pub struct DefaultCircuitBreaker<T, E> {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<InternalState>>,
    _phantom: std::marker::PhantomData<(T, E)>,
}

impl<T, E> DefaultCircuitBreaker<T, E> {
    /// Create new circuit breaker with configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(InternalState::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Check if circuit should transition from open to half-open.
    fn should_attempt_recovery(&self, state: &InternalState) -> bool {
        match state.current_state {
            CircuitState::Open => {
                if let Some(recovery_time) = state.next_recovery_attempt {
                    Timestamp::now() >= recovery_time
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Transition circuit to open state.
    fn trip_circuit(&self, state: &mut InternalState) {
        state.current_state = CircuitState::Open;
        state.last_state_change = Timestamp::now();
        state.next_recovery_attempt =
            Some(Timestamp::now().add_seconds(self.config.recovery_timeout_seconds));
        state.consecutive_successes = 0;
    }

    /// Transition circuit to half-open state.
    fn transition_to_half_open(&self, state: &mut InternalState) {
        state.current_state = CircuitState::HalfOpen;
        state.last_state_change = Timestamp::now();
        state.next_recovery_attempt = None;
        state.consecutive_failures = 0;
        state.consecutive_successes = 0;
        state.half_open_concurrent = 0;
    }

    /// Transition circuit to closed state.
    fn close_circuit(&self, state: &mut InternalState) {
        state.current_state = CircuitState::Closed;
        state.last_state_change = Timestamp::now();
        state.next_recovery_attempt = None;
        state.consecutive_failures = 0;
        state.consecutive_successes = 0;
        state.half_open_concurrent = 0;
    }

    /// Record successful request.
    fn record_success(&self, state: &mut InternalState, response_time_ms: f64) {
        state.successful_requests += 1;
        state.total_requests += 1;
        state.total_response_time_ms += response_time_ms;
        state.consecutive_failures = 0;

        match state.current_state {
            CircuitState::Closed => {
                // Normal operation, no state change
            }
            CircuitState::HalfOpen => {
                state.consecutive_successes += 1;
                state.half_open_concurrent = state.half_open_concurrent.saturating_sub(1);

                // Check if we should close the circuit
                if state.consecutive_successes >= self.config.success_threshold {
                    self.close_circuit(state);
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but handle gracefully
                state.half_open_concurrent = state.half_open_concurrent.saturating_sub(1);
            }
        }
    }

    /// Record failed request.
    fn record_failure(&self, state: &mut InternalState, response_time_ms: f64) {
        state.failed_requests += 1;
        state.total_requests += 1;
        state.total_response_time_ms += response_time_ms;
        state.consecutive_failures += 1;
        state.consecutive_successes = 0;

        match state.current_state {
            CircuitState::Closed => {
                // Check if we should trip the circuit
                if state.consecutive_failures >= self.config.failure_threshold {
                    self.trip_circuit(state);
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state trips the circuit
                state.half_open_concurrent = state.half_open_concurrent.saturating_sub(1);
                self.trip_circuit(state);
            }
            CircuitState::Open => {
                // Already open, just decrement concurrent counter if needed
                state.half_open_concurrent = state.half_open_concurrent.saturating_sub(1);
            }
        }
    }

    /// Record rejected request.
    fn record_rejection(&self, state: &mut InternalState) {
        state.rejected_requests += 1;
    }
}

#[async_trait]
impl<T, E> CircuitBreaker<T, E> for DefaultCircuitBreaker<T, E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    async fn call<F, Fut>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T, E>> + Send,
    {
        let start_time = std::time::Instant::now();

        // Check circuit state and handle transitions
        let should_execute = {
            let mut state = self
                .state
                .write()
                .map_err(|e| CircuitBreakerError::InternalError {
                    message: format!("Failed to acquire write lock: {}", e),
                })?;

            match state.current_state {
                CircuitState::Closed => true,
                CircuitState::Open => {
                    if self.should_attempt_recovery(&state) {
                        self.transition_to_half_open(&mut state);
                        true
                    } else {
                        self.record_rejection(&mut state);
                        false
                    }
                }
                CircuitState::HalfOpen => {
                    if state.half_open_concurrent >= self.config.half_open_max_requests {
                        self.record_rejection(&mut state);
                        return Err(CircuitBreakerError::TooManyConcurrentRequests);
                    }
                    state.half_open_concurrent += 1;
                    true
                }
            }
        };

        if !should_execute {
            return Err(CircuitBreakerError::CircuitOpen);
        }

        // Execute operation with timeout
        let operation_timeout = Duration::from_secs(self.config.operation_timeout_seconds);
        let result = timeout(operation_timeout, operation()).await;

        let elapsed = start_time.elapsed().as_millis() as f64;

        // Process result and update state
        let mut state = self
            .state
            .write()
            .map_err(|e| CircuitBreakerError::InternalError {
                message: format!("Failed to acquire write lock: {}", e),
            })?;

        match result {
            Ok(Ok(value)) => {
                self.record_success(&mut state, elapsed);
                Ok(value)
            }
            Ok(Err(e)) => {
                self.record_failure(&mut state, elapsed);
                Err(CircuitBreakerError::OperationFailed(e))
            }
            Err(_) => {
                self.record_failure(&mut state, elapsed);
                Err(CircuitBreakerError::Timeout {
                    timeout_ms: operation_timeout.as_millis() as u64,
                })
            }
        }
    }

    fn state(&self) -> CircuitState {
        self.state
            .read()
            .map(|state| state.current_state)
            .unwrap_or(CircuitState::Open) // Fail-safe: treat lock poisoning as open
    }

    fn metrics(&self) -> CircuitMetrics {
        let state = self.state.read().unwrap();

        CircuitMetrics {
            state: state.current_state,
            total_requests: state.total_requests,
            successful_requests: state.successful_requests,
            failed_requests: state.failed_requests,
            rejected_requests: state.rejected_requests,
            consecutive_failures: state.consecutive_failures,
            last_state_change: state.last_state_change,
            next_recovery_attempt: state.next_recovery_attempt,
            failure_rate: state.failure_rate(),
            avg_response_time_ms: state.avg_response_time_ms(),
        }
    }

    fn reset(&self) {
        let mut state = self.state.write().unwrap();
        self.close_circuit(&mut state);
        state.total_requests = 0;
        state.successful_requests = 0;
        state.failed_requests = 0;
        state.rejected_requests = 0;
        state.total_response_time_ms = 0.0;
    }
}

// ============================================================================
// Default Circuit Breaker Factory
// ============================================================================

/// Default factory for creating circuit breakers.
pub struct DefaultCircuitBreakerFactory;

impl DefaultCircuitBreakerFactory {
    /// Create new factory instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultCircuitBreakerFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreakerFactory for DefaultCircuitBreakerFactory {
    fn create_circuit_breaker(
        &self,
        config: CircuitBreakerConfig,
    ) -> DefaultCircuitBreaker<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        DefaultCircuitBreaker::new(config)
    }

    fn create_typed_circuit_breaker<T, E>(
        &self,
        config: CircuitBreakerConfig,
    ) -> DefaultCircuitBreaker<T, E>
    where
        T: Send + Sync + 'static,
        E: Send + Sync + 'static,
    {
        DefaultCircuitBreaker::new(config)
    }
}

#[cfg(test)]
#[path = "breaker_tests.rs"]
mod tests;
