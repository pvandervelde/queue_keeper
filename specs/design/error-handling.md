# Error Handling Strategy

## Overview

Queue-Keeper's error handling strategy ensures system reliability while meeting the <1 second GitHub webhook response SLA. The design separates transient from permanent failures, applies appropriate recovery mechanisms, and maintains observability into system health.

## Error Classification Strategy

### Error Categories and Response Patterns

**Client Errors (4xx - No Retry)**

- **Invalid Webhook Signatures**: Return HTTP 401 immediately, log security event
- **Malformed Payloads**: Return HTTP 400, preserve payload for analysis
- **Missing Headers**: Return HTTP 400, log GitHub integration issues
- **Payload Too Large**: Return HTTP 413, track repository payload patterns
- **Rate Limited**: Return HTTP 429 with Retry-After header

**Server Errors (5xx - Conditional Retry)**

- **Internal Processing Errors**: Retry with exponential backoff up to 3 attempts
- **Service Unavailable**: Apply circuit breaker, graceful degradation
- **Resource Exhaustion**: Scale horizontally, temporary backpressure
- **Timeout Errors**: Increase timeout thresholds, retry with longer limits

**Network Errors (Retry with Backoff)**

- **Connection Timeouts**: Exponential backoff with jitter (100ms → 1.6s)
- **DNS Resolution Failures**: Retry with alternative DNS servers
- **TLS Handshake Failures**: Retry with certificate validation logging
- **Network Unreachable**: Circuit breaker activation after 5 failures

**External Service Errors (Circuit Breaker Protection)**

- **Azure Service Bus Throttling**: Backpressure control, rate reduction
- **Blob Storage Timeouts**: Graceful degradation (skip storage, continue processing)
- **Key Vault Unavailability**: Fallback to cached secrets with expiry extension
- **Authentication Failures**: No retry, immediate alert to operations team

## Retry Policy Framework

### Exponential Backoff Strategy

**Retry Configuration Parameters**:

- **Maximum Attempts**: 5 retries for transient failures, 0 for permanent failures
- **Base Delay**: 100ms initial delay between attempts
- **Maximum Delay**: 16 seconds cap to prevent excessive delays
- **Backoff Factor**: 2.0 (doubles delay each attempt)
- **Jitter**: ±25% randomization to prevent thundering herd effects

**Retry Schedule Design**:

| Attempt | Delay Range | Cumulative Time |
|---------|-------------|-----------------|
| 1st Retry | 75-125ms | ~100ms |
| 2nd Retry | 150-250ms | ~350ms |
| 3rd Retry | 300-500ms | ~750ms |
| 4th Retry | 600-1000ms | ~1.6s |
| 5th Retry | 1200-2000ms | ~3.2s |

**Rationale**: Total retry window under 4 seconds preserves GitHub webhook SLA while providing sufficient recovery opportunity for transient issues.

### Circuit Breaker Protection

**Circuit Breaker States and Transitions**:

- **Closed State**: Normal operation, tracking failure rate
- **Open State**: Fast-fail mode, rejecting requests immediately
- **Half-Open State**: Testing recovery with limited request throughput

**Configuration by Service**:

| Service | Failure Threshold | Success Threshold | Timeout | Reset Period |
|---------|-------------------|-------------------|---------|--------------|
| **Service Bus** | 5 failures | 3 successes | 30 seconds | 5 minutes |
| **Blob Storage** | 3 failures | 2 successes | 10 seconds | 2 minutes |
| **Key Vault** | 3 failures | 2 successes | 15 seconds | 3 minutes |

**Circuit Breaker Rationale**: Protects system from cascading failures while providing rapid recovery when services restore. Thresholds calibrated for each service's typical failure patterns.

## Service-Specific Error Strategies

### Azure Service Bus Error Handling

**Error Response Mapping**:

| Service Bus Error | Response Strategy | Rationale |
|-------------------|------------------|-----------|
| **429 Throttling** | Circuit breaker + exponential backoff | Protect against rate limit violations |
| **503 Service Busy** | Short backoff retry (no circuit breaker) | Temporary Azure service capacity issues |
| **401/403 Auth Failures** | Immediate failure + security alert | Authentication issues require manual intervention |
| **404 Queue Not Found** | Immediate failure + configuration alert | Indicates deployment or configuration problems |
| **Network Timeouts** | Circuit breaker protection + retry | Transient connectivity issues |

**Backpressure Control Strategy**:

- **Throttling Detection**: Monitor 429 responses and Service Bus quotas
- **Rate Reduction**: Halve send rate when throttling detected
- **Permit System**: Use semaphore to limit concurrent Service Bus operations
- **Recovery**: Gradual rate increase when throttling subsides

