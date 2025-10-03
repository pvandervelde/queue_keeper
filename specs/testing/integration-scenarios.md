# Integration Testing Scenarios

## Overview

Queue-Keeper integration testing validates end-to-end functionality across Azure services, GitHub webhook processing, and bot ecosystem integration. These scenarios ensure system reliability under real-world conditions and validate architectural decisions through comprehensive testing.

## Azure Service Integration Testing

### Service Bus Integration Scenarios

**Scenario 1: Session-Ordered Message Processing**

*Purpose*: Validate ordered processing of related webhook events

*Setup*:

- Configure test bot queue with session support enabled
- Generate GitHub webhook sequence (push, PR, commit status)
- Each event shares same repository/PR session ID

*Test Steps*:

1. Send 3 related webhook events in rapid succession
2. Verify all messages route to same session ID in bot queue
3. Confirm sequential processing order maintained
4. Validate no message loss or duplication

*Success Criteria*:

- All 3 messages delivered with identical session ID
- Processing order matches submission order
- No duplicate messages in queue or dead letter queue

**Scenario 2: Dead Letter Queue Handling**

*Purpose*: Validate failure recovery and replay mechanisms

*Setup*:

- Configure bot queue with 3 retry attempts
- Simulate downstream bot service unavailability
- Prepare webhook events for processing

*Test Steps*:

1. Send webhook event to unavailable bot queue
2. Verify 3 retry attempts with exponential backoff
3. Confirm message routes to dead letter queue after retries
4. Restore bot service availability
5. Replay message from dead letter queue

*Success Criteria*:

- Exactly 3 retry attempts before dead letter routing
- Message preserved with complete metadata in dead letter queue
- Successful processing after replay operation

### Blob Storage Integration Scenarios

**Scenario 3: Audit Trail Persistence**

*Purpose*: Ensure webhook payload storage reliability and replay capability

*Setup*:

- Configure blob storage with partitioned container structure
- Prepare webhook payloads of varying sizes (1KB to 100KB)
- Monitor storage account metrics

*Test Steps*:

1. Send webhook events and verify immediate blob storage
2. Validate partition structure (year/month/day/hour)
3. Test blob read performance for replay scenarios
4. Verify payload immutability and integrity

*Success Criteria*:

- All payloads stored within 500ms of webhook receipt
- Correct partition path structure maintained
- Payload content matches original webhook exactly
- Blobs immediately readable for replay operations

**Scenario 4: Storage Account Failover**

*Purpose*: Validate graceful degradation when blob storage unavailable

*Setup*:

- Configure circuit breaker for blob storage operations
- Simulate storage account throttling/unavailability
- Monitor webhook processing during storage issues

*Test Steps*:

1. Trigger blob storage circuit breaker (5 consecutive failures)
2. Send webhook events during storage unavailability
3. Verify webhook processing continues without storage
4. Confirm circuit breaker recovery after storage restoration

*Success Criteria*:

- Webhook processing maintains <1s response time during storage outage
- No webhook processing failures due to storage unavailability
- Circuit breaker opens/closes according to configuration
- Storage operations resume automatically after restoration

### Key Vault Integration Scenarios

**Scenario 5: Secret Caching and Rotation**

*Purpose*: Validate secret management and caching behavior

*Setup*:

- Configure webhook secrets in Key Vault
- Enable 5-minute cache TTL for secrets
- Prepare GitHub webhook with valid signature

*Test Steps*:

1. Send webhook, verify secret retrieval and caching
2. Update secret in Key Vault (simulate rotation)
3. Send webhooks within cache window (should succeed)
4. Wait for cache expiry, send webhook (should fetch new secret)
5. Verify signature validation with both old and new secrets

*Success Criteria*:

- Initial secret fetch completes within 100ms
- Cached secret used for subsequent requests within TTL
- New secret retrieved after cache expiry
- No authentication failures during proper rotation timing

## GitHub Webhook Integration Testing

### Webhook Signature Validation Scenarios

**Scenario 6: Signature Algorithm Compliance**

*Purpose*: Ensure GitHub webhook signature validation accuracy

*Setup*:

- Generate test webhooks with known HMAC-SHA256 signatures
- Include edge cases (empty payload, large payload, special characters)
- Test with both valid and invalid signatures

*Test Steps*:

1. Send webhook with valid GitHub signature
2. Send webhook with invalid signature
3. Send webhook with missing signature header
4. Send webhook with malformed signature format

*Success Criteria*:

- Valid signatures accepted and processed normally
- Invalid signatures rejected with HTTP 401
- Missing signatures rejected with HTTP 400
- All rejection responses returned within 100ms

### Multi-Repository Testing Scenarios

**Scenario 7: Concurrent Repository Processing**

*Purpose*: Validate independent processing of different repositories

*Setup*:

- Configure 5 test repositories with different bot subscriptions
- Generate concurrent webhook events from all repositories
- Monitor session ID generation and queue routing

*Test Steps*:

1. Send simultaneous webhooks from all 5 repositories
2. Verify each repository generates unique session IDs
3. Confirm proper routing to subscribed bot queues only
4. Validate no cross-repository event contamination

*Success Criteria*:

- All repositories process independently and concurrently
- Session IDs remain repository-specific
- Bot subscriptions route to correct queues only
- No processing delays due to concurrent load

**Scenario 8: Large Repository Burst Handling**

*Purpose*: Test CI/CD pipeline burst scenarios

*Setup*:

- Simulate active repository with multiple bot subscriptions
- Generate 50 webhook burst within 30 seconds (push, PR, checks)
- Monitor queue depths and processing performance

*Test Steps*:

