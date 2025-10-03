# Platform Requirements

## Overview

Queue-Keeper's platform requirements define the Azure service dependencies, constraints, and technical specifications necessary for reliable webhook processing. These requirements ensure proper service selection, configuration, and operational characteristics across the Azure ecosystem.

## Azure Service Dependencies

### Core Platform Services

| Service | Tier/SKU | Rationale | Key Constraints |
|---------|----------|-----------|-----------------|
| **Azure Container Apps** | Consumption Plan | Auto-scaling, always-on capability | 30-replica limit, 2GB memory max per replica |
| **Azure Service Bus** | Standard Tier | Session support, dead letter queues | 1GB namespace quota, 256KB message size limit |
| **Azure Blob Storage** | General Purpose v2, Hot Tier | Audit trail storage, replay capability | Strong consistency, 5TB container limit |
| **Azure Key Vault** | Standard Tier | Webhook secret management | 25,000 operations/10s limit, soft delete enabled |

### Supporting Platform Services

| Service | Configuration | Purpose | Requirements |
|---------|---------------|---------|--------------|
| **Azure Front Door** | Standard Tier | DDoS protection, WAF, global load balancing | Custom domain support, SSL termination |
| **Application Insights** | Workspace-based | Telemetry, distributed tracing | 90-day retention minimum |
| **Azure Monitor** | Standard | Metrics, alerting, log aggregation | Integration with Service Bus metrics |
| **Azure Active Directory** | Managed Identity | Service-to-service authentication | System-assigned identity for Container Apps |

## Service Bus Requirements

### Queue Configuration Requirements

**Session-Enabled Queues**: All bot queues MUST support sessions for ordered message processing.

**Dead Letter Queue Configuration**:

- **Max Delivery Count**: 5 attempts before dead letter routing
- **Default TTL**: 14 days for message retention
- **Dead Letter TTL**: 30 days for failed message analysis
- **Lock Duration**: 5 minutes for processing time allowance

**Namespace Quotas and Limits**:

- **Throughput Units**: Minimum 1 TU, auto-scale to 20 TU maximum
- **Connection Limits**: 1,000 concurrent connections per namespace
- **Message Size**: 256KB maximum (GitHub webhook constraint)
- **Queue Depth Monitoring**: Alert at 10,000 messages per queue

### Pricing Tier Rationale

**Standard Tier Selection**:

- **Sessions Support**: Required for ordered message processing
- **Dead Letter Queues**: Essential for failure handling and replay
- **Duplicate Detection**: Prevents processing same webhook multiple times
- **Cost Efficiency**: Premium tier unnecessary for current scale requirements

## Blob Storage Requirements

### Storage Account Configuration

**Performance Tier**: Standard performance sufficient for webhook audit requirements.

**Replication Strategy**:

- **Primary**: Locally Redundant Storage (LRS) for cost efficiency
- **Backup Consideration**: Geo-redundant storage (GRS) for compliance if required

**Container Structure**:

```
webhook-payloads/
├── year=2025/
│   ├── month=10/
│       ├── day=01/
│           ├── hour=14/
│               └── {event-id}.json
```

**Retention Policy**:

- **Hot Tier**: 90 days for active replay scenarios
- **Cool Tier**: 91-365 days for compliance and analysis
- **Archive Tier**: >365 days for long-term compliance

### Access Patterns and Performance

**Consistency Requirements**:

- **Strong Consistency**: New blobs MUST be immediately readable for replay scenarios
- **Read Performance**: 99.9% read requests <100ms latency
- **Write Performance**: 99% write requests <500ms latency

**Access Control**:

- **Managed Identity**: Container Apps access via system-assigned identity
- **RBAC Roles**: Storage Blob Data Contributor for Queue-Keeper service
- **Network Access**: Private endpoint recommended for production

## Key Vault Requirements

### Secret Management Strategy

**Secret Types and Rotation**:

| Secret Type | Rotation Frequency | Access Pattern | Caching Strategy |
|-------------|-------------------|----------------|------------------|
| **GitHub Webhook Secrets** | On-demand (security events) | High frequency (every request) | 5-minute cache TTL |
| **Service Bus Connection Strings** | Quarterly | Application startup | Application lifetime cache |
| **Storage Account Keys** | Bi-annually | Moderate frequency | 1-hour cache TTL |

**Access Control Requirements**:

- **Managed Identity**: Container Apps access via system-assigned identity
- **RBAC**: Key Vault Secrets User role for runtime access
- **Network Security**: Private endpoint for production environments
- **Audit Logging**: All secret access logged to Azure Monitor

### Performance and Availability

**Operation Limits**:

