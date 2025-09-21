# Infrastructure Requirements and Deployment Guide

## Overview

This document defines the infrastructure prerequisites, deployment requirements, and operational procedures for Queue-Keeper. Since Queue-Keeper is a library crate rather than a deployable service, this guide focuses on the infrastructure requirements that consuming applications must provide.

## Infrastructure Prerequisites

#### Azure Service Bus Requirements

**Namespace Configuration**:

- Premium tier for high-throughput scenarios (Standard tier acceptable for development)
- Managed identity authentication support
- Network access configuration (public endpoint or private endpoint)
- Proper resource tagging for cost allocation and management

**Queue Configuration Requirements**:

- Session support enabled for ordered message processing
- Duplicate detection configured with appropriate time window
- Dead letter queue configuration for poison message handling
- Message TTL configured based on business requirements
- Appropriate delivery count limits for retry behavior

**Security Requirements**:

- Managed identity access with appropriate RBAC roles
- Network security group rules for private endpoint scenarios
- Audit logging enabled for compliance requirements

#### AWS SQS Requirements (Alternative Provider)

**Queue Configuration**:

- FIFO queues for ordered processing scenarios
- Standard queues for high-throughput scenarios
- Dead letter queue configuration with redrive policies
- Message retention period configuration
- Visibility timeout tuning for processing patterns

**Security Configuration**:

- IAM roles and policies for queue access
- Resource-based policies for cross-account access
- Encryption at rest and in transit
- VPC endpoint configuration for private access

#### Authentication and Authorization

**Azure Requirements**:

- Managed identity (system-assigned or user-assigned)
- Service Bus Data Owner or appropriate RBAC roles
- Key Vault access policies for secret retrieval
- Application Insights contributor access for telemetry

**AWS Requirements**:

- IAM roles with appropriate SQS permissions
- AWS Systems Manager Parameter Store access for configuration
- CloudWatch metrics and logging permissions
- KMS key access for encryption scenarios

## Library Deployment Requirements

### Crate Distribution

Queue-Keeper is distributed as a Rust library crate and does not require direct deployment. However, consuming applications must meet certain requirements:

**Publication Requirements**:

- Published to crates.io with semantic versioning
- Comprehensive documentation on docs.rs
- Clear migration guides for breaking changes
- Security advisories through RustSec database

**Integration Requirements**:

- Minimum Rust version compatibility (MSRV)
- Feature flag configuration for optional dependencies
- Environment variable configuration patterns
- Logging and telemetry integration points

### Consumer Application Requirements

**Build Dependencies**:

- Rust toolchain 1.70 or later
- Target platform compilation support (x86_64-unknown-linux-gnu for Azure Functions)
- Cross-compilation capabilities for multi-architecture deployments

**Runtime Dependencies**:

- tokio async runtime with appropriate feature flags
- OpenTelemetry SDK for observability integration
- Provider-specific SDK dependencies (Azure SDK, AWS SDK)
- TLS certificates for HTTPS connections

## Consumer Application Configuration

### Configuration Requirements

**Environment-Based Configuration**:

- Support for development, staging, and production environments
- Environment-specific log levels and debugging options
- Provider-specific connection and authentication configuration
- Runtime configuration validation and error reporting

**Configuration Sources**:

- Environment variables for runtime configuration
- Configuration files for default values
- Key Vault or parameter store for sensitive values
- Command-line arguments for development scenarios

**Validation Requirements**:

- Environment name validation (development, staging, production)
- Log level validation (trace, debug, info, warn, error)
- Bot configuration validation (name, queue, events, ordering)
- Provider-specific configuration validation
- Duplicate detection for bot names and queue names

## Consumer Application Monitoring Requirements

### Health Check Requirements

**Health Endpoint Requirements**:

- HTTP health check endpoint for container orchestration platforms
- JSON response format with health status and timestamp information
- Dependency health aggregation with degraded vs unhealthy status
- Environment and version information for deployment verification

**Dependency Health Checks**:

- Message queue provider connectivity validation
- Authentication service accessibility verification
- Configuration service health monitoring
- External API dependency status checks

**Health Status Classification**:

- Healthy: All dependencies accessible and functional
- Degraded: Non-critical dependencies unavailable but core functionality intact
- Unhealthy: Critical dependencies unavailable, service non-functional

### Observability Integration Requirements

**Telemetry Requirements**:

- OpenTelemetry SDK integration for distributed tracing
- Structured logging with JSON format and trace correlation
- Metrics collection using Prometheus-compatible format
- Integration with cloud monitoring services (Application Insights, CloudWatch)

