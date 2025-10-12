# Audit Logging Interface

**Architectural Layer**: Cross-Cutting Concern (Infrastructure Interface)
**Module Path**: `src/audit_logging.rs`
**Responsibilities** (from RDD):

- Knows: Event structures, audit requirements, retention policies, compliance formats
- Does: Records immutable audit trail, manages log lifecycle, provides compliance reporting

## Dependencies

- Types: `EventId`, `SessionId`, `Timestamp`, `Repository`, `AuditLogId` (audit-logging-types.md)
- Interfaces: `BlobStorage` (blob-storage.md), `KeyVault` (key-vault.md)
- Shared: `Result<T, E>`, `QueueKeeperError` (shared-types.md)

## Overview

The Audit Logging Interface defines how Queue-Keeper creates and maintains an immutable audit trail for compliance, security, and operational purposes. This system implements REQ-015 (Audit Logging) by providing comprehensive logging of all webhook processing activities, administrative actions, and system events with proper retention and compliance support.

**Critical Design Principles:**

- **Immutability**: Audit logs cannot be modified once written
- **Completeness**: All significant events are recorded with full context
- **Compliance**: Structured logging meets regulatory requirements (SOX, GDPR, etc.)
- **Performance**: Non-blocking logging doesn't impact webhook processing
- **Retention**: Configurable retention with automated cleanup
- **Security**: Audit logs are tamper-evident and access-controlled

## Types

### AuditEvent

Core audit event structure for all logged activities.

```rust
/// Immutable audit event record
///
/// Represents a single auditable action with complete context
/// for compliance and operational analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique identifier for this audit entry
    pub audit_id: AuditLogId,

    /// When the auditable event occurred
    pub occurred_at: Timestamp,

    /// When the audit entry was created
    pub logged_at: Timestamp,

    /// Type of event being audited
    pub event_type: AuditEventType,

    /// Actor who initiated the action (user, system, bot)
    pub actor: AuditActor,

    /// Resource affected by the action
    pub resource: AuditResource,

    /// Action that was performed
    pub action: AuditAction,

    /// Result of the action
    pub result: AuditResult,

    /// Additional structured context
    pub context: AuditContext,

    /// Hash of the event data for tamper detection
    pub content_hash: String,

    /// Previous audit entry hash (for chain verification)
    pub previous_hash: Option<String>,
}

impl AuditEvent {
    /// Create new audit event
    pub fn new(
        event_type: AuditEventType,
        actor: AuditActor,
        resource: AuditResource,
        action: AuditAction,
        result: AuditResult,
        context: AuditContext,
    ) -> Self;

    /// Verify content hash integrity
    pub fn verify_integrity(&self) -> bool;

    /// Get compliance category for retention rules
    pub fn get_compliance_category(&self) -> ComplianceCategory;

    /// Check if event should be encrypted at rest
    pub fn requires_encryption(&self) -> bool;

    /// Get retention period for this event
    pub fn get_retention_period(&self) -> Duration;
}
```

### AuditEventType

Categories of events that generate audit entries.

```rust
/// Type of event being audited
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Webhook processing events
    WebhookProcessing,

    /// Administrative actions
    Administration,

    /// Security-related events
    Security,

    /// Configuration changes
    Configuration,

    /// Data access events
    DataAccess,

    /// System events
    System,

    /// Compliance-specific events
    Compliance,
}

impl AuditEventType {
    /// Get compliance importance level
    pub fn get_compliance_level(&self) -> ComplianceLevel;

    /// Get required retention period
    pub fn get_retention_period(&self) -> Duration;

    /// Check if encryption is required
    pub fn requires_encryption(&self) -> bool;
}
```

### AuditActor

Represents who or what initiated the audited action.

```rust
/// Actor who performed the audited action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditActor {
    /// Human user (admin, developer)
    User {
        user_id: String,
        username: String,
        email: Option<String>,
        role: Option<String>,
    },

    /// System component
    System {
        component_name: String,
        instance_id: String,
        version: String,
    },

    /// External service
    ExternalService {
        service_name: String,
        service_id: String,
        authenticated: bool,
    },

    /// Automated process
    Automation {
        process_name: String,
        trigger: String,
        scheduled: bool,
    },

    /// Anonymous/unknown actor
    Anonymous {
        source_ip: Option<String>,
        user_agent: Option<String>,
    },
}

impl AuditActor {
    /// Get actor identifier for indexing
    pub fn get_actor_id(&self) -> String;

    /// Get human-readable description
    pub fn get_description(&self) -> String;

    /// Check if actor is privileged
    pub fn is_privileged(&self) -> bool;
}
```