- **Standard Tier**: 25,000 operations per 10-second period
- **Queue-Keeper Usage**: ~100 operations/minute under normal load
- **Burst Capacity**: 1,000 operations/minute during high webhook activity

**Availability Requirements**:

- **SLA**: 99.9% availability (Azure Standard Tier)
- **Fallback Strategy**: Cached secrets continue operation during outages
- **Recovery Time**: <5 minutes to restore from Key Vault outages

## Container Apps Requirements

### Compute and Memory

**Resource Allocation**:

- **CPU**: 0.5 vCPU minimum, 2.0 vCPU maximum per replica
- **Memory**: 1GB minimum, 4GB maximum per replica
- **Ephemeral Storage**: 2GB temporary storage per replica

**Scaling Configuration**:

- **Minimum Replicas**: 1 (always-on requirement)
- **Maximum Replicas**: 30 (platform limit)
- **Scale Rule**: CPU >70% or Memory >80% triggers scale-out
- **Scale Rule**: Service Bus queue depth >100 messages triggers scale-out

### Networking and Security

**Network Configuration**:

- **Ingress**: External ingress enabled for GitHub webhook delivery
- **Egress**: Unrestricted egress for Azure service communication
- **Custom Domain**: Support for custom webhook endpoint URLs

**Security Requirements**:

- **HTTPS Only**: TLS 1.2 minimum, automatic certificate management
- **Managed Identity**: System-assigned identity for Azure service access
- **Environment Variables**: Secure injection of configuration values
- **Image Security**: Base images scanned for vulnerabilities

## Monitoring and Observability Requirements

### Application Insights Configuration

**Telemetry Collection**:

- **Request Telemetry**: All HTTP requests with correlation IDs
- **Dependency Telemetry**: Azure service calls with timing
- **Custom Events**: Webhook processing stages and outcomes
- **Performance Counters**: CPU, memory, thread pool metrics

**Retention and Sampling**:

- **Data Retention**: 90 days minimum for troubleshooting
- **Sampling Rate**: 100% for error traces, 10% for successful requests
- **Custom Metrics**: Webhook processing time, queue depth, error rates

### Azure Monitor Integration

**Log Analytics Workspace**:

- **Centralized Logging**: All Azure services log to single workspace
- **Kusto Queries**: Pre-built queries for common troubleshooting scenarios
- **Alert Rules**: Proactive alerting on SLA violations and error patterns

**Metric Collection**:

- **Platform Metrics**: Automatic collection from all Azure services
- **Custom Metrics**: Application-specific metrics via Application Insights
- **Cross-Service Correlation**: Distributed tracing across service boundaries

## Compliance and Security Requirements

### Data Protection

**Data Classification**:

- **GitHub Webhook Payloads**: Potentially sensitive, encrypt at rest
- **Application Logs**: May contain repository information, retention limits apply
- **Metrics Data**: Aggregated, no PII concerns

**Encryption Requirements**:

- **Data at Rest**: Azure Storage Service Encryption (SSE) with Microsoft-managed keys
- **Data in Transit**: TLS 1.2 for all service-to-service communication
- **Key Management**: Azure Key Vault for application secrets

### Access Control

**Identity and Access Management**:

- **Service Identity**: Managed identities for all service-to-service authentication
- **Human Access**: Azure AD integration for operational access
- **Principle of Least Privilege**: Minimal permissions for each service component

**Network Security**:

- **Private Endpoints**: Recommended for Key Vault and Storage Account
- **Network Security Groups**: Restrict traffic to necessary ports and protocols
- **Azure Front Door**: WAF protection and DDoS mitigation

## Resource Planning and Limits

### Capacity Planning

**Expected Load Characteristics**:

- **Normal Operations**: 100 webhooks/minute average
- **Peak Traffic**: 1,000 webhooks/minute during CI/CD bursts
- **Growth Planning**: 10x capacity headroom for future expansion

**Service Limits Impact**:

- **Service Bus**: 1,000 messages/second limit provides 10x headroom
- **Container Apps**: 30 replica limit supports ~3,000 concurrent requests
- **Key Vault**: 25,000 operations/10s supports burst scenarios
- **Blob Storage**: No practical limits for webhook payload storage

### Cost Optimization

**Service Tier Rationale**:

- **Container Apps Consumption**: Pay-per-use scaling vs. dedicated plans
- **Service Bus Standard**: Balanced feature set vs. cost for session support
- **Blob Storage Standard**: Sufficient performance for audit trail requirements
- **Key Vault Standard**: Premium tier unnecessary for current secret volume

This platform requirements specification ensures Queue-Keeper can reliably operate within Azure's service constraints while meeting performance and security requirements.
