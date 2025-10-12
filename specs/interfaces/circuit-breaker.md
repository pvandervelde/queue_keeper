# Circuit Breaker Interface

**Architectural Layer**: Infrastructure Interface
**Module Path**: `src/circuit_breaker.rs`
**Responsibilities** (from RDD):

- Knows: Failure thresholds, circuit state, recovery timing
- Does: Protects against cascading failures, manages circuit state transitions, provides fail-fast behavior

## Dependencies

- Types: None (infrastructure pattern)
- Interfaces: None (self-contained)
- Shared: `Result<T, E>` (shared-types.md)

## Primary Traits

### CircuitBreaker

#### Purpose

Implements the circuit breaker pattern to protect against cascading failures in external service dependencies.

#### Interface Definition

```rust
#[async_trait]
pub trait CircuitBreaker<T, E>: Send + Sync {
    /// Execute operation with circuit breaker protection
    async fn call<F, Fut>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, E>> + Send;

    /// Get current circuit breaker state
    fn state(&self) -> CircuitState;

    /// Get circuit breaker metrics
    fn metrics(&self) -> CircuitMetrics;

    /// Reset circuit breaker to closed state (admin operation)
    fn reset(&self);

    /// Check if circuit breaker is healthy
    fn is_healthy(&self) -> bool;
}
```

### CircuitBreakerFactory

#### Purpose

Creates and configures circuit breakers for different services.

#### Interface Definition

```rust
pub trait CircuitBreakerFactory: Send + Sync {
    /// Create circuit breaker for service
    fn create_circuit_breaker(
        &self,
        config: CircuitBreakerConfig,
    ) -> Box<dyn CircuitBreaker<Vec<u8>, ServiceError>>;

    /// Create typed circuit breaker
    fn create_typed_circuit_breaker<T, E>(
        &self,
        config: CircuitBreakerConfig,
    ) -> Box<dyn CircuitBreaker<T, E>>
    where
        T: Send + 'static,
        E: Send + 'static;
}
```

## Supporting Types

### CircuitState

```rust
/// Current state of the circuit breaker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed, allowing requests through
    Closed,

    /// Circuit is open, rejecting all requests
    Open,

    /// Circuit is half-open, allowing limited test requests
    HalfOpen,
}

impl CircuitState {
    /// Check if requests are allowed in current state
    pub fn allows_requests(&self) -> bool {
        matches!(self, Self::Closed | Self::HalfOpen)
    }

    /// Check if circuit is in failure state
    pub fn is_failure_state(&self) -> bool {
        matches!(self, Self::Open | Self::HalfOpen)
    }
}
```

### CircuitBreakerConfig

```rust
/// Configuration for circuit breaker behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Service name for identification
    pub service_name: String,

    /// Number of consecutive failures to trip circuit
    pub failure_threshold: u32,

    /// Time window for counting failures (seconds)
    pub failure_window_seconds: u64,

    /// Time circuit stays open before allowing test requests (seconds)
    pub recovery_timeout_seconds: u64,

    /// Number of successful requests needed to close circuit from half-open
    pub success_threshold: u32,

    /// Timeout for individual operations (seconds)
    pub operation_timeout_seconds: u64,

    /// Maximum number of concurrent requests in half-open state
    pub half_open_max_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            service_name: "unknown".to_string(),
            failure_threshold: 5,           // REQ-009: 5 consecutive failures
            failure_window_seconds: 60,     // 1 minute window
            recovery_timeout_seconds: 30,   // REQ-009: 30-second cooldown
            success_threshold: 3,           // 3 successes to close
            operation_timeout_seconds: 10,  // 10 second operation timeout
            half_open_max_requests: 5,      // Limited testing
        }
    }
}
```

### CircuitMetrics

