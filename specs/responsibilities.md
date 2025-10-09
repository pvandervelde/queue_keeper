# Component Responsibilities

This document defines the responsibilities and collaboration patterns for Queue-Keeper components using Responsibility-Driven Design (RDD) principles.

## WebhookHandler

**Responsibilities:**

- **Knows**: HTTP request format, GitHub webhook headers, response status codes
- **Does**: Accepts HTTP POST requests, extracts headers and payload, coordinates processing pipeline, returns appropriate HTTP responses

**Collaborators:**

- **SignatureValidator**: Delegates webhook signature verification
- **PayloadStorer**: Delegates raw payload persistence to blob storage
- **EventNormalizer**: Delegates event transformation and entity extraction
- **QueueRouter**: Delegates normalized event distribution to bot queues
- **ErrorManager**: Delegates error handling and retry coordination

**Roles:**

- **HTTP Endpoint**: Primary interface for GitHub webhook delivery
- **Pipeline Orchestrator**: Coordinates the webhook processing workflow
- **Response Manager**: Ensures GitHub receives proper HTTP responses within SLA

**Boundaries:**

- **In**: Raw HTTP requests from GitHub API Gateway
- **Out**: HTTP responses to GitHub, processing requests to internal components
- **Not Responsible For**: Business logic, storage operations, queue operations

---

## SignatureValidator

**Responsibilities:**

- **Knows**: HMAC-SHA256 algorithm, webhook secret retrieval, signature validation status
- **Does**: Retrieves secrets from Key Vault, validates webhook signatures, caches secrets for performance

**Collaborators:**

- **SecretProvider** (Key Vault): Retrieves GitHub webhook secrets
- **SecretCache**: Caches secrets for 5-minute TTL
- **ErrorManager**: Reports authentication failures and security events

**Roles:**

- **Security Gateway**: Ensures webhook authenticity before processing
- **Secret Manager**: Handles secret retrieval and caching
- **Authentication Oracle**: Determines webhook trustworthiness

**Boundaries:**

- **In**: Raw webhook payload and signature header
- **Out**: Authentication pass/fail decision, security audit events
- **Not Responsible For**: Secret storage, HTTP responses, business logic

---

## PayloadStorer

**Responsibilities:**

- **Knows**: Blob storage paths, metadata structure, immutable storage requirements
- **Does**: Persists raw webhook payloads to blob storage, generates audit metadata, supports replay scenarios

**Collaborators:**

- **BlobStorageProvider**: Performs actual storage operations
- **PathGenerator**: Creates immutable blob paths with timestamps
- **MetadataBuilder**: Constructs storage metadata (event type, validation status)
- **ErrorManager**: Handles storage failures with graceful degradation

**Roles:**

- **Audit Manager**: Ensures comprehensive audit trail for all webhooks
- **Replay Enabler**: Provides data source for event replay functionality
- **Compliance Guardian**: Maintains immutable records for regulatory requirements

**Boundaries:**

- **In**: Raw webhook payload and processing metadata
- **Out**: Storage confirmation and blob reference
- **Not Responsible For**: Data transformation, queue operations, business logic

---

## EventNormalizer

**Responsibilities:**

- **Knows**: GitHub event schema, entity detection logic, session ID generation rules
- **Does**: Transforms GitHub payloads to EventEnvelope format, extracts entity information, generates session IDs

**Collaborators:**

- **EntityExtractor**: Determines entity type and ID from GitHub payload
- **SessionIdGenerator**: Creates session IDs for ordered processing
- **EventIdGenerator**: Creates unique event identifiers
- **SchemaValidator**: Validates normalized event structure

**Roles:**

- **Schema Translator**: Converts GitHub-specific format to system-standard format
- **Entity Classifier**: Identifies the primary GitHub object affected by the event
- **Session Coordinator**: Enables ordered processing through session ID assignment

**Boundaries:**

- **In**: Raw GitHub webhook payload and headers
- **Out**: Normalized EventEnvelope with standardized structure
- **Not Responsible For**: Queue routing, storage operations, authentication

---

## QueueRouter

**Responsibilities:**

- **Knows**: Bot subscription configuration, queue naming conventions, routing rules
- **Does**: Determines target queues for events, distributes normalized events, ensures atomic delivery

**Collaborators:**

- **ConfigurationManager**: Retrieves bot subscription configuration
- **QueueProvider** (Service Bus): Sends messages to bot queues
- **RoutingRuleEngine**: Evaluates event types against bot subscriptions
- **ErrorManager**: Handles routing failures and dead letter scenarios

**Roles:**

- **Distribution Manager**: Routes events to appropriate bot queues
- **Configuration Interpreter**: Applies static routing rules from configuration
- **Atomicity Guarantor**: Ensures all-or-nothing delivery to target queues

**Boundaries:**

- **In**: Normalized EventEnvelope and routing configuration
- **Out**: Messages delivered to bot queues
- **Not Responsible For**: Message processing, bot logic, queue creation

