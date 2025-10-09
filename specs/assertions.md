# Behavioral Assertions

This document defines testable behaviors and constraints that must be validated during implementation and testing of Queue-Keeper.

## Webhook Processing Assertions

### 1. Signature Validation

All incoming webhooks MUST be validated using HMAC-SHA256 with the GitHub webhook secret before any processing occurs.

**Test Scenarios:**

- Valid signature with correct secret → Processing continues
- Invalid signature with wrong secret → HTTP 401, no processing
- Missing signature header → HTTP 401, no processing
- Malformed signature format → HTTP 401, no processing

### 2. Response Time SLA

Queue-Keeper MUST respond to GitHub within 1 second for 95% of requests under normal load conditions.

**Test Scenarios:**

- Measure response times under 100 concurrent requests
- Verify P95 latency stays below 1000ms
- Confirm no requests exceed GitHub's 10-second timeout

### 3. Payload Persistence

Every valid webhook payload MUST be persisted to blob storage before normalization, ensuring no data loss even if downstream processing fails.

**Test Scenarios:**

- Successful webhook → Payload stored in blob storage
- Downstream failure after storage → Payload remains in storage
- Storage failure → Processing continues with warning log
- Replay scenario → Original payload retrievable from storage

### 4. Event ID Uniqueness

Generated event IDs MUST be globally unique and sortable, preventing duplicate processing across system restarts.

**Test Scenarios:**

- Multiple instances generating event IDs → No collisions
- System restart → New event IDs don't conflict with previous
- Event ID format → Lexicographically sortable by creation time

### 5. Session ID Consistency

Events for the same GitHub entity (PR/issue) MUST generate identical session IDs to ensure ordered processing.

**Test Scenarios:**

- PR opened → Session ID generated
- Same PR updated → Identical session ID generated
- Different PR → Different session ID generated
- System restart → Same entities produce same session IDs

## Queue Routing Assertions

### 6. One-to-Many Routing

A single webhook event MUST be successfully delivered to all configured bot queues based on static subscription configuration.

**Test Scenarios:**

- Event matches multiple bot subscriptions → Delivered to all matching queues
- Event matches single bot subscription → Delivered to one queue only
- Event matches no subscriptions → No queue deliveries
- Partial delivery failure → All deliveries rolled back

### 7. Ordering Guarantee

Events with identical session IDs MUST be processed sequentially by the same bot instance, while events with different session IDs MAY be processed in parallel.

**Test Scenarios:**

- Same session ID events → Processed in chronological order
- Different session ID events → May process concurrently
- Session timeout → Next consumer can claim session
- Bot failure during session → Session eventually released

### 8. Routing Atomicity

Either all configured bot queues receive the event, or the entire operation fails and gets retried.

**Test Scenarios:**

- All queue sends succeed → Event processing completes
- One queue send fails → All deliveries rolled back
- Network partition during routing → Either all succeed or all fail
- Retry after failure → Eventual delivery to all queues

### 9. Dead Letter Handling

Events that fail delivery after maximum retry attempts MUST be routed to the dead letter queue with failure metadata.

**Test Scenarios:**

- 5 consecutive failures → Event routed to DLQ
- DLQ message contains original event + failure details
- DLQ message preserves session ID for ordered replay
- Replay from DLQ → Original processing behavior

## Error Handling Assertions

### 10. Retry Behavior

Transient failures MUST trigger exponential backoff retry with increasing delays (1s, 2s, 4s, 8s, 16s maximum).

**Test Scenarios:**

- Transient failure → Retry with 1-second delay
- Second failure → Retry with 2-second delay
- Fifth failure → Event moved to dead letter queue
- Permanent failure → No retry, immediate failure

### 11. Circuit Breaker

After 5 consecutive failures to any downstream service, the circuit breaker MUST open and fail fast for 30 seconds.

**Test Scenarios:**

- 5 consecutive Service Bus failures → Circuit opens
- Circuit open → Fast fail for 30 seconds
- Circuit half-open → Limited requests allowed
- Service recovery → Circuit closes

### 12. Graceful Degradation

If blob storage is unavailable, webhook processing MUST continue but log warnings about missing audit trail.

**Test Scenarios:**

- Blob storage circuit open → Processing continues
- Warning logs generated → Audit trail compromised
- Blob storage recovery → Normal audit logging resumes
- No user-facing impact from storage degradation

### 13. Invalid Signature Response

Webhooks with invalid signatures MUST receive HTTP 401 responses without any processing or storage.

**Test Scenarios:**

- Invalid signature → HTTP 401 response
- No blob storage operation attempted
- No normalization or routing attempted
- Security event logged for monitoring

## Configuration Assertions

### 14. Static Configuration

Bot subscription configuration MUST be loaded at application startup and remain immutable until restart.

**Test Scenarios:**

- Valid configuration → Application starts successfully
- Invalid configuration → Application fails to start
- Runtime configuration change → No effect until restart
- Configuration reload requires application restart

### 15. Configuration Validation

Invalid configuration (duplicate bot names, invalid event types, malformed queue names) MUST prevent application startup with clear error messages.

**Test Scenarios:**

- Duplicate bot names → Startup failure with clear error
- Invalid event type pattern → Startup failure with clear error
- Malformed queue name → Startup failure with clear error
- Missing required fields → Startup failure with clear error