### AuditResource

Represents what was acted upon.

```rust
/// Resource that was acted upon
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditResource {
    /// GitHub webhook event
    WebhookEvent {
        event_id: EventId,
        session_id: SessionId,
        repository: Repository,
        event_type: String,
    },

    /// Bot configuration
    BotConfiguration {
        bot_name: String,
        configuration_version: Option<String>,
    },

    /// Queue or session
    Queue {
        queue_name: String,
        session_id: Option<SessionId>,
    },

    /// Secret or credential
    Secret {
        secret_name: String,
        key_vault: String,
    },

    /// System configuration
    SystemConfiguration {
        component: String,
        setting_name: String,
    },

    /// Stored data
    Data {
        data_type: String,
        identifier: String,
        location: Option<String>,
    },

    /// Administrative resource
    Administrative {
        resource_type: String,
        resource_id: String,
    },
}

impl AuditResource {
    /// Get resource type for categorization
    pub fn get_resource_type(&self) -> String;

    /// Get resource identifier
    pub fn get_resource_id(&self) -> String;

    /// Check if resource contains sensitive data
    pub fn is_sensitive(&self) -> bool;
}
```

### AuditAction

Specific action that was performed.

```rust
/// Action that was performed on the resource
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    /// Data operations
    Create { details: Option<String> },
    Read { query: Option<String> },
    Update { changes: Option<String> },
    Delete { reason: Option<String> },

    /// Processing operations
    Process { operation: String },
    Route { destination: String },
    Validate { validation_type: String },
    Transform { transformation: String },

    /// Administrative operations
    Configure { setting: String, value: Option<String> },
    Deploy { version: String },
    Restart { reason: String },
    Monitor { metric: String },

    /// Security operations
    Authenticate { method: String },
    Authorize { permission: String },
    Encrypt { algorithm: String },
    Decrypt { purpose: String },

    /// Custom operation
    Custom { operation: String, details: Option<String> },
}

impl AuditAction {
    /// Get action category for filtering
    pub fn get_category(&self) -> ActionCategory;

    /// Check if action is high-risk
    pub fn is_high_risk(&self) -> bool;

    /// Get required approval level
    pub fn get_approval_level(&self) -> ApprovalLevel;
}
```

### AuditResult

Outcome of the audited action.

```rust
/// Result of the audited action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditResult {
    /// Action completed successfully
    Success {
        duration: Option<Duration>,
        details: Option<String>,
    },

    /// Action failed
    Failure {
        error_code: String,
        error_message: String,
        retryable: bool,
    },

    /// Action partially completed
    Partial {
        success_count: usize,
        failure_count: usize,
        details: String,
    },

    /// Action was skipped
    Skipped {
        reason: String,
    },

    /// Action is pending/in progress
    Pending {
        estimated_completion: Option<Timestamp>,
    },

    /// Action was cancelled
    Cancelled {
        reason: String,
        cancelled_by: String,
    },
}

impl AuditResult {
    /// Check if result indicates success
    pub fn is_successful(&self) -> bool;

    /// Check if result indicates an error
    pub fn is_error(&self) -> bool;

    /// Get error code if applicable
    pub fn get_error_code(&self) -> Option<&str>;
}
```

### AuditContext

Additional context and metadata for audit events.