1. Send rapid burst of 50 related webhooks (same repository)
2. Monitor Container Apps auto-scaling behavior
3. Verify all webhooks route to correct bot queues
4. Confirm end-to-end processing within SLA

*Success Criteria*:

- All 50 webhooks processed within 2 minutes end-to-end
- Container Apps scales appropriately for load
- No queue backup >1000 messages
- Response time remains <1s throughout burst

## Bot Ecosystem Integration Testing

### Cross-Bot Communication Scenarios

**Scenario 9: Multi-Bot Event Coordination**

*Purpose*: Validate event routing to multiple bot subscriptions

*Setup*:

- Configure single repository with 3 bot subscriptions
- Each bot subscribes to overlapping GitHub event types
- Monitor queue delivery and bot processing coordination

*Test Steps*:

1. Send GitHub webhook matching all 3 bot subscriptions
2. Verify event delivered to all 3 bot queues
3. Confirm each bot processes event independently
4. Validate no duplicate processing across bots

*Success Criteria*:

- Single webhook generates 3 separate queue messages
- Each bot queue receives identical event payload
- Session IDs consistent across all bot queues
- Processing occurs in parallel without conflicts

### Configuration Change Impact Testing

**Scenario 10: Bot Subscription Configuration Updates**

*Purpose*: Test behavior during configuration changes

*Setup*:

- Deploy initial bot subscription configuration
- Prepare configuration update (add new bot, modify subscriptions)
- Generate test webhooks before, during, and after updates

*Test Steps*:

1. Send webhooks with initial configuration
2. Deploy configuration update (container restart)
3. Send webhooks immediately after restart
4. Verify new subscription patterns take effect

*Success Criteria*:

- No webhook processing during brief restart window
- New configuration active within 30 seconds of restart
- No events lost during configuration transition
- Historical events route according to configuration at time of receipt

## Error Injection and Recovery Testing

### Network Resilience Scenarios

**Scenario 11: Azure Service Network Partitions**

*Purpose*: Validate network failure recovery mechanisms

*Setup*:

- Configure network policies to simulate service isolation
- Enable circuit breakers for all Azure service dependencies
- Monitor service recovery and circuit breaker behavior

*Test Steps*:

1. Isolate Service Bus network access (simulate partition)
2. Send webhooks during Service Bus unavailability
3. Restore Service Bus network connectivity
4. Verify automatic recovery and queue processing resumption

*Success Criteria*:

- Circuit breaker opens within 30 seconds of network failure
- Webhook processing fails fast during partition
- Circuit breaker closes within 1 minute of recovery
- Queued messages process normally after restoration

### Data Consistency Scenarios

**Scenario 12: Partial Failure Recovery**

*Purpose*: Ensure data consistency during partial system failures

*Setup*:

- Configure monitoring for blob storage and Service Bus operations
- Simulate blob storage failure after successful queue delivery
- Test replay scenarios from audit trail

*Test Steps*:

1. Send webhook that successfully routes to queues
2. Simulate blob storage failure after queue success
3. Verify webhook processing reported as successful to GitHub
4. Confirm event available for replay from queues despite storage failure

*Success Criteria*:

- GitHub receives success response despite blob storage failure
- Event successfully delivered to all configured bot queues
- Partial failure logged for operational awareness
- System continues processing subsequent webhooks normally

## Contract Testing with GitHub

### GitHub Webhook Schema Validation

**Scenario 13: GitHub Event Schema Compliance**

*Purpose*: Validate handling of GitHub webhook schema evolution

*Setup*:

- Collect current GitHub webhook examples for all event types
- Generate test cases with missing optional fields
- Test with unknown/future fields in payload

*Test Steps*:

1. Send current GitHub webhook schemas for all supported events
2. Send webhooks with missing optional fields
3. Send webhooks with additional unknown fields
4. Verify normalization handles all variations gracefully

*Success Criteria*:

- All current GitHub webhook schemas process successfully
- Missing optional fields don't cause processing failures
- Unknown fields are preserved in audit trail
- Normalized events maintain consistent schema regardless of input variations

### GitHub Retry Behavior Simulation

**Scenario 14: GitHub Webhook Retry Handling**

*Purpose*: Validate idempotent processing of GitHub retry attempts

*Setup*:

- Configure GitHub webhook retry simulation (same payload, different delivery ID)
- Monitor event deduplication and idempotency behavior
- Test with both successful and failed initial processing

*Test Steps*:

1. Send initial webhook with delivery ID A
2. Send identical webhook with delivery ID B (simulates GitHub retry)
3. Verify deduplication prevents duplicate processing
4. Confirm both deliveries logged for audit purposes

*Success Criteria*:

- Only one normalized event generated despite multiple deliveries
- Both GitHub delivery attempts logged in audit trail
- Bot queues receive event exactly once
- Idempotency keys prevent duplicate processing across system restarts

## Performance and Load Testing Integration

### Sustained Load Scenarios

**Scenario 15: 24-Hour Sustained Load Test**

*Purpose*: Validate system stability under sustained production load

*Setup*:

- Generate realistic webhook traffic pattern (100/min average, 1000/min peaks)
- Monitor all system metrics throughout test duration
- Include realistic bot processing delays and occasional failures

*Test Steps*:

1. Start sustained webhook generation at production rates
2. Monitor system performance, resource utilization, and SLA compliance
3. Inject periodic load spikes and service failures
4. Verify system stability and recovery over 24-hour period

*Success Criteria*:

- 99.9% availability maintained throughout test period
- Response time SLA (<1s) maintained under sustained load
- Resource utilization remains within acceptable limits
- No memory leaks or resource accumulation over time

This comprehensive integration testing strategy ensures Queue-Keeper operates reliably across all dependencies while maintaining performance and data integrity under real-world conditions.
