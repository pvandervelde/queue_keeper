# Configuration Management

## Overview

Queue-Keeper uses static configuration for bot subscriptions and routing rules, managed through YAML configuration files. Configuration is immutable at runtime, requiring container restart for changes to ensure consistency and simplicity.

## Configuration Architecture

### Static Configuration Approach

**Configuration Loading Strategy**

- Load YAML configuration at container startup
- Validate all configuration before service becomes ready
- Cache configuration in memory for performance
- No runtime configuration changes (requires restart for updates)

**Secret Management Integration**

- Azure Key Vault integration for sensitive values (GitHub secrets, connection strings)
- Automatic secret caching with configurable TTL (default: 5 minutes)
- Background secret refresh to handle key rotation
- Secure storage of cached secrets in memory only

**Configuration Validation Requirements**

- Validate bot names and queue name formats
- Verify event type patterns match GitHub webhook schema
- Ensure ordering configuration consistency
- Check session management settings for ordered bots

## Configuration Schema Design

### Bot Configuration Requirements

**Bot Registration Properties**

- **Identity**: Unique bot name and dedicated queue identifier
- **Event Subscriptions**: List of GitHub event types the bot processes
- **Ordering Requirements**: Whether events need ordered processing
- **Ordering Scope**: Level of ordering (none, per-entity, repository-wide)
- **Concurrency Limits**: Maximum concurrent sessions for ordered bots
- **Session Management**: Timeout settings for session-based processing

### Ordering Scope Options

**None**: Events processed in any order for maximum parallelism

- Use case: Stateless bots (notifications, logging, metrics collection)
- Performance: Highest throughput, no session overhead

**Entity**: Events ordered per individual PR/Issue/entity

- Use case: Bots tracking state changes (task management, PR workflows)
- Session ID pattern: `{repo}/{entity_type}/{entity_id}`

**Repository**: All events for repository processed in order

- Use case: Deployment coordination, repository-wide state management
- Session ID pattern: `{repo}/repository`

### Infrastructure Configuration Areas

**GitHub Integration**

- Webhook secret management through Azure Key Vault
- API rate limiting and retry configuration

**Azure Service Bus**

- Namespace and connection settings
- Queue creation and management parameters

**Storage Services**

- Blob storage for webhook payload archival
- Container and retention policy configuration

**Observability**

- OpenTelemetry configuration and sampling rates
- Application Insights integration settings

**Scalability Controls**

- Concurrency limits for webhook processing and queue routing
- Circuit breaker thresholds and recovery timeouts
- Rate limiting configuration for burst protection

### Configuration Validation Requirements

**Bot Configuration Validation**

- Bot names must be unique and follow naming conventions (1-50 characters)
- Queue names must follow Azure Service Bus naming constraints
- Event subscriptions must match valid GitHub webhook event types
- Ordering configuration must be internally consistent

**Ordering Configuration Consistency Rules**

- Bots with `ordered: false` must have `ordering_scope: none`
- Bots with `ordered: true` must specify entity or repository scope
- Ordered bots must define `max_concurrent_sessions` limits
- Session timeout values must be valid ISO 8601 durations

**Runtime Configuration Constraints**

- All referenced Azure Key Vault secrets must exist
- Service Bus namespace must be accessible
- Storage account and container must be configured
- Observability endpoints must be reachable

## Session Management Strategy

### Session ID Generation Requirements

**Dynamic Session ID Creation**

- Generate session IDs based on bot's ordering scope configuration
- No session ID needed for unordered processing (scope: none)
- Entity-scoped sessions use pattern: `{repo}/{entity_type}/{entity_id}`
- Repository-scoped sessions use pattern: `{repo}/repository`

**Event Routing Logic**

- Match incoming events against bot event subscriptions
- Generate appropriate session ID for each subscribed bot
- Route events to bot-specific queues with session context
- Track routing results for monitoring and debugging

**Session Lifecycle Management**

- Session timeout configuration per bot
- Maximum concurrent session limits to prevent resource exhaustion
- Automatic session cleanup for expired or completed sessions

## Container Deployment Configuration

### Container Architecture Requirements

**Multi-Stage Build Strategy**

- Build binary in Rust build environment
- Deploy to minimal runtime container (Debian slim)
- Non-root user execution for security
- Health check endpoints for container orchestration

**Configuration Management**

- YAML configuration mounted as container volume
- Environment variables for runtime settings
- Azure Key Vault integration for secrets
- Configuration validation before service readiness

### Kubernetes Deployment Requirements

**Scalability Configuration**

- Horizontal pod autoscaling based on CPU/memory utilization
- Minimum 2 replicas for availability
- Maximum 10 replicas for cost control
- Resource limits: 512Mi memory, 500m CPU per pod

**Service Configuration**

- Internal ClusterIP service for webhook endpoint
- Load balancing across multiple pod replicas
- Health check probes for liveness and readiness
- Graceful shutdown handling for configuration changes

## Scalability Considerations

### Webhook Burst Handling

**Backpressure Control Requirements**

- Semaphore-based concurrency limits for webhook processing
- Separate rate limits for queue routing operations
- Circuit breaker pattern for downstream service protection
- Graceful degradation during system overload

**Processing Pipeline Design**

- Webhook signature validation before resource allocation
- Circuit breaker integration for Service Bus operations
- Error classification for retry vs. immediate failure
- Request queuing with overflow handling

**Event Processing Flow**

- Webhook validation and normalization
- Routing semaphore acquisition for queue operations
- Bot queue routing based on configuration
- Result tracking with trace context and processing metrics

This configuration management approach provides:

1. **Flexible Ordering**: Support for no ordering, per-entity ordering, and repository-level ordering
2. **Container-Based Architecture**: Always-on containers with cached configuration and secrets
3. **Scalability Controls**: Built-in backpressure and circuit breaker mechanisms
4. **Operational Simplicity**: Configuration changes require container restart but provide strong consistency
5. **Performance Optimization**: In-memory caching of configuration and secrets with automatic refresh
