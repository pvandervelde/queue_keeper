# Domain Vocabulary

This document defines the core concepts and terminology used throughout the Queue-Keeper system to ensure consistent understanding across all components and interfaces.

## Core Concepts

### Webhook

An HTTP POST request sent by GitHub to notify external systems about events occurring in repositories.

- **Source**: GitHub.com or GitHub Enterprise Server
- **Authentication**: HMAC-SHA256 signature in `X-Hub-Signature-256` header
- **Delivery**: Contains event type in `X-GitHub-Event` header and unique delivery ID
- **Payload**: JSON structure containing event-specific data
- **Timeout**: GitHub expects response within 10 seconds, Queue-Keeper targets <1 second

### Event

A normalized representation of a GitHub webhook after validation and transformation.

- **Event ID**: Globally unique identifier (ULID or UUID) for deduplication
- **Event Type**: GitHub event classification (pull_request, issues, push, etc.)
- **Repository**: Source repository information (owner, name, ID)
- **Entity**: Primary object involved in the event (PR, issue, branch, etc.)
- **Session ID**: Grouping identifier for ordered processing
- **Payload**: Original GitHub webhook data preserved for downstream processing

### Entity

The primary GitHub object that an event relates to, used for session-based ordering.

- **Pull Request**: Identified by PR number, enables sequential processing of PR lifecycle
- **Issue**: Identified by issue number, enables sequential processing of issue lifecycle
- **Branch**: Identified by branch name, enables sequential processing of branch events
- **Repository**: The repository itself, used for repository-level events
- **Release**: Identified by release tag, used for release management events
- **Unknown**: Fallback for unrecognized or entity-less events

### Session

A logical grouping mechanism that ensures related events are processed in chronological order.

- **Session ID**: String identifier following pattern `{owner}/{repo}/{entity_type}/{entity_id}`
- **Purpose**: Prevents race conditions when multiple events affect the same GitHub entity
- **Scope**: Events with identical session IDs must be processed sequentially
- **Parallelism**: Events with different session IDs may be processed concurrently
- **Implementation**: Azure Service Bus sessions or AWS SQS FIFO message groups

### Bot

An automation service that consumes normalized events from Queue-Keeper.

- **Examples**: Task-Tactician, Merge-Warden, Spec-Sentinel
- **Queue**: Each bot has a dedicated Service Bus queue for event delivery
- **Subscription**: Static configuration defining which event types the bot receives
- **Processing**: Responsible for acknowledging, rejecting, or retrying events
- **Ordering**: May require session-based ordered delivery or parallel processing

### Queue

A message queue that delivers events from Queue-Keeper to bot consumers.

- **Provider**: Azure Service Bus (primary) or AWS SQS (future)
- **Sessions**: Enabled for bots requiring ordered processing
- **Dead Letter**: Failed events automatically routed after retry exhaustion
- **Naming**: Convention `queue-keeper-{bot-name}` (e.g., `queue-keeper-task-tactician`)
- **Configuration**: TTL, retry limits, and session settings defined per bot

### Routing

The process of determining which bot queues should receive a normalized event.

- **Subscription**: Static mapping from event types to bot queues
- **One-to-Many**: Single event may be delivered to multiple bot queues
- **Filtering**: Bots specify event type patterns they want to receive
- **Atomicity**: Either all target queues receive the event or the operation fails

## Processing Concepts

### Signature Validation

The cryptographic verification that a webhook originated from GitHub.

- **Algorithm**: HMAC-SHA256 using shared webhook secret
- **Header**: `X-Hub-Signature-256` contains computed signature
- **Secret**: Retrieved from Azure Key Vault, cached for 5 minutes
- **Security**: Invalid signatures result in HTTP 401 and no further processing
- **Timing**: Uses constant-time comparison to prevent timing attacks

### Normalization

The transformation of raw GitHub webhook payloads into the standard event schema.

- **Input**: Raw webhook JSON payload and headers
- **Output**: EventEnvelope with standardized structure
- **Entity Detection**: Extracts entity type and ID from payload structure
- **Session Generation**: Creates session ID based on entity information
- **Metadata**: Adds processing timestamps, correlation IDs, and routing information