```rust
/// Metrics and statistics for circuit breaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitMetrics {
    /// Current circuit state
    pub state: CircuitState,

    /// Total number of requests processed
    pub total_requests: u64,

    /// Number of successful requests
    pub successful_requests: u64,

    /// Number of failed requests
    pub failed_requests: u64,

    /// Number of requests rejected by open circuit
    pub rejected_requests: u64,

    /// Consecutive failures in current window
    pub consecutive_failures: u32,

    /// Time when circuit last changed state
    pub last_state_change: Timestamp,

    /// Time when circuit will next attempt recovery (if open)
    pub next_recovery_attempt: Option<Timestamp>,

    /// Current failure rate (0.0 to 1.0)
    pub failure_rate: f64,

    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
}

impl CircuitMetrics {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }

    /// Check if circuit should trip based on failure rate
    pub fn should_trip(&self, threshold: u32) -> bool {
        self.consecutive_failures >= threshold
    }
}
```

### CircuitBreakerError

```rust
/// Errors that can occur with circuit breaker
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open - requests rejected")]
    CircuitOpen,

    #[error("Operation timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Operation failed: {0}")]
    OperationFailed(E),

    #[error("Circuit breaker internal error: {message}")]
    InternalError { message: String },

    #[error("Too many concurrent requests in half-open state")]
    TooManyConcurrentRequests,
}

impl<E> CircuitBreakerError<E> {
    /// Check if error should count as failure for circuit breaker
    pub fn counts_as_failure(&self) -> bool {
        matches!(
            self,
            Self::OperationFailed(_) | Self::Timeout { .. } | Self::InternalError { .. }
        )
    }

    /// Check if error is due to circuit breaker protection
    pub fn is_circuit_protection(&self) -> bool {
        matches!(self, Self::CircuitOpen | Self::TooManyConcurrentRequests)
    }
}
```

## Service-Specific Circuit Breakers

### ServiceBusCircuitBreaker

```rust
/// Circuit breaker configuration for Azure Service Bus
pub fn service_bus_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        service_name: "azure-service-bus".to_string(),
        failure_threshold: 5,           // 5 consecutive failures
        failure_window_seconds: 60,     // 1 minute window
        recovery_timeout_seconds: 30,   // 30 second cooldown
        success_threshold: 3,           // 3 successes to close
        operation_timeout_seconds: 5,   // 5 second timeout for queue operations
        half_open_max_requests: 3,      // Conservative testing
    }
}
```

### BlobStorageCircuitBreaker

```rust
/// Circuit breaker configuration for Azure Blob Storage
pub fn blob_storage_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        service_name: "azure-blob-storage".to_string(),
        failure_threshold: 5,           // 5 consecutive failures
        failure_window_seconds: 60,     // 1 minute window
        recovery_timeout_seconds: 30,   // 30 second cooldown
        success_threshold: 2,           // 2 successes to close
        operation_timeout_seconds: 10,  // 10 second timeout for blob operations
        half_open_max_requests: 5,      // Allow more testing for storage
    }
}
```

### KeyVaultCircuitBreaker

```rust
/// Circuit breaker configuration for Azure Key Vault
pub fn key_vault_circuit_breaker_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig {
        service_name: "azure-key-vault".to_string(),
        failure_threshold: 3,           // More sensitive due to security impact
        failure_window_seconds: 60,     // 1 minute window
        recovery_timeout_seconds: 60,   // Longer cooldown for security
        success_threshold: 2,           // 2 successes to close
        operation_timeout_seconds: 5,   // 5 second timeout for secret operations
        half_open_max_requests: 2,      // Very conservative testing
    }
}
```

## Usage Examples

### Basic Circuit Breaker Usage

```rust
use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, service_bus_circuit_breaker_config};

let config = service_bus_circuit_breaker_config();
let circuit_breaker = DefaultCircuitBreaker::new(config);

// Protected operation
let result = circuit_breaker.call(|| async {
    service_bus_client.send_message(message).await
}).await;

match result {
    Ok(response) => {
        println!("Message sent successfully");
    }
    Err(CircuitBreakerError::CircuitOpen) => {
        println!("Service Bus circuit is open - using fallback");
        // Handle graceful degradation
    }
    Err(CircuitBreakerError::OperationFailed(e)) => {
        println!("Operation failed: {}", e);
    }
    Err(e) => {
        println!("Circuit breaker error: {}", e);
    }
}
```

### Integration with Health Checks