### Azure Blob Storage Error Handling

**Graceful Degradation Philosophy**:
Blob storage failures MUST NOT prevent webhook processing from continuing, as payload storage is for audit/replay purposes rather than core functionality.

**Error Handling Approach**:

| Blob Storage Error | Response Strategy | Impact |
|--------------------|------------------|--------|
| **Circuit Breaker Open** | Skip storage, log warning, continue processing | No impact on webhook SLA |
| **Throttling (429)** | Apply backoff, eventual consistency acceptable | Delayed audit capability |
| **Network Timeouts** | Retry with longer timeout, then skip | Minimal performance impact |
| **Authentication Failures** | No retry, immediate alert, continue processing | Operations team notification |

### Azure Key Vault Error Handling

**Secret Caching Strategy**:
Implement multi-layer fallback to ensure webhook signature validation continues during Key Vault outages.

**Fallback Hierarchy**:

1. **Fresh Cache**: Use unexpired cached secrets (5-minute TTL)
2. **Expired Cache Extension**: Use expired cache during outages with extended TTL
3. **Emergency Fallback**: Alert operations team, reject webhooks with HTTP 503

**Cache Management**:

- **Normal Operation**: 5-minute cache TTL for performance
- **Key Vault Outage**: Extend cache TTL to 30 minutes for emergency operation
- **Secret Rotation**: Immediate cache invalidation when new secrets detected

## Dead Letter Queue Management

### Dead Letter Queue Strategy

## Dead Letter Queue Management

### Dead Letter Event Strategy

**Event Preservation Requirements**:
All failed webhook events MUST be preserved with complete context for debugging and replay capabilities.

**Dead Letter Event Information**:

- **Event Identity**: Original event ID and bot subscription ID
- **Failure Context**: Failure count, timestamps (first/last failure)
- **Error Details**: Structured error information and retry attempt history
- **Original Payload**: Complete webhook payload for replay
- **Routing Information**: Maintains session affinity for ordered replays

### Dead Letter Processing Philosophy

**Three-Tier Failure Management**:

1. **Retry Tier**: Standard exponential backoff retry within SLA window
2. **Dead Letter Tier**: Events that exceeded retry limits, available for manual replay
3. **Poison Message Tier**: Events that consistently fail replay attempts

**Replay Strategy**:

- **Manual Triggers**: Operations team can replay events by bot subscription, time range, or error type
- **Batch Processing**: Support bulk replay operations with progress tracking
- **Session Preservation**: Maintain ordering guarantees during replay operations
- **Failure Escalation**: Events that fail replay move to poison message tier

**Replay Success Criteria**:

- Successful processing through webhook pipeline
- Proper session ordering maintained
- Metrics and observability events recorded

## Error Monitoring and Alerting

### Error Metrics Strategy

**Key Error Metrics**:

| Metric Category | Purpose | Alerting Threshold |
|-----------------|---------|-------------------|
| **Error Rate by Category** | Track business vs infrastructure failures | >1% over 5 minutes |
| **Error Rate by Service** | Identify problematic Azure services | >5% for individual service |
| **Retry Count Distribution** | Monitor retry policy effectiveness | >50% events requiring retry |
| **Circuit Breaker State** | Track service protection activation | Any circuit breaker open >10 minutes |
| **Dead Letter Queue Depth** | Monitor failure accumulation | >100 events in DLQ |

### Error Alerting Strategy

**Alert Severity Levels**:

| Severity | Response Time | Escalation | Purpose |
|----------|---------------|------------|---------|
| **Warning** | Business hours | Platform team | Early indicators, trend monitoring |
| **Medium** | 4 hours | Platform team + manager | Service degradation, manual intervention needed |
| **High** | 1 hour | Platform team + on-call | Service reliability impact |
| **Critical** | 15 minutes | Platform team + on-call + incident commander | Customer-facing outage |

**Alert Conditions**:

- **High Error Rate**: >1% errors over 5 minutes (Warning), >5% over 2 minutes (Critical)
- **Circuit Breaker Open**: Any service circuit breaker open >10 minutes (High)
- **Dead Letter Queue Growth**: >100 messages accumulated (Medium)
- **Authentication Failures**: >10 failures/hour pattern (High, includes security team)
- **Service Unavailable**: Azure service returning 503s consistently (High)

**Alert Routing Strategy**:

- **Platform Team**: All error-related alerts during business hours
- **On-Call Rotation**: High/Critical alerts requiring immediate response
- **Security Team**: Authentication failures and potential security incidents
- **Incident Commander**: Critical alerts affecting customer SLA

This comprehensive error handling specification ensures Queue-Keeper can gracefully handle failures while maintaining system reliability and providing clear observability into error conditions.
