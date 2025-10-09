# Implementation Constraints

This document defines the hard rules and architectural boundaries that must be enforced during Queue-Keeper implementation.

## Type System Constraints

### Domain Types

- All domain identifiers MUST use branded types (EventId, SessionId, RepositoryId)
- All public APIs MUST return `Result<T, E>` or `Promise<Result<T, E>>` for fallible operations
- NEVER use `any` equivalent types in domain code (Rust: avoid `dyn Any`)
- All error types MUST be discriminated unions with specific error variants
- All datetime values MUST use UTC timezone with explicit `DateTime<Utc>` types

### Event Processing Types

- EventEnvelope MUST be immutable after creation
- Session IDs MUST be limited to 128 characters maximum
- Event IDs MUST be globally unique and sortable (ULID recommended)
- Repository names MUST follow GitHub format validation (`owner/name`)
- Entity IDs MUST be strings, even for numeric GitHub IDs

### Queue Message Types

- All queue messages MUST implement `Clone + Send + Sync` traits
- Message receipts MUST be opaque types that prevent direct construction
- Queue operations MUST be generic over message and receipt types
- TTL values MUST be `Duration` types, never raw seconds

## Module Boundary Constraints

### Application Structure

```
src/
├── webhook/         # Webhook processing logic
│   ├── mod.rs       # Webhook types and main handler
│   ├── validation.rs # Signature validation
│   └── normalization.rs # Event normalization
├── routing/         # Event routing and distribution
│   ├── mod.rs       # Routing logic and configuration
│   ├── rules.rs     # Routing rule engine
│   └── session.rs   # Session management for ordering
├── storage/         # Blob storage operations
│   ├── mod.rs       # Storage interface and types
│   └── azure_blob.rs # Azure Blob Storage implementation
├── queues/          # Queue operations
│   ├── mod.rs       # Queue interface and types
│   └── service_bus.rs # Azure Service Bus implementation
├── config.rs        # Configuration loading and validation
├── error.rs         # Error types and handling
├── observability.rs # Tracing and metrics
└── app.rs           # Application setup and coordination
```

### Dependency Direction Rules

```
Webhook → Routing → Queues
   ↓         ↓        ↓
Storage ← Config → Observability
```

### Cross-Module Communication

- Components communicate through explicit interfaces, never direct struct access
- No circular dependencies between modules
- All cross-module calls MUST be async-compatible
- Shared types defined in common module, not duplicated

## Error Handling Constraints

### Error Classification Rules

- Expected errors are values (Result type), not exceptions/panics
- Exceptions/panics only for unrecoverable failures (out of memory, hardware failure)
- All error types MUST include sufficient context for debugging
- Error messages MUST NOT contain sensitive information (secrets, tokens)

### Error Type Requirements

```rust
// REQUIRED: All domain errors must implement this pattern
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Invalid webhook signature")]
    InvalidSignature,

    #[error("Event not found: {event_id}")]
    EventNotFound { event_id: EventId },

    #[error("Storage operation failed: {source}")]
    StorageFailure {
        #[from]
        source: StorageError
    },
}
```

### Retry Constraint Rules

- Retry logic MUST distinguish transient from permanent errors
- Maximum retry attempts: 5 for any operation
- Exponential backoff MUST include jitter (±25%)
- Circuit breakers MUST open after 5 consecutive failures
- No retries for authentication failures or malformed data

## Security Constraints

### Secret Management Rules

- Secrets MUST NEVER appear in logs, error messages, or debug output
- All secret types MUST implement secure string patterns with redacted Debug
- Secret caching TTL MUST NOT exceed 5 minutes
- Secrets MUST be retrieved using authenticated service identities only

### Cryptographic Requirements

- Webhook signature validation MUST use constant-time comparison
- HMAC-SHA256 ONLY for webhook signatures (no SHA1, MD5, etc.)
- Random values MUST use cryptographically secure generators
- All TLS connections MUST validate certificates

### Input Validation Constraints

- ALL external input MUST be validated before processing
- Webhook payloads limited to 1MB maximum
- String inputs MUST be length-limited to prevent DoS
- JSON parsing MUST have depth limits to prevent stack overflow

## Performance Constraints

### Response Time Requirements