```rust
/// Additional context for audit events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditContext {
    /// Session or correlation identifier
    pub correlation_id: Option<String>,

    /// Request identifier
    pub request_id: Option<String>,

    /// Source IP address
    pub source_ip: Option<String>,

    /// User agent string
    pub user_agent: Option<String>,

    /// HTTP method and path
    pub http_context: Option<HttpContext>,

    /// Geographic information
    pub geo_context: Option<GeoContext>,

    /// Performance metrics
    pub performance: Option<PerformanceContext>,

    /// Security context
    pub security: Option<SecurityContext>,

    /// Business context
    pub business: Option<BusinessContext>,

    /// Additional structured data
    pub additional_data: HashMap<String, String>,
}

impl Default for AuditContext {
    fn default() -> Self {
        Self {
            correlation_id: None,
            request_id: None,
            source_ip: None,
            user_agent: None,
            http_context: None,
            geo_context: None,
            performance: None,
            security: None,
            business: None,
            additional_data: HashMap::new(),
        }
    }
}

/// HTTP request context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpContext {
    pub method: String,
    pub path: String,
    pub query_params: Option<String>,
    pub headers: HashMap<String, String>,
    pub response_status: Option<u16>,
}

/// Geographic context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoContext {
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
}

/// Performance metrics context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceContext {
    pub duration_ms: u64,
    pub memory_usage_bytes: Option<u64>,
    pub cpu_usage_percent: Option<f64>,
    pub network_bytes_sent: Option<u64>,
    pub network_bytes_received: Option<u64>,
}

/// Security-related context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityContext {
    pub authentication_method: Option<String>,
    pub authorization_granted: bool,
    pub security_level: SecurityLevel,
    pub threat_indicators: Vec<String>,
    pub compliance_tags: Vec<String>,
}

/// Business process context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusinessContext {
    pub process_name: String,
    pub process_version: Option<String>,
    pub business_unit: Option<String>,
    pub cost_center: Option<String>,
    pub project_id: Option<String>,
}
```

### AuditLogId

Unique identifier for audit log entries.

```rust
/// Unique identifier for audit log entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuditLogId(Ulid);

impl AuditLogId {
    /// Generate new audit log ID
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self, AuditError> {
        let ulid = s.parse().map_err(|_| AuditError::InvalidAuditId {
            audit_id: s.to_string()
        })?;
        Ok(Self(ulid))
    }

    /// Get string representation
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }

    /// Get underlying ULID
    pub fn as_ulid(&self) -> Ulid {
        self.0
    }
}

impl fmt::Display for AuditLogId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

## Core Interfaces

### AuditLogger

Main interface for audit logging operations.

```rust
/// Interface for audit logging operations
///
/// Provides non-blocking, tamper-evident audit logging with
/// compliance support and automated retention management.
#[async_trait::async_trait]
pub trait AuditLogger: Send + Sync {
    /// Log audit event asynchronously
    ///
    /// Events are queued for immediate writing to ensure
    /// webhook processing is not blocked.
    async fn log_event(&self, event: AuditEvent) -> Result<AuditLogId, AuditError>;

    /// Log webhook processing event
    async fn log_webhook_processing(
        &self,
        event_id: EventId,
        session_id: SessionId,
        repository: Repository,
        action: WebhookProcessingAction,
        result: AuditResult,
        context: AuditContext,
    ) -> Result<AuditLogId, AuditError>;

    /// Log administrative action
    async fn log_admin_action(
        &self,
        actor: AuditActor,
        resource: AuditResource,
        action: AuditAction,
        result: AuditResult,
        context: AuditContext,
    ) -> Result<AuditLogId, AuditError>;

    /// Log security event
    async fn log_security_event(
        &self,
        security_event: SecurityAuditEvent,
        context: AuditContext,
    ) -> Result<AuditLogId, AuditError>;

    /// Batch log multiple events
    async fn log_events_batch(
        &self,
        events: Vec<AuditEvent>,
    ) -> Result<Vec<AuditLogId>, AuditError>;

    /// Flush pending log entries
    async fn flush(&self) -> Result<(), AuditError>;
}
```

### AuditQuery

Interface for querying audit logs.

```rust
/// Interface for querying audit logs
///
/// Provides controlled access to audit log data for compliance
/// reporting, security analysis, and operational monitoring.
#[async_trait::async_trait]
pub trait AuditQuery: Send + Sync {
    /// Query audit events by criteria
    async fn query_events(
        &self,
        query: AuditQuery,
        pagination: PaginationOptions,
    ) -> Result<AuditQueryResult, AuditError>;

    /// Get specific audit event by ID
    async fn get_event(&self, audit_id: AuditLogId) -> Result<Option<AuditEvent>, AuditError>;

    /// Get audit trail for specific resource
    async fn get_resource_trail(
        &self,
        resource: AuditResource,
        time_range: TimeRange,
    ) -> Result<Vec<AuditEvent>, AuditError>;