**Metrics Categories**:

- Business metrics: Messages processed, routing success rates, processing latency
- Technical metrics: Function execution duration, memory utilization, error rates
- Security metrics: Authentication failures, suspicious activity patterns
- Performance metrics: Throughput, latency percentiles, resource utilization

#### Key Performance Indicators

**Business Metrics**:

- Webhooks processed per minute
- Processing latency (p50, p95, p99)
- Error rate by repository
- Bot queue routing success rate

**Technical Metrics**:

- Function execution duration
- Memory utilization
- HTTP response codes
- Dependency call latencies

**Security Metrics**:

- Signature validation failure rate
- Rate limiting triggers
- Suspicious traffic patterns

### Alerting Strategy

#### Critical Alerts (PagerDuty)

```yaml
# Azure Monitor Alert Rules
alerts:
  - name: "Queue-Keeper Function Unavailable"
    severity: "Critical"
    condition: "avg(availability) < 0.95 over 5 minutes"
    action: "PagerDuty: Platform Engineering"

  - name: "High Error Rate"
    severity: "Critical"
    condition: "sum(errors) / sum(requests) > 0.05 over 10 minutes"
    action: "PagerDuty: Platform Engineering"

  - name: "Processing Latency High"
    severity: "High"
    condition: "avg(processing_duration_p95) > 2000ms over 15 minutes"
    action: "Slack: #platform-alerts"

  - name: "Dead Letter Queue Growing"
    severity: "High"
    condition: "count(dead_letter_messages) > 10 over 30 minutes"
    action: "Slack: #platform-alerts"

  - name: "Signature Validation Failures"
    severity: "Medium"
    condition: "sum(signature_failures) > 50 over 1 hour"
    action: "Slack: #security-alerts"
```

#### Alert Runbooks

**Queue-Keeper Function Unavailable**:

1. Check Azure Function status in portal
2. Review Application Insights for errors
3. Verify dependent services (Service Bus, Key Vault, Storage)
4. Check recent deployments for correlation
5. Escalate to Platform Engineering if not resolved in 15 minutes

**High Error Rate**:

1. Identify error patterns in Application Insights
2. Check GitHub webhook delivery status
3. Verify signature validation is working
4. Review dependency health checks
5. Consider enabling circuit breaker if widespread

### Log Management

### Logging Requirements

**Structured Logging Format**:

- JSON-formatted log entries with consistent field naming
- Trace correlation IDs for distributed tracing integration
- Event metadata including repository, event type, and processing time
- Error context with retry counts and failure reasons
- Security audit fields for authentication and authorization events

#### Log Retention Policy

- **Application Logs**: 90 days in Application Insights
- **Security Logs**: 1 year retention for compliance
- **Audit Logs**: 7 years retention in cold storage
- **Debug Logs**: 30 days retention (staging only)

### Performance Monitoring

#### SLA Monitoring

```yaml
sla_targets:
  availability: 99.9%          # 8.76 hours downtime/year
  response_time_p95: 1000ms    # 95th percentile under 1 second
  error_rate: 0.1%            # Less than 0.1% error rate
  processing_capacity: 1000/min # Sustained throughput

monitoring_queries:
  - name: "Availability SLA"
    query: |
      requests
      | where timestamp > ago(30d)
      | summarize
          total_requests = count(),
          successful_requests = countif(success == true)
      | extend availability = todouble(successful_requests) / todouble(total_requests) * 100

  - name: "Response Time SLA"
    query: |
      requests
      | where timestamp > ago(24h)
      | summarize percentile_95 = percentile(duration, 95) by bin(timestamp, 5m)
      | where percentile_95 > 1000
```

#### Capacity Planning

**Scaling Triggers**:

- Queue depth > 100 messages: Scale out
- CPU utilization > 80%: Scale up
- Memory utilization > 80%: Scale up
- Response time p95 > 2s: Investigate bottlenecks

**Resource Limits**:

- Maximum instances: 10 (Azure Function Premium plan)
- Maximum memory per instance: 1.5GB
- Maximum concurrent executions: 200 per instance

## Troubleshooting Guide

### Common Issues

#### Issue: Webhook Signature Validation Failures

**Symptoms**:

- HTTP 401 responses to GitHub webhooks
- Increase in `signature_validation_failures` metric
- GitHub webhook delivery failures

**Diagnosis**:

```bash
# Check recent signature validation errors
az monitor metrics list \
  --resource "/subscriptions/{subscription}/resourceGroups/rg-queue-keeper-prod/providers/Microsoft.Web/sites/func-queue-keeper-prod" \
  --metric "signature_validation_failures" \
  --interval PT5M

# Review Application Insights logs
az monitor log-analytics query \
  --workspace "workspace-id" \
  --analytics-query "
    traces
    | where timestamp > ago(1h)
    | where message contains 'signature validation failed'
    | order by timestamp desc
  "
```

**Resolution**:

1. Verify webhook secret in Key Vault matches GitHub configuration
2. Check for recent secret rotation events
3. Validate webhook secret permissions for Function identity
4. Test signature validation with known good payload

#### Issue: Service Bus Queue Backlog

**Symptoms**:

- Increasing queue depth in Service Bus metrics
- Processing delays for webhook events
- Timeout errors from downstream bots

**Diagnosis**:

```bash
# Check queue depth across all bot queues
az servicebus queue show \
  --resource-group rg-queue-keeper-prod \
  --namespace-name sb-offaxis-automation-prod \
  --name queue-keeper-task-tactician \
  --query "messageCount"

# Check dead letter queue for failed messages
az servicebus queue show \
  --resource-group rg-queue-keeper-prod \
  --namespace-name sb-offaxis-automation-prod \
  --name queue-keeper-dead-letter \
  --query "messageCount"
```

**Resolution**:

1. Scale up downstream bot functions if they're the bottleneck
2. Investigate and resolve dead letter queue messages
3. Consider temporarily increasing Service Bus pricing tier
4. Review session timeout configuration for stuck sessions

### Recovery Procedures

#### Webhook Replay from Blob Storage

```bash
#!/bin/bash
# Script: replay-webhooks.sh
# Purpose: Replay webhooks from blob storage for a specific time range

RESOURCE_GROUP="rg-queue-keeper-prod"
STORAGE_ACCOUNT="queuekeeperprod"
FUNCTION_APP="func-queue-keeper-prod"
START_DATE="2025-09-18T10:00:00Z"
END_DATE="2025-09-18T11:00:00Z"

echo "Replaying webhooks from $START_DATE to $END_DATE"

# List blobs in time range
az storage blob list \
  --account-name $STORAGE_ACCOUNT \
  --container-name webhooks \
  --query "[?properties.lastModified >= '$START_DATE' && properties.lastModified <= '$END_DATE'].name" \
  --output tsv | while read blob_name; do

  echo "Replaying webhook: $blob_name"

  # Download blob content
  webhook_payload=$(az storage blob download \
    --account-name $STORAGE_ACCOUNT \
    --container-name webhooks \
    --name "$blob_name" \
    --output tsv)

  # Trigger replay function
  az functionapp function invoke \
    --resource-group $RESOURCE_GROUP \
    --name $FUNCTION_APP \
    --function-name replay-webhook \
    --data "$webhook_payload"

  # Rate limit to avoid overwhelming the system
  sleep 0.1
done

echo "Webhook replay completed"
```

#### Emergency Circuit Breaker Activation

```bash
#!/bin/bash
# Script: emergency-circuit-breaker.sh
# Purpose: Manually activate circuit breaker to protect system

RESOURCE_GROUP="rg-queue-keeper-prod"
FUNCTION_APP="func-queue-keeper-prod"

echo "Activating emergency circuit breaker"

# Set circuit breaker environment variable
az functionapp config appsettings set \
  --resource-group $RESOURCE_GROUP \
  --name $FUNCTION_APP \
  --settings EMERGENCY_CIRCUIT_BREAKER=true

echo "Circuit breaker activated. Webhooks will be rejected with 503 status."
echo "To deactivate, run:"
echo "az functionapp config appsettings delete --resource-group $RESOURCE_GROUP --name $FUNCTION_APP --setting-names EMERGENCY_CIRCUIT_BREAKER"
```

### Disaster Recovery

#### Full System Recovery

1. **Assess Scope**: Determine affected components and data loss
2. **Activate DR Plan**: Follow documented disaster recovery procedures
3. **Restore Infrastructure**: Deploy from Terraform in DR region
4. **Restore Configuration**: Deploy latest application code and configuration
5. **Validate Health**: Run comprehensive health checks and smoke tests
6. **Resume Traffic**: Update DNS/traffic routing to DR environment
7. **Backfill Data**: Replay missed webhooks from blob storage
8. **Monitor Closely**: Enhanced monitoring during recovery period

#### Recovery Time Objectives

- **Infrastructure Recovery**: 30 minutes (automated Terraform deployment)
- **Application Recovery**: 15 minutes (automated CI/CD deployment)
- **Data Recovery**: Variable (depends on webhook backlog size)
- **Total RTO**: 60 minutes maximum for full system recovery