- Webhook processing MUST complete in <1 second (95th percentile)
- Individual component operations MUST complete in <100ms
- Database/storage operations MUST have 5-second timeout maximum
- Network operations MUST have configurable timeout with 30-second default

### Resource Usage Limits

- Memory usage MUST NOT exceed 512MB per instance under normal load
- CPU usage MUST remain below 80% under sustained load
- File descriptors MUST be properly closed to prevent leaks
- Connection pools MUST have maximum connection limits

### Concurrency Constraints

- All public APIs MUST be thread-safe
- No shared mutable state without explicit synchronization
- Async operations MUST be cancellation-safe
- No blocking operations in async contexts

## Testing Constraints

### Coverage Requirements

- Core domain MUST have 100% unit test coverage
- Port interfaces MUST have contract tests
- Adapters tested via integration tests
- All error paths MUST be tested

### Test Organization Rules

- Unit tests in same module as implementation
- Integration tests in separate `tests/` directory
- Mock implementations for all external dependencies
- No network calls in unit tests

### Test Data Constraints

- Test data MUST NOT contain real secrets or tokens
- Deterministic test data for reproducible results
- Test isolation - no shared state between tests
- Property-based testing for input validation

## Deployment Constraints

### Configuration Management

- Configuration MUST be immutable after application startup
- Environment-specific values MUST come from environment variables
- No hardcoded secrets, URLs, or environment-specific values in code
- Configuration validation MUST occur at startup

### Logging and Observability

- All log messages MUST be structured (JSON format)
- Correlation IDs MUST be propagated through entire request chain
- No PII (personally identifiable information) in logs
- Log levels: ERROR (failures), WARN (retries), INFO (success), DEBUG (details)

### Health Checks and Monitoring

- Health check endpoint MUST respond in <1 second
- Health checks MUST verify all critical dependencies
- Metrics MUST be exposed in standard format (Prometheus-compatible)
- Distributed tracing MUST follow W3C Trace Context standard

## API Contract Constraints

### HTTP Interface Rules

- All endpoints MUST return standard HTTP status codes
- Response bodies MUST be valid JSON with proper content-type headers
- Error responses MUST include correlation IDs for debugging
- Request timeouts MUST be handled gracefully with appropriate responses

### Queue Interface Rules

- Message processing MUST be idempotent
- Message acknowledgment MUST be explicit, not automatic
- Session-based ordering MUST be respected for related messages
- Dead letter queue handling MUST preserve original message context

### External Service Integration

- All external service calls MUST have timeout and retry configuration
- Circuit breakers MUST protect against cascading failures
- Service discovery MUST support multiple endpoints for failover
- Authentication tokens MUST be refreshed before expiration

## Data Integrity Constraints

### Storage Requirements

- All stored webhook payloads MUST be immutable
- Storage operations MUST be atomic or properly transactional
- Data corruption MUST be detectable through checksums or signatures
- Backup and recovery procedures MUST be tested regularly

### Queue Message Integrity

- Message deduplication MUST prevent processing duplicates
- Message TTL MUST prevent indefinite queue growth
- Session ordering MUST guarantee FIFO processing within session
- Failed messages MUST preserve enough context for debugging

## Compliance and Audit Constraints

### Audit Trail Requirements

- ALL webhook processing activities MUST be auditable
- Audit logs MUST include correlation IDs for end-to-end tracing
- Log retention MUST meet regulatory requirements (minimum 90 days)
- Audit data MUST be tamper-evident and immutable

### Data Privacy Rules

- No user PII stored beyond what's necessary for functionality
- All data storage MUST comply with applicable privacy regulations
- Data deletion requests MUST be honored within defined timeframes
- Cross-border data transfer MUST comply with regional requirements

## Violation Detection

### Static Analysis Requirements

- Linting rules MUST enforce coding standards and best practices
- Dependency analysis MUST detect circular imports and violations
- Security scanning MUST identify known vulnerabilities
- License compliance MUST be verified for all dependencies

### Runtime Monitoring

- Performance metrics MUST detect constraint violations
- Error rates MUST trigger alerts when thresholds exceeded
- Resource usage MUST be monitored and capped
- Timeout violations MUST be logged and alerted

These constraints define the non-negotiable boundaries for Queue-Keeper implementation, ensuring system reliability, security, and maintainability.