    /// Get audit events for session
    async fn get_session_trail(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<AuditEvent>, AuditError>;

    /// Generate compliance report
    async fn generate_compliance_report(
        &self,
        report_spec: ComplianceReportSpec,
    ) -> Result<ComplianceReport, AuditError>;

    /// Verify audit chain integrity
    async fn verify_chain_integrity(
        &self,
        start_time: Timestamp,
        end_time: Timestamp,
    ) -> Result<IntegrityVerificationResult, AuditError>;

    /// Get audit statistics
    async fn get_statistics(
        &self,
        time_range: TimeRange,
        group_by: Option<StatisticsGroupBy>,
    ) -> Result<AuditStatistics, AuditError>;
}
```

### AuditRetention

Interface for audit log retention management.

```rust
/// Interface for audit log retention and lifecycle management
///
/// Manages audit log retention according to compliance requirements
/// with automated cleanup and archival processes.
#[async_trait::async_trait]
pub trait AuditRetention: Send + Sync {
    /// Archive old audit logs
    async fn archive_logs(
        &self,
        before_date: Timestamp,
        archive_location: String,
    ) -> Result<ArchiveResult, AuditError>;

    /// Delete expired audit logs
    async fn delete_expired_logs(
        &self,
        retention_policy: RetentionPolicy,
    ) -> Result<DeletionResult, AuditError>;

    /// Get retention status
    async fn get_retention_status(&self) -> Result<RetentionStatus, AuditError>;

    /// Compress old audit logs
    async fn compress_logs(
        &self,
        before_date: Timestamp,
        compression_level: CompressionLevel,
    ) -> Result<CompressionResult, AuditError>;

    /// Restore archived logs
    async fn restore_archived_logs(
        &self,
        archive_location: String,
        time_range: TimeRange,
    ) -> Result<RestoreResult, AuditError>;

    /// Validate retention compliance
    async fn validate_compliance(
        &self,
        compliance_rules: Vec<ComplianceRule>,
    ) -> Result<ComplianceValidationResult, AuditError>;
}
```

## Supporting Types

### Webhook Processing Actions

Specific actions for webhook processing audit events.

```rust
/// Specific webhook processing actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookProcessingAction {
    /// Webhook received
    Received {
        event_type: String,
        payload_size: usize,
    },

    /// Signature validation
    SignatureValidation {
        method: String,
        valid: bool,
    },

    /// Event normalization
    Normalization {
        original_type: String,
        normalized_type: String,
    },

    /// Bot routing
    BotRouting {
        matched_bots: Vec<String>,
        routing_duration_ms: u64,
    },

    /// Queue delivery
    QueueDelivery {
        queue_name: String,
        delivery_method: String,
    },

    /// Blob storage
    BlobStorage {
        storage_location: String,
        storage_size: u64,
    },

    /// Processing completion
    ProcessingComplete {
        total_duration_ms: u64,
        success_count: usize,
        failure_count: usize,
    },
}
```

### Security Audit Events

Security-specific audit event types.

```rust
/// Security-specific audit events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityAuditEvent {
    /// Authentication attempt
    AuthenticationAttempt {
        method: String,
        success: bool,
        failure_reason: Option<String>,
    },

    /// Authorization check
    AuthorizationCheck {
        resource: String,
        permission: String,
        granted: bool,
    },

    /// Suspicious activity detected
    SuspiciousActivity {
        activity_type: String,
        risk_level: RiskLevel,
        indicators: Vec<String>,
    },

    /// Security configuration change
    SecurityConfigurationChange {
        setting: String,
        old_value: Option<String>,
        new_value: String,
    },

    /// Access attempt to sensitive resource
    SensitiveResourceAccess {
        resource: String,
        access_type: String,
        authorized: bool,
    },

    /// Encryption/decryption operation
    CryptographicOperation {
        operation: String,
        algorithm: String,
        key_id: String,
    },
}
```

### Compliance and Reporting Types

Types for compliance reporting and management.

```rust
/// Compliance category for retention rules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceCategory {
    /// Financial compliance (SOX, etc.)
    Financial,

    /// Data privacy compliance (GDPR, CCPA)
    Privacy,

    /// Security compliance (ISO 27001, SOC 2)
    Security,

    /// Industry-specific compliance
    Industry { standard: String },

    /// Legal hold requirements
    LegalHold,

    /// Operational audit requirements
    Operational,
}

/// Compliance importance level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceLevel {
    /// Critical for compliance
    Critical,

    /// Important for compliance
    Important,

    /// Standard operational logging
    Standard,

    /// Debug/diagnostic information
    Debug,
}