### 16. Secret Caching

GitHub webhook secrets MUST be cached for maximum 5 minutes to balance performance and security.

**Test Scenarios:**

- Secret retrieved → Cached for 5 minutes
- Cache hit → No Key Vault request
- Cache expiry → New Key Vault request
- Cache miss → Key Vault request with caching

## Security Assertions

### 17. Secret Rotation

Webhook secret rotation MUST be supported without system downtime, with new secrets taking effect within 5 minutes.

**Test Scenarios:**

- Secret rotated in Key Vault → System continues operating
- Cache expiry → New secret retrieved and cached
- Old secret still valid → Processed until cache expiry
- New secret → Processed immediately after cache refresh

### 18. Audit Logging

All webhook processing activities MUST generate structured audit logs with correlation IDs for end-to-end tracing.

**Test Scenarios:**

- Webhook received → Correlation ID generated
- All log messages → Include correlation ID
- End-to-end trace → Followable via correlation ID
- Structured logging → JSON format with standard fields

### 19. Rate Limiting

Repeated authentication failures from the same IP address MUST trigger rate limiting after 10 failures in 5 minutes.

**Test Scenarios:**

- 10 auth failures from same IP → Rate limiting triggered
- Rate limited IP → HTTP 429 responses
- Time window expiry → Rate limiting reset
- Different IP → Independent rate limiting

## Performance Assertions

### 20. Memory Usage

Queue-Keeper MUST operate within 512MB memory limit per function instance under normal load.

**Test Scenarios:**

- Normal load processing → Memory usage < 512MB
- Memory leak detection → No unbounded growth
- Large payload processing → Memory usage remains bounded
- Garbage collection → Effective memory reclamation

### 21. Concurrent Processing

System MUST support minimum 1000 concurrent webhook requests without degradation.

**Test Scenarios:**

- 1000 concurrent requests → Response times within SLA
- Resource contention → No deadlocks or race conditions
- Connection pooling → Efficient resource usage
- Backpressure handling → Graceful degradation under load

### 22. Auto-scaling

Function instances MUST auto-scale based on queue depth (>100 messages) and resource utilization (>80% CPU/memory).

**Test Scenarios:**

- Queue depth > 100 → New instance started
- CPU > 80% → New instance started
- Memory > 80% → New instance started
- Load decreases → Instances scaled down

## Data Integrity Assertions

### 23. Payload Immutability

Raw webhook payloads stored in blob storage MUST be immutable and tamper-evident.

**Test Scenarios:**

- Payload stored → Cannot be modified
- Checksum validation → Detects tampering
- Access controls → Prevent unauthorized modification
- Audit trail → All access logged

### 24. Event Schema Validation

Normalized events MUST conform to the defined schema version and pass validation before queue delivery.

**Test Scenarios:**

- Valid event schema → Queue delivery succeeds
- Invalid event schema → Processing fails with clear error
- Schema version mismatch → Handled gracefully
- Forward compatibility → New fields ignored

### 25. Replay Idempotency

Replaying the same webhook multiple times MUST produce identical normalized events with the same event ID.

**Test Scenarios:**

- Original processing → Event ID generated
- Replay processing → Identical event ID generated
- Multiple replays → Consistent results
- Event content → Bit-for-bit identical

## Edge Case Assertions

### GitHub Behavior Edge Cases

#### Webhook Retries

GitHub may retry webhook delivery up to 5 times; system MUST handle duplicate deliveries gracefully using event ID deduplication.

**Test Scenarios:**

- Duplicate GitHub delivery → Detected via event ID
- Processing once only → Idempotent behavior
- Retry with different delivery ID → Treated as separate event

#### Large Payloads

Webhooks approaching 1MB size limit MUST be processed without memory issues or timeouts.

**Test Scenarios:**

- 1MB payload → Processed successfully
- Memory usage → Remains within limits
- Processing time → Within SLA
- Storage successful → Full payload preserved

#### Malformed JSON

Invalid JSON payloads MUST be rejected with HTTP 400 and logged for investigation.

**Test Scenarios:**

- Invalid JSON → HTTP 400 response
- Error logging → Structured error information
- No processing → Prevents downstream corruption
- Investigation data → Sufficient for debugging

### Azure Service Edge Cases

#### Service Bus Throttling

When Service Bus returns throttling errors (429), Queue-Keeper MUST implement exponential backoff and circuit breaker protection.

**Test Scenarios:**

- Service Bus 429 → Exponential backoff applied
- Persistent throttling → Circuit breaker opens
- Recovery → Circuit breaker closes
- Backpressure → Applied upstream

#### Key Vault Unavailability

If Key Vault is temporarily unavailable, cached secrets MUST continue to work until cache expiry.

**Test Scenarios:**

- Key Vault outage → Cached secrets used
- Cache expiry during outage → Extended TTL applied
- Key Vault recovery → Normal caching resumed
- No service disruption → Processing continues

#### Blob Storage Consistency

New blob writes MUST be immediately readable for replay scenarios (strong consistency required).

**Test Scenarios:**

- Write then read → Data immediately available
- Cross-region access → Consistent data returned
- Replay request → Original data retrievable
- Concurrent access → Consistent view

This comprehensive set of behavioral assertions ensures Queue-Keeper meets all functional, performance, and reliability requirements while providing clear validation criteria for testing and monitoring.