---

## ConfigurationManager

**Responsibilities:**

- **Knows**: Bot subscription definitions, queue configurations, validation rules
- **Does**: Loads configuration at startup, validates configuration structure, provides configuration access

**Collaborators:**

- **ConfigurationLoader**: Reads configuration from files or environment
- **ConfigurationValidator**: Validates configuration structure and constraints
- **ErrorManager**: Reports configuration errors that prevent startup

**Roles:**

- **Configuration Authority**: Single source of truth for system configuration
- **Validation Gate**: Prevents startup with invalid configuration
- **Configuration Provider**: Supplies configuration to other components

**Boundaries:**

- **In**: Raw configuration files and environment variables
- **Out**: Validated configuration structures
- **Not Responsible For**: Runtime configuration changes, dynamic updates

---

## ErrorManager

**Responsibilities:**

- **Knows**: Error classification rules, retry policies, circuit breaker states, observability requirements
- **Does**: Classifies errors as transient vs permanent, applies retry logic, manages circuit breakers, routes failed events to DLQ

**Collaborators:**

- **RetryPolicyEngine**: Implements exponential backoff retry strategies
- **CircuitBreakerManager**: Tracks service health and applies protection
- **DeadLetterQueueProvider**: Routes failed events after retry exhaustion
- **TelemetryProvider**: Reports errors and metrics to monitoring systems

**Roles:**

- **Reliability Guardian**: Implements reliability patterns (retry, circuit breaker)
- **Error Classifier**: Distinguishes between recoverable and permanent failures
- **Observability Publisher**: Ensures error visibility through metrics and logging

**Boundaries:**

- **In**: Errors from all system components
- **Out**: Recovery actions, telemetry events, DLQ messages
- **Not Responsible For**: Business logic, success path processing, configuration

---

## Component Collaboration Patterns

### Webhook Processing Flow

```
GitHub → WebhookHandler → SignatureValidator → PayloadStorer → EventNormalizer → QueueRouter → Service Bus
                    ↓                                                                           ↓
              ErrorManager ←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←←← Bot Queues
```

**Collaboration Rules:**

1. **Sequential Processing**: Each component completes before the next begins
2. **Error Propagation**: Failures bubble up to ErrorManager for handling
3. **Atomic Operations**: Either entire pipeline succeeds or fails as a unit
4. **Timeout Respect**: All operations must complete within GitHub's 10-second timeout

### Configuration Loading Flow

```
Startup → ConfigurationManager → ConfigurationValidator → All Components
                ↓
          ErrorManager (if validation fails)
```

**Collaboration Rules:**

1. **Startup Dependency**: All components depend on valid configuration
2. **Fail-Fast**: Invalid configuration prevents application startup
3. **Immutable Configuration**: No runtime configuration changes allowed

### Error Handling Flow

```
Any Component → ErrorManager → RetryPolicyEngine → CircuitBreakerManager → TelemetryProvider
                     ↓              ↓                    ↓                       ↓
               DeadLetterQueue   Component Retry   Service Protection    Monitoring Systems
```

**Collaboration Rules:**

1. **Centralized Error Handling**: All errors flow through ErrorManager
2. **Policy-Driven Decisions**: Retry and circuit breaker policies determine response
3. **Observability First**: All errors generate telemetry for monitoring

### Graceful Degradation Patterns

**Blob Storage Failure:**

- PayloadStorer reports failure to ErrorManager
- ErrorManager allows processing to continue with warning
- EventNormalizer and QueueRouter proceed normally
- Audit trail is compromised but core functionality preserved

**Key Vault Failure:**

- SignatureValidator uses cached secrets beyond normal expiry
- ErrorManager extends cache TTL during outage
- Processing continues with degraded security posture
- Operations team alerted for manual intervention

**Service Bus Failure:**

- QueueRouter reports failure to ErrorManager
- ErrorManager opens circuit breaker for Service Bus
- PayloadStorer continues to preserve webhooks for replay
- Fast-fail response to GitHub while preserving event data

## Responsibility Matrix

| Component | Authentication | Storage | Normalization | Routing | Error Handling | Configuration |
|-----------|----------------|---------|---------------|---------|----------------|---------------|
| WebhookHandler | - | - | - | - | Coordinate | Use |
| SignatureValidator | **Own** | - | - | - | Report | Use |
| PayloadStorer | - | **Own** | - | - | Report | Use |
| EventNormalizer | - | - | **Own** | - | Report | Use |
| QueueRouter | - | - | - | **Own** | Report | Use |
| ConfigurationManager | - | - | - | - | Report | **Own** |
| ErrorManager | - | - | - | - | **Own** | Use |

**Legend:**

- **Own**: Primary responsibility and decision authority
- **Coordinate**: Orchestrates but delegates actual work
- **Report**: Provides information but does not make decisions
- **Use**: Consumes services but has no authority over them

This responsibility model ensures clear component boundaries while enabling effective collaboration for webhook processing workflows.