/// Security level classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// Public information
    Public,

    /// Internal use only
    Internal,

    /// Confidential information
    Confidential,

    /// Restricted/classified information
    Restricted,
}

/// Risk level assessment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Action category for filtering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionCategory {
    DataOperation,
    ProcessingOperation,
    AdministrativeOperation,
    SecurityOperation,
    SystemOperation,
    CustomOperation,
}

/// Approval level requirement
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalLevel {
    None,
    Supervisor,
    Manager,
    Executive,
    Board,
}

/// Query criteria for audit events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQuery {
    /// Time range filter
    pub time_range: Option<TimeRange>,

    /// Event type filter
    pub event_types: Option<Vec<AuditEventType>>,

    /// Actor filter
    pub actors: Option<Vec<String>>,

    /// Resource filter
    pub resources: Option<Vec<String>>,

    /// Action filter
    pub actions: Option<Vec<String>>,

    /// Result filter
    pub results: Option<Vec<String>>,

    /// Full-text search
    pub search_text: Option<String>,

    /// Custom filters
    pub custom_filters: HashMap<String, String>,
}

/// Time range specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: Timestamp,
    pub end: Timestamp,
}

/// Pagination options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationOptions {
    pub page: usize,
    pub per_page: usize,
    pub sort_by: Option<String>,
    pub sort_order: SortOrder,
}

/// Sort order
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQueryResult {
    pub events: Vec<AuditEvent>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

/// Compliance report specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReportSpec {
    pub report_type: ComplianceReportType,
    pub time_range: TimeRange,
    pub scope: ComplianceScope,
    pub format: ReportFormat,
    pub include_details: bool,
}

/// Compliance report type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComplianceReportType {
    AccessReport,
    ChangeReport,
    SecurityReport,
    DataProcessingReport,
    RetentionReport,
    CustomReport { template: String },
}

/// Compliance scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceScope {
    pub categories: Vec<ComplianceCategory>,
    pub resources: Option<Vec<String>>,
    pub actors: Option<Vec<String>>,
}

/// Report format
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportFormat {
    Json,
    Csv,
    Pdf,
    Html,
    Excel,
}

/// Generated compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub report_id: String,
    pub generated_at: Timestamp,
    pub spec: ComplianceReportSpec,
    pub summary: ReportSummary,
    pub content: ReportContent,
}

/// Report summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_events: usize,
    pub event_breakdown: HashMap<String, usize>,
    pub compliance_issues: Vec<ComplianceIssue>,
    pub recommendations: Vec<String>,
}

/// Report content (format-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReportContent {
    Json(serde_json::Value),
    Csv(String),
    Binary(Vec<u8>), // For PDF, Excel, etc.
}

/// Compliance issue identified in report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceIssue {
    pub issue_type: String,
    pub severity: ComplianceLevel,
    pub description: String,
    pub affected_events: Vec<AuditLogId>,
    pub remediation: Option<String>,
}
```

### Retention and Lifecycle Types

Types for audit log lifecycle management.

```rust
/// Retention policy specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub rules: Vec<RetentionRule>,
    pub default_retention: Duration,
    pub legal_hold_enabled: bool,
}

/// Individual retention rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionRule {
    pub category: ComplianceCategory,
    pub retention_period: Duration,
    pub archive_after: Option<Duration>,
    pub compress_after: Option<Duration>,
    pub encryption_required: bool,
}

/// Archive operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub archived_count: usize,
    pub archive_location: String,
    pub archive_size_bytes: u64,
    pub archive_duration: Duration,
    pub errors: Vec<String>,
}

/// Deletion operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionResult {
    pub deleted_count: usize,
    pub freed_space_bytes: u64,
    pub deletion_duration: Duration,
    pub errors: Vec<String>,
}

/// Retention status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionStatus {
    pub total_logs: usize,
    pub logs_by_age: HashMap<String, usize>,
    pub archived_logs: usize,
    pub compressed_logs: usize,
    pub pending_deletion: usize,
    pub storage_usage_bytes: u64,
    pub compliance_status: ComplianceStatus,
}

/// Compliance status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    pub compliant: bool,
    pub issues: Vec<ComplianceIssue>,
    pub last_validated: Timestamp,
    pub next_validation: Timestamp,
}

/// Compression level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionLevel {
    None,
    Fast,
    Balanced,
    Maximum,
}