### Retry Logic

The mechanism for handling transient failures during event processing.

- **Strategy**: Exponential backoff with jitter (100ms → 1.6s → 6.4s → 25.6s)
- **Maximum Attempts**: 5 retries for transient failures
- **Backoff Factor**: 2.0x multiplier with ±25% jitter
- **Failure Classification**: Distinguishes permanent vs transient errors
- **Dead Letter**: Events exceeding retry limit moved to DLQ with failure context

### Circuit Breaker

A protection mechanism that prevents cascading failures to downstream services.

- **Failure Threshold**: Opens after 5 consecutive failures to a service
- **Open State**: Fast-fails requests for 30 seconds
- **Half-Open**: Tests service recovery with limited requests
- **Services**: Applied to Service Bus, Blob Storage, and Key Vault
- **Recovery**: Automatic reset when service calls succeed

## Infrastructure Concepts

### Blob Storage

Azure Storage container for persisting raw webhook payloads.

- **Purpose**: Audit trail and replay capability
- **Path Structure**: `{year}/{month}/{day}/{event_id}.json`
- **Metadata**: Event type, repository, timestamp, signature validation status
- **Immutability**: Payloads never modified after initial storage
- **Retention**: Configurable retention policy for compliance

### Service Bus

Azure Service Bus namespace containing bot queues and dead letter queues.

- **Sessions**: Enabled for ordered message delivery per session ID
- **Dead Letter**: Automatic routing of failed messages after retry exhaustion
- **Duplicate Detection**: 10-minute window for GitHub webhook retry deduplication
- **TTL**: 24-hour message time-to-live for automatic cleanup
- **Scaling**: Auto-scaling based on queue depth and processing demand

### Key Vault

Azure Key Vault storing GitHub webhook secrets and other sensitive configuration.

- **Secrets**: GitHub webhook secrets per repository or organization
- **Access**: Managed Identity authentication for Queue-Keeper
- **Caching**: 5-minute cache TTL for performance
- **Rotation**: Supports secret rotation without system downtime

### Dead Letter Queue

Special queue for events that failed processing after maximum retry attempts.

- **Purpose**: Preserves failed events for manual investigation and replay
- **Content**: Original event plus failure metadata and retry history
- **Replay**: Administrative interface for reprocessing failed events
- **Monitoring**: Alerts when DLQ depth exceeds threshold

## Error Concepts

### Transient Error

Temporary failure condition that may succeed if retried.

- **Examples**: Network timeouts, service throttling, temporary service unavailability
- **Response**: Exponential backoff retry up to maximum attempts
- **Circuit Breaker**: May trigger protection if failures persist

### Permanent Error

Failure condition that will not succeed if retried.

- **Examples**: Invalid webhook signature, malformed payload, configuration errors
- **Response**: Immediate failure with appropriate HTTP status code
- **No Retry**: Skip retry logic to avoid wasted resources

### Graceful Degradation

System behavior when non-critical services are unavailable.

- **Blob Storage**: Continue processing without audit storage, log warnings
- **Key Vault**: Use cached secrets beyond normal expiry during outages
- **Monitoring**: Reduce telemetry granularity if Application Insights unavailable

## Observability Concepts

### Correlation ID

Unique identifier that traces a request across all system components.

- **Source**: Generated at webhook receipt or extracted from GitHub delivery ID
- **Propagation**: Included in all log messages and telemetry events
- **Purpose**: Enables end-to-end tracing and debugging

### Distributed Tracing

Observability technique that tracks requests across multiple services.

- **Implementation**: W3C Trace Context standard
- **Spans**: Each processing stage creates a span with timing and metadata
- **Correlation**: Links Queue-Keeper processing with downstream bot execution

### Health Check

Endpoint that reports system and dependency health status.

- **Dependencies**: Service Bus connectivity, Key Vault access, Blob Storage availability
- **Status**: Healthy, Degraded, or Unhealthy with specific failure details
- **Monitoring**: Used by Azure health probes and monitoring systems

This vocabulary establishes the shared language for Queue-Keeper architecture and implementation, ensuring consistent terminology across all system components.
