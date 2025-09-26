# Architecture Overview

## System Context

Queue-Keeper serves as the central nervous system for OffAxis Dynamics' GitHub automation infrastructure. It acts as a reliable, ordered message broker between GitHub webhooks and downstream automation bots.

```mermaid
C4Context
    title System Context Diagram for Queue-Keeper

    Person(dev, "Developer", "Performs actions on GitHub repositories")
    System_Ext(github, "GitHub", "Source control platform generating webhook events")

    System_Boundary(offaxis, "OffAxis Dynamics") {
        System(qk, "Queue-Keeper", "Webhook intake and routing service")
        System(bots, "Automation Bots", "Task-Tactician, Merge-Warden, Spec-Sentinel")
    }

    System_Ext(azure, "Azure Services", "Cloud infrastructure and services")

    Rel(dev, github, "Creates PRs, Issues, Pushes")
    Rel(github, qk, "Sends webhooks", "HTTPS")
    Rel(qk, bots, "Routes events", "Service Bus")
    Rel(qk, azure, "Stores data, manages secrets", "HTTPS")
    Rel(bots, azure, "Reads queues", "Service Bus")
```

## Container Architecture

```mermaid
C4Container
    title Container Diagram for Queue-Keeper

    System_Ext(github, "GitHub", "Webhook source")

    Container_Boundary(azure_infra, "Azure Infrastructure") {
        Container(apim, "API Gateway", "Azure Front Door", "Load balancing, SSL termination")
        Container(app, "Queue-Keeper Container", "Azure Container Apps", "Always-on Rust HTTP service with cached secrets/config")
        Container(sb, "Service Bus", "Azure Service Bus", "Message queues with sessions")
        Container(blob, "Blob Storage", "Azure Storage", "Raw webhook persistence")
        Container(kv, "Key Vault", "Azure Key Vault", "GitHub webhook secrets")
        Container(insights, "Application Insights", "Azure Monitor", "Distributed tracing and telemetry")
    }

    Container_Boundary(bot_infra, "Bot Infrastructure") {
        Container(bot1, "Task-Tactician", "Azure Function", "Issue and PR automation")
        Container(bot2, "Merge-Warden", "Azure Function", "PR merge automation")
        Container(bot3, "Spec-Sentinel", "Azure Function", "Documentation validation")
    }

    Rel(github, apim, "POST webhook", "HTTPS")
    Rel(apim, app, "Forward request", "HTTPS")
    Rel(app, blob, "Store raw payload", "HTTPS")
    Rel(app, kv, "Get webhook secrets (cached)", "HTTPS")
    Rel(app, sb, "Send normalized events", "AMQP")
    Rel(app, insights, "Send telemetry + traces", "HTTPS")

    Rel(sb, bot1, "Trigger function", "Service Bus Trigger")
    Rel(sb, bot2, "Trigger function", "Service Bus Trigger")
    Rel(sb, bot3, "Trigger function", "Service Bus Trigger")
```

## Component Architecture

### Core Components

```mermaid
graph TB
    subgraph "Queue-Keeper Function"
        WH[Webhook Handler]
        SV[Signature Validator]
        PS[Payload Storer]
        EN[Event Normalizer]
        QR[Queue Router]
        CM[Configuration Manager]
        EM[Error Manager]
    end

    subgraph "External Dependencies"
        GH[GitHub Webhooks]
        KV[Azure Key Vault]
        BS[Blob Storage]
        SB[Service Bus Queues]
        AI[Application Insights]
    end

    GH --> WH
    WH --> SV
    SV --> KV
    SV --> PS
    PS --> BS
    PS --> EN
    EN --> QR
    QR --> CM
    QR --> SB

    WH --> EM
    SV --> EM
    PS --> EM
    EN --> EM
    QR --> EM
    EM --> AI
```

## Component Responsibilities

### Webhook Handler

- **Purpose**: HTTP endpoint for GitHub webhook delivery
- **Responsibilities**:
  - Accept HTTP POST requests from GitHub
  - Extract webhook headers and payload
  - Route to signature validation
  - Return appropriate HTTP responses
  - Handle GitHub retry behavior

### Signature Validator

- **Purpose**: Verify webhook authenticity
- **Responsibilities**:
  - Retrieve GitHub webhook secrets from Key Vault
  - Validate HMAC-SHA256 signatures
  - Cache secrets for performance
  - Log validation failures for security monitoring