/// Compression result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    pub compressed_count: usize,
    pub original_size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub compression_ratio: f64,
    pub compression_duration: Duration,
}

/// Restore operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub restored_count: usize,
    pub restore_location: String,
    pub restore_duration: Duration,
    pub errors: Vec<String>,
}

/// Compliance validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceValidationResult {
    pub compliant: bool,
    pub validation_time: Duration,
    pub rules_checked: usize,
    pub violations: Vec<ComplianceViolation>,
    pub recommendations: Vec<String>,
}

/// Compliance rule violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub rule_id: String,
    pub violation_type: String,
    pub severity: ComplianceLevel,
    pub description: String,
    pub affected_logs: Vec<AuditLogId>,
    pub required_action: String,
}

/// Integrity verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityVerificationResult {
    pub verified_count: usize,
    pub tampered_count: usize,
    pub missing_count: usize,
    pub chain_valid: bool,
    pub verification_duration: Duration,
    pub issues: Vec<IntegrityIssue>,
}

/// Integrity issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityIssue {
    pub audit_id: AuditLogId,
    pub issue_type: IntegrityIssueType,
    pub description: String,
    pub detected_at: Timestamp,
}

/// Type of integrity issue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrityIssueType {
    InvalidHash,
    BrokenChain,
    MissingEntry,
    TimestampAnomalyy,
    ContentModification,
}

/// Statistics grouping options
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatisticsGroupBy {
    EventType,
    Actor,
    Resource,
    Action,
    Result,
    Hour,
    Day,
    Week,
    Month,
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub time_range: TimeRange,
    pub total_events: usize,
    pub event_breakdown: HashMap<String, usize>,
    pub top_actors: Vec<(String, usize)>,
    pub top_resources: Vec<(String, usize)>,
    pub error_rate: f64,
    pub average_events_per_day: f64,
    pub compliance_metrics: ComplianceMetrics,
}

/// Compliance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceMetrics {
    pub total_compliance_events: usize,
    pub events_by_category: HashMap<ComplianceCategory, usize>,
    pub retention_compliance_rate: f64,
    pub encryption_compliance_rate: f64,
    pub access_control_compliance_rate: f64,
}

/// Compliance rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRule {
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub category: ComplianceCategory,
    pub requirements: Vec<ComplianceRequirement>,
    pub validation_query: String,
    pub severity: ComplianceLevel,
}

/// Individual compliance requirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRequirement {
    pub requirement_id: String,
    pub description: String,
    pub validation_logic: String,
    pub required_fields: Vec<String>,
    pub acceptable_values: Option<Vec<String>>,
}
```

## Error Types

### AuditError

Comprehensive error type for audit logging operations.

```rust
/// Errors that can occur during audit logging operations
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("Invalid audit ID: {audit_id}")]
    InvalidAuditId { audit_id: String },

    #[error("Audit log not found: {audit_id}")]
    AuditLogNotFound { audit_id: AuditLogId },

    #[error("Storage error: {message}")]
    StorageError { message: String },

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("Encryption error: {message}")]
    EncryptionError { message: String },

    #[error("Integrity verification failed: {message}")]
    IntegrityError { message: String },

    #[error("Query error: {message}")]
    QueryError { message: String },

    #[error("Retention policy violation: {message}")]
    RetentionViolation { message: String },

    #[error("Compliance violation: {rule} - {message}")]
    ComplianceViolation { rule: String, message: String },

    #[error("Archive operation failed: {message}")]
    ArchiveError { message: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },

    #[error("Capacity exceeded: {current}/{max}")]
    CapacityExceeded { current: usize, max: usize },

    #[error("Timeout during {operation} after {duration:?}")]
    Timeout { operation: String, duration: Duration },

    #[error("Service unavailable: {service} - {message}")]
    ServiceUnavailable { service: String, message: String },
}