```rust
pub struct ServiceHealthChecker {
    service_bus_circuit: Arc<dyn CircuitBreaker<(), ServiceBusError>>,
    blob_storage_circuit: Arc<dyn CircuitBreaker<(), BlobStorageError>>,
    key_vault_circuit: Arc<dyn CircuitBreaker<(), KeyVaultError>>,
}

impl HealthChecker for ServiceHealthChecker {
    async fn check_deep_health(&self) -> HealthStatus {
        let mut checks = HashMap::new();

        // Check circuit breaker states
        checks.insert("service_bus_circuit".to_string(), HealthCheckResult {
            healthy: self.service_bus_circuit.is_healthy(),
            message: format!("State: {:?}", self.service_bus_circuit.state()),
            duration_ms: 0,
        });

        checks.insert("blob_storage_circuit".to_string(), HealthCheckResult {
            healthy: self.blob_storage_circuit.is_healthy(),
            message: format!("State: {:?}", self.blob_storage_circuit.state()),
            duration_ms: 0,
        });

        checks.insert("key_vault_circuit".to_string(), HealthCheckResult {
            healthy: self.key_vault_circuit.is_healthy(),
            message: format!("State: {:?}", self.key_vault_circuit.state()),
            duration_ms: 0,
        });

        let is_healthy = checks.values().all(|check| check.healthy);

        HealthStatus {
            is_healthy,
            checks,
        }
    }
}
```

### Graceful Degradation Example

```rust
pub struct WebhookProcessor {
    blob_storage: Arc<dyn BlobStorage>,
    blob_circuit: Arc<dyn CircuitBreaker<BlobMetadata, BlobStorageError>>,
    queue_client: Arc<dyn QueueClient>,
    queue_circuit: Arc<dyn CircuitBreaker<(), QueueError>>,
}

impl WebhookProcessor {
    pub async fn process_webhook(&self, request: WebhookRequest) -> Result<EventEnvelope, WebhookError> {
        let envelope = self.normalize_event(&request)?;

        // Try to store payload with circuit breaker protection
        let _storage_result = self.blob_circuit.call(|| {
            let storage = self.blob_storage.clone();
            let envelope = envelope.clone();
            async move {
                storage.store_payload(&envelope.event_id, &envelope.into()).await
            }
        }).await;

        // Continue processing even if storage fails (graceful degradation)
        if let Err(CircuitBreakerError::CircuitOpen) = _storage_result {
            warn!("Blob storage circuit open - continuing without persistence");
        }

        // Queue routing with circuit breaker protection
        self.queue_circuit.call(|| {
            let client = self.queue_client.clone();
            let envelope = envelope.clone();
            async move {
                client.route_to_bots(&envelope).await
            }
        }).await.map_err(|e| match e {
            CircuitBreakerError::CircuitOpen => {
                WebhookError::ServiceUnavailable {
                    service: "queue-routing".to_string(),
                    message: "Queue routing circuit is open".to_string(),
                }
            }
            CircuitBreakerError::OperationFailed(queue_err) => {
                WebhookError::QueueingFailed(queue_err)
            }
            _ => WebhookError::InternalError {
                message: format!("Circuit breaker error: {}", e),
            }
        })?;

        Ok(envelope)
    }
}
```

## Implementation Notes

### REQ-009 Compliance

- Circuit breaker MUST trip after 5 consecutive failures to any downstream service
- Half-open state MUST allow limited testing after 30-second cooldown period
- Circuit breaker status MUST be exposed via health check endpoints
- Graceful degradation patterns for each protected service

### State Management

- Circuit state changes must be thread-safe
- Metrics must be updated atomically
- State transitions must be logged for debugging
- Recovery attempts must be rate-limited

### Performance Considerations

- Circuit breaker overhead must be minimal (<1ms per call)
- State checks must be non-blocking
- Metrics collection must not impact operation performance
- Memory usage must be bounded and predictable

### Monitoring Integration

- Circuit breaker state changes must generate events
- Metrics must be exposed via Prometheus endpoint
- Health checks must report circuit breaker status
- Alerting rules must monitor circuit breaker trips