### Payload Storer

- **Purpose**: Persist raw webhook data
- **Responsibilities**:
  - Store complete webhook payload to Blob Storage
  - Generate immutable blob paths with timestamps
  - Include metadata (headers, validation status)
  - Support replay scenarios

### Event Normalizer

- **Purpose**: Transform webhooks to standard schema
- **Responsibilities**:
  - Parse GitHub webhook payloads
  - Extract repository, entity, and event information
  - Generate unique event IDs and session IDs
  - Create normalized event objects
  - Handle unknown event types gracefully

### Queue Router

- **Purpose**: Distribute events to bot queues
- **Responsibilities**:
  - Read bot subscription configuration
  - Determine target queues for each event type
  - Send messages to Service Bus with session IDs
  - Handle routing failures and retries
  - Track routing metrics

### Configuration Manager

- **Purpose**: Manage bot subscription configuration
- **Responsibilities**:
  - Load static configuration at startup
  - Validate configuration format and content
  - Provide configuration access to other components
  - Support configuration updates via restart

### Error Manager

- **Purpose**: Handle failures and observability
- **Responsibilities**:
  - Implement retry logic with exponential backoff
  - Route failed events to dead letter queues
  - Generate structured logs and metrics
  - Support manual replay operations

## Data Flow Architecture

### Normal Processing Flow

```mermaid
sequenceDiagram
    participant GH as GitHub
    participant AG as API Gateway
    participant QK as Queue-Keeper
    participant KV as Key Vault
    participant BS as Blob Storage
    participant SB as Service Bus
    participant BOT as Bot Function

    GH->>AG: POST webhook
    AG->>QK: Forward request

    QK->>KV: Get webhook secret
    KV-->>QK: Return secret
    QK->>QK: Validate signature

    QK->>BS: Store raw payload
    BS-->>QK: Confirm storage

    QK->>QK: Normalize event
    QK->>QK: Determine routing

    loop For each target queue
        QK->>SB: Send normalized event
        SB-->>QK: Confirm delivery
    end

    QK-->>AG: HTTP 200 OK
    AG-->>GH: HTTP 200 OK

    SB->>BOT: Trigger function
    BOT->>SB: Process message
```

### Error Handling Flow

```mermaid
sequenceDiagram
    participant QK as Queue-Keeper
    participant SB as Service Bus
    participant DLQ as Dead Letter Queue
    participant AI as App Insights

    QK->>SB: Send event (attempt 1)
    SB-->>QK: Failure

    QK->>QK: Wait (exponential backoff)
    QK->>SB: Send event (attempt 2)
    SB-->>QK: Failure

    QK->>QK: Wait (exponential backoff)
    QK->>SB: Send event (attempt 3)
    SB-->>QK: Failure

    QK->>DLQ: Send to dead letter queue
    QK->>AI: Log failure metrics
```

## Queue Architecture

### Service Bus Topology

```mermaid
graph TB
    subgraph "Service Bus Namespace"
        Q1[queue-keeper-task-tactician<br/>Sessions: Enabled<br/>Duplicate Detection: Enabled]
        Q2[queue-keeper-merge-warden<br/>Sessions: Enabled<br/>Duplicate Detection: Enabled]
        Q3[queue-keeper-spec-sentinel<br/>Sessions: Enabled<br/>Duplicate Detection: Disabled]
        DLQ[queue-keeper-dead-letter<br/>Sessions: Disabled<br/>TTL: 30 days]
    end

    subgraph "Queue-Keeper"
        QR[Queue Router]
    end

    subgraph "Bot Functions"
        B1[Task-Tactician]
        B2[Merge-Warden]
        B3[Spec-Sentinel]
    end

    QR --> Q1
    QR --> Q2
    QR --> Q3
    QR --> DLQ

    Q1 --> B1
    Q2 --> B2
    Q3 --> B3
```

### Session Management Strategy

**Session ID Pattern**: `{repo_owner}/{repo_name}/{entity_type}/{entity_id}`

**Benefits**:

- Guarantees ordered processing per entity
- Enables parallel processing of different entities
- Prevents concurrent processing of same entity
- Supports bot scaling without ordering violations

**Configuration**:

- Session timeout: 5 minutes (auto-complete if bot doesn't acknowledge)
- Max concurrent sessions per queue: 100
- Duplicate detection window: 10 minutes

## Scalability Architecture

### Auto-Scaling Strategy

```mermaid
graph TB
    subgraph "Scaling Triggers"
        QD[Queue Depth > 100]
        CPU[CPU > 80%]
        MEM[Memory > 80%]
        LAT[Response Time > 2s]
    end

    subgraph "Scaling Actions"
        SI[Scale Instance Count]
        SR[Scale Resources]
        CB[Circuit Breaker]
    end

    subgraph "Scaling Limits"
        MAX[Max 10 Instances]
        MIN[Min 1 Instance]
        RES[Max 1GB Memory]
    end

    QD --> SI
    CPU --> SI
    MEM --> SR
    LAT --> CB

    SI --> MAX
    SI --> MIN
    SR --> RES
```

### Performance Characteristics

| Metric | Target | Monitoring |
|--------|--------|------------|
| Response Time | < 1s (95th percentile) | Application Insights |
| Throughput | 1000 req/min sustained | Service Bus metrics |
| Error Rate | < 0.1% | Application Insights |
| Queue Depth | < 100 messages | Service Bus metrics |
| Memory Usage | < 512MB per instance | Azure Monitor |
| CPU Usage | < 80% per instance | Azure Monitor |

## Security Architecture

### Authentication Flow

```mermaid
sequenceDiagram
    participant GH as GitHub
    participant QK as Queue-Keeper
    participant KV as Key Vault
    participant MSI as Managed Identity

    GH->>QK: Webhook + X-Hub-Signature-256
    QK->>MSI: Get token for Key Vault
    MSI-->>QK: Azure AD token
    QK->>KV: Get webhook secret (with token)
    KV-->>QK: Webhook secret
    QK->>QK: Validate HMAC-SHA256
    QK-->>GH: HTTP 200/401
```

### Security Boundaries

1. **Network Security**: API Gateway with DDoS protection
2. **Application Security**: Signature validation, input sanitization
3. **Data Security**: Encryption at rest and in transit
4. **Identity Security**: Managed Identity for service-to-service auth
5. **Secret Security**: Key Vault with access policies and rotation

## Deployment Architecture

### Infrastructure Components

```yaml
# High-level Terraform resources (managed externally)
resource_groups:
  - queue-keeper-prod
  - queue-keeper-staging

azure_functions:
  - name: queue-keeper-prod
    runtime: custom (Rust)
    plan: Premium (EP1)

service_bus:
  - namespace: offaxis-automation-prod
    queues: [task-tactician, merge-warden, spec-sentinel, dead-letter]

storage_accounts:
  - name: queuekeeperblobs
    containers: [webhooks, dead-letters]

key_vault:
  - name: queue-keeper-secrets
    secrets: [github-webhook-secret]
```

### Deployment Pipeline

1. **Build**: Rust compilation with cargo
2. **Test**: Unit and integration tests
3. **Package**: Create container image for Azure Container Apps
4. **Deploy**: Blue-green deployment with health checks
5. **Verify**: Smoke tests and monitoring validation

## Monitoring Architecture

### Observability Stack

```mermaid
graph TB
    subgraph "Data Sources"
        QK[Queue-Keeper Function]
        SB[Service Bus]
        BS[Blob Storage]
        KV[Key Vault]
    end

    subgraph "Collection"
        AI[Application Insights]
        AM[Azure Monitor]
    end

    subgraph "Analysis"
        WB[Workbooks]
        AL[Alerts]
        DB[Dashboards]
    end

    QK --> AI
    SB --> AM
    BS --> AM
    KV --> AM

    AI --> WB
    AM --> AL
    AI --> DB
```

### Key Metrics

- **Business Metrics**: Events processed, routing success rate, processing latency
- **Technical Metrics**: Function invocations, memory usage, error rates
- **Infrastructure Metrics**: Service Bus queue depth, blob storage usage
- **Security Metrics**: Signature validation failures, unauthorized requests

## Disaster Recovery Architecture

### Backup Strategy

- **Configuration**: Source control (Git)
- **Secrets**: Key Vault with soft delete enabled
- **Data**: Blob storage with geo-redundancy
- **Queues**: Service Bus geo-disaster recovery pairing

### Recovery Procedures

- **RTO**: 5 minutes for automated failover
- **RPO**: 0 (no data loss acceptable)
- **Runbooks**: Automated recovery scripts and manual procedures
- **Testing**: Monthly disaster recovery drills