impl AuditError {
    /// Check if error is transient
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            AuditError::StorageError { .. } |
            AuditError::ServiceUnavailable { .. } |
            AuditError::Timeout { .. } |
            AuditError::CapacityExceeded { .. }
        )
    }

    /// Check if error is a compliance issue
    pub fn is_compliance_error(&self) -> bool {
        matches!(
            self,
            AuditError::RetentionViolation { .. } |
            AuditError::ComplianceViolation { .. } |
            AuditError::IntegrityError { .. }
        )
    }

    /// Get retry delay for transient errors
    pub fn get_retry_delay(&self) -> Option<Duration> {
        match self {
            AuditError::StorageError { .. } => Some(Duration::from_secs(5)),
            AuditError::ServiceUnavailable { .. } => Some(Duration::from_secs(30)),
            AuditError::Timeout { .. } => Some(Duration::from_secs(10)),
            AuditError::CapacityExceeded { .. } => Some(Duration::from_secs(60)),
            _ => None,
        }
    }
}
```

## Usage Examples

### Basic Audit Logging

```rust
// Log webhook processing event
let audit_context = AuditContext {
    correlation_id: Some(session_id.to_string()),
    request_id: Some(event_id.to_string()),
    source_ip: Some("192.168.1.100".to_string()),
    ..Default::default()
};

let audit_id = audit_logger.log_webhook_processing(
    event_id,
    session_id,
    repository,
    WebhookProcessingAction::Received {
        event_type: "pull_request.opened".to_string(),
        payload_size: 4096,
    },
    AuditResult::Success {
        duration: Some(Duration::from_millis(250)),
        details: Some("Successfully processed webhook".to_string()),
    },
    audit_context,
).await?;
```

### Administrative Action Logging

```rust
// Log configuration change
let admin_actor = AuditActor::User {
    user_id: "admin123".to_string(),
    username: "admin".to_string(),
    email: Some("admin@company.com".to_string()),
    role: Some("Administrator".to_string()),
};

let config_resource = AuditResource::BotConfiguration {
    bot_name: "security-bot".to_string(),
    configuration_version: Some("v2.1.0".to_string()),
};

let config_action = AuditAction::Configure {
    setting: "webhook_timeout".to_string(),
    value: Some("30s".to_string()),
};

audit_logger.log_admin_action(
    admin_actor,
    config_resource,
    config_action,
    AuditResult::Success {
        duration: Some(Duration::from_millis(100)),
        details: None,
    },
    audit_context,
).await?;
```

### Security Event Logging

```rust
// Log suspicious activity
let security_event = SecurityAuditEvent::SuspiciousActivity {
    activity_type: "Repeated failed webhook signatures".to_string(),
    risk_level: RiskLevel::Medium,
    indicators: vec![
        "Multiple signature failures from same IP".to_string(),
        "Unusual request patterns".to_string(),
    ],
};

audit_logger.log_security_event(security_event, audit_context).await?;
```

### Compliance Reporting

```rust
// Generate access report for SOX compliance
let report_spec = ComplianceReportSpec {
    report_type: ComplianceReportType::AccessReport,
    time_range: TimeRange {
        start: Timestamp::now().subtract_duration(Duration::from_secs(30 * 24 * 3600)), // 30 days
        end: Timestamp::now(),
    },
    scope: ComplianceScope {
        categories: vec![ComplianceCategory::Financial],
        resources: None,
        actors: None,
    },
    format: ReportFormat::Pdf,
    include_details: true,
};

let report = audit_query.generate_compliance_report(report_spec).await?;
```

## Implementation Considerations

### Performance

1. **Asynchronous Logging**: All audit operations are non-blocking to avoid impacting webhook processing
2. **Batch Processing**: Support batching of audit events for improved throughput
3. **Efficient Storage**: Use appropriate storage backends optimized for append-only workloads
4. **Indexed Queries**: Proper indexing strategy for common query patterns

### Security

1. **Tamper Evidence**: Hash chaining and content verification prevent tampering
2. **Encryption**: Sensitive audit data encrypted at rest and in transit
3. **Access Control**: Role-based access to audit logs and compliance reports
4. **Audit of Audits**: Meta-audit logging of access to audit logs themselves

### Compliance

1. **Retention Management**: Automated retention and deletion according to compliance requirements
2. **Data Privacy**: GDPR-compliant handling of personal data in audit logs
3. **Immutability**: Audit logs cannot be modified once written
4. **Chain of Custody**: Complete tracking of who accessed what and when

### Scalability

1. **Partitioning**: Time-based partitioning for efficient storage and queries
2. **Archival**: Automated archival of old logs to cold storage
3. **Compression**: Automated compression of archived logs
4. **Distributed Storage**: Support for distributed storage backends

This Audit Logging interface provides comprehensive compliance and security logging capabilities for REQ-015 while ensuring operational efficiency and regulatory compliance.
