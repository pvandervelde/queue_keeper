// GENERATED FROM: specs/interfaces/audit-logging.md
// Audit Logging Interface - Comprehensive audit trail for compliance and security
//
// This module provides interfaces for creating and maintaining immutable
// audit logs with compliance support, retention management, and security
// features for regulatory requirements.

use crate::{EventId, Repository, SessionId, Timestamp};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, str::FromStr, time::Duration};
use thiserror::Error;
use ulid::Ulid;

// ============================================================================
// Core Types
// ============================================================================

/// Immutable audit event record
///
/// Represents a single auditable action with complete context
/// for compliance and operational analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    ) -> Self {
        let audit_id = AuditLogId::new();
        let occurred_at = Timestamp::now();
        let logged_at = Timestamp::now();

        let content_hash = Self::calculate_content_hash(
            &audit_id,
            &occurred_at,
            &event_type,
            &actor,
            &resource,
            &action,
            &result,
            &context,
        );

        Self {
            audit_id,
            occurred_at,
            logged_at,
            event_type,
            actor,
            resource,
            action,
            result,
            context,
            content_hash,
            previous_hash: None,
        }
    }

    /// Verify content hash integrity
    pub fn verify_integrity(&self) -> bool {
        let calculated_hash = Self::calculate_content_hash(
            &self.audit_id,
            &self.occurred_at,
            &self.event_type,
            &self.actor,
            &self.resource,
            &self.action,
            &self.result,
            &self.context,
        );
        calculated_hash == self.content_hash
    }

    /// Get compliance category for retention rules
    pub fn get_compliance_category(&self) -> ComplianceCategory {
        match self.event_type {
            AuditEventType::Security => ComplianceCategory::Security,
            AuditEventType::Administration => ComplianceCategory::Operational,
            AuditEventType::Configuration => ComplianceCategory::Operational,
            AuditEventType::DataAccess => ComplianceCategory::Privacy,
            AuditEventType::Compliance => ComplianceCategory::Financial,
            _ => ComplianceCategory::Operational,
        }
    }

    /// Check if event should be encrypted at rest
    pub fn requires_encryption(&self) -> bool {
        matches!(
            self.event_type,
            AuditEventType::Security | AuditEventType::DataAccess | AuditEventType::Compliance
        ) || self.resource.is_sensitive()
    }

    /// Get retention period for this event
    pub fn get_retention_period(&self) -> Duration {
        match self.get_compliance_category() {
            ComplianceCategory::Financial => Duration::from_secs(7 * 365 * 24 * 3600), // 7 years
            ComplianceCategory::Security => Duration::from_secs(3 * 365 * 24 * 3600),  // 3 years
            ComplianceCategory::Privacy => Duration::from_secs(2 * 365 * 24 * 3600),   // 2 years
            ComplianceCategory::LegalHold => Duration::from_secs(10 * 365 * 24 * 3600), // 10 years
            _ => Duration::from_secs(365 * 24 * 3600),                                 // 1 year
        }
    }

    /// Calculate content hash for integrity verification
    fn calculate_content_hash(
        audit_id: &AuditLogId,
        occurred_at: &Timestamp,
        event_type: &AuditEventType,
        actor: &AuditActor,
        resource: &AuditResource,
        action: &AuditAction,
        result: &AuditResult,
        context: &AuditContext,
    ) -> String {
        // In a real implementation, this would use a cryptographic hash
        // For now, we'll create a simple hash representation
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            audit_id,
            occurred_at,
            serde_json::to_string(event_type).unwrap_or_default(),
            serde_json::to_string(actor).unwrap_or_default(),
            serde_json::to_string(resource).unwrap_or_default(),
            serde_json::to_string(action).unwrap_or_default(),
            serde_json::to_string(result).unwrap_or_default(),
            serde_json::to_string(context).unwrap_or_default(),
        )
    }
}

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
    pub fn get_compliance_level(&self) -> ComplianceLevel {
        match self {
            AuditEventType::Security => ComplianceLevel::Critical,
            AuditEventType::Compliance => ComplianceLevel::Critical,
            AuditEventType::Administration => ComplianceLevel::Important,
            AuditEventType::Configuration => ComplianceLevel::Important,
            AuditEventType::DataAccess => ComplianceLevel::Important,
            AuditEventType::WebhookProcessing => ComplianceLevel::Standard,
            AuditEventType::System => ComplianceLevel::Standard,
        }
    }

    /// Get required retention period
    pub fn get_retention_period(&self) -> Duration {
        match self {
            AuditEventType::Security => Duration::from_secs(3 * 365 * 24 * 3600), // 3 years
            AuditEventType::Compliance => Duration::from_secs(7 * 365 * 24 * 3600), // 7 years
            AuditEventType::Administration => Duration::from_secs(2 * 365 * 24 * 3600), // 2 years
            AuditEventType::Configuration => Duration::from_secs(2 * 365 * 24 * 3600), // 2 years
            AuditEventType::DataAccess => Duration::from_secs(2 * 365 * 24 * 3600), // 2 years
            _ => Duration::from_secs(365 * 24 * 3600),                            // 1 year
        }
    }

    /// Check if encryption is required
    pub fn requires_encryption(&self) -> bool {
        matches!(
            self,
            AuditEventType::Security | AuditEventType::DataAccess | AuditEventType::Compliance
        )
    }
}

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
    pub fn get_actor_id(&self) -> String {
        match self {
            AuditActor::User { user_id, .. } => user_id.clone(),
            AuditActor::System {
                component_name,
                instance_id,
                ..
            } => {
                format!("{}:{}", component_name, instance_id)
            }
            AuditActor::ExternalService {
                service_name,
                service_id,
                ..
            } => {
                format!("{}:{}", service_name, service_id)
            }
            AuditActor::Automation { process_name, .. } => process_name.clone(),
            AuditActor::Anonymous { source_ip, .. } => {
                source_ip.clone().unwrap_or_else(|| "unknown".to_string())
            }
        }
    }

    /// Get human-readable description
    pub fn get_description(&self) -> String {
        match self {
            AuditActor::User { username, role, .. } => match role {
                Some(role) => format!("{} ({})", username, role),
                None => username.clone(),
            },
            AuditActor::System {
                component_name,
                version,
                ..
            } => {
                format!("{} v{}", component_name, version)
            }
            AuditActor::ExternalService {
                service_name,
                authenticated,
                ..
            } => {
                if *authenticated {
                    format!("{} (authenticated)", service_name)
                } else {
                    format!("{} (unauthenticated)", service_name)
                }
            }
            AuditActor::Automation {
                process_name,
                scheduled,
                ..
            } => {
                if *scheduled {
                    format!("{} (scheduled)", process_name)
                } else {
                    format!("{} (triggered)", process_name)
                }
            }
            AuditActor::Anonymous { source_ip, .. } => {
                format!(
                    "Anonymous ({})",
                    source_ip.as_ref().unwrap_or(&"unknown".to_string())
                )
            }
        }
    }

    /// Check if actor is privileged
    pub fn is_privileged(&self) -> bool {
        match self {
            AuditActor::User { role, .. } => role
                .as_ref()
                .is_some_and(|r| r.contains("admin") || r.contains("Admin")),
            AuditActor::System { .. } => true,
            AuditActor::ExternalService { authenticated, .. } => *authenticated,
            AuditActor::Automation { .. } => true,
            AuditActor::Anonymous { .. } => false,
        }
    }
}

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
    pub fn get_resource_type(&self) -> String {
        match self {
            AuditResource::WebhookEvent { .. } => "webhook_event".to_string(),
            AuditResource::BotConfiguration { .. } => "bot_configuration".to_string(),
            AuditResource::Queue { .. } => "queue".to_string(),
            AuditResource::Secret { .. } => "secret".to_string(),
            AuditResource::SystemConfiguration { .. } => "system_configuration".to_string(),
            AuditResource::Data { .. } => "data".to_string(),
            AuditResource::Administrative { .. } => "administrative".to_string(),
        }
    }

    /// Get resource identifier
    pub fn get_resource_id(&self) -> String {
        match self {
            AuditResource::WebhookEvent { event_id, .. } => event_id.to_string(),
            AuditResource::BotConfiguration { bot_name, .. } => bot_name.clone(),
            AuditResource::Queue { queue_name, .. } => queue_name.clone(),
            AuditResource::Secret { secret_name, .. } => secret_name.clone(),
            AuditResource::SystemConfiguration { setting_name, .. } => setting_name.clone(),
            AuditResource::Data { identifier, .. } => identifier.clone(),
            AuditResource::Administrative { resource_id, .. } => resource_id.clone(),
        }
    }

    /// Check if resource contains sensitive data
    pub fn is_sensitive(&self) -> bool {
        matches!(
            self,
            AuditResource::Secret { .. } | AuditResource::SystemConfiguration { .. }
        )
    }
}

/// Action that was performed on the resource
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    /// Data operations
    Create {
        details: Option<String>,
    },
    Read {
        query: Option<String>,
    },
    Update {
        changes: Option<String>,
    },
    Delete {
        reason: Option<String>,
    },

    /// Processing operations
    Process {
        operation: String,
    },
    Route {
        destination: String,
    },
    Validate {
        validation_type: String,
    },
    Transform {
        transformation: String,
    },

    /// Administrative operations
    Configure {
        setting: String,
        value: Option<String>,
    },
    Deploy {
        version: String,
    },
    Restart {
        reason: String,
    },
    Monitor {
        metric: String,
    },

    /// Security operations
    Authenticate {
        method: String,
    },
    Authorize {
        permission: String,
    },
    Encrypt {
        algorithm: String,
    },
    Decrypt {
        purpose: String,
    },

    /// Custom operation
    Custom {
        operation: String,
        details: Option<String>,
    },
}

impl AuditAction {
    /// Get action category for filtering
    pub fn get_category(&self) -> ActionCategory {
        match self {
            AuditAction::Create { .. }
            | AuditAction::Read { .. }
            | AuditAction::Update { .. }
            | AuditAction::Delete { .. } => ActionCategory::DataOperation,

            AuditAction::Process { .. }
            | AuditAction::Route { .. }
            | AuditAction::Validate { .. }
            | AuditAction::Transform { .. } => ActionCategory::ProcessingOperation,

            AuditAction::Configure { .. }
            | AuditAction::Deploy { .. }
            | AuditAction::Restart { .. }
            | AuditAction::Monitor { .. } => ActionCategory::AdministrativeOperation,

            AuditAction::Authenticate { .. }
            | AuditAction::Authorize { .. }
            | AuditAction::Encrypt { .. }
            | AuditAction::Decrypt { .. } => ActionCategory::SecurityOperation,

            AuditAction::Custom { .. } => ActionCategory::CustomOperation,
        }
    }

    /// Check if action is high-risk
    pub fn is_high_risk(&self) -> bool {
        matches!(
            self,
            AuditAction::Delete { .. }
                | AuditAction::Configure { .. }
                | AuditAction::Deploy { .. }
                | AuditAction::Restart { .. }
                | AuditAction::Decrypt { .. }
        )
    }

    /// Get required approval level
    pub fn get_approval_level(&self) -> ApprovalLevel {
        match self {
            AuditAction::Delete { .. } => ApprovalLevel::Manager,
            AuditAction::Configure { .. } => ApprovalLevel::Supervisor,
            AuditAction::Deploy { .. } => ApprovalLevel::Manager,
            AuditAction::Restart { .. } => ApprovalLevel::Supervisor,
            AuditAction::Decrypt { .. } => ApprovalLevel::Supervisor,
            _ => ApprovalLevel::None,
        }
    }
}

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
    Skipped { reason: String },

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
    pub fn is_successful(&self) -> bool {
        matches!(self, AuditResult::Success { .. })
    }

    /// Check if result indicates an error
    pub fn is_error(&self) -> bool {
        matches!(self, AuditResult::Failure { .. })
    }

    /// Get error code if applicable
    pub fn get_error_code(&self) -> Option<&str> {
        match self {
            AuditResult::Failure { error_code, .. } => Some(error_code),
            _ => None,
        }
    }
}

/// Additional context for audit events
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Unique identifier for audit log entries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuditLogId(Ulid);

impl AuditLogId {
    /// Generate new audit log ID
    pub fn new() -> Self {
        Self(Ulid::new())
    }
}

impl Default for AuditLogId {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLogId {
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

impl FromStr for AuditLogId {
    type Err = AuditError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ulid = s.parse().map_err(|_| AuditError::InvalidAuditId {
            audit_id: s.to_string(),
        })?;
        Ok(Self(ulid))
    }
}

// ============================================================================
// Core Interfaces
// ============================================================================

/// Interface for audit logging operations
///
/// Provides non-blocking, tamper-evident audit logging with
/// compliance support and automated retention management.
#[async_trait]
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

/// Interface for querying audit logs
///
/// Provides controlled access to audit log data for compliance
/// reporting, security analysis, and operational monitoring.
#[async_trait]
pub trait AuditQuery: Send + Sync {
    /// Query audit events by criteria
    async fn query_events(
        &self,
        query: AuditQuerySpec,
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
    async fn get_session_trail(&self, session_id: SessionId)
        -> Result<Vec<AuditEvent>, AuditError>;

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

/// Interface for audit log retention and lifecycle management
///
/// Manages audit log retention according to compliance requirements
/// with automated cleanup and archival processes.
#[async_trait]
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

// ============================================================================
// Supporting Types
// ============================================================================

/// Specific webhook processing actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookProcessingAction {
    /// Webhook received
    Received {
        event_type: String,
        payload_size: usize,
    },

    /// Signature validation
    SignatureValidation { method: String, valid: bool },

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

// ============================================================================
// Compliance and Supporting Types
// ============================================================================

/// Compliance category for retention rules
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
pub struct AuditQuerySpec {
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

// ============================================================================
// Retention and Lifecycle Types (Minimal Implementation)
// ============================================================================

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
    TimestampAnomaly,
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

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during audit logging operations
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Timeout {
        operation: String,
        duration: Duration,
    },

    #[error("Service unavailable: {service} - {message}")]
    ServiceUnavailable { service: String, message: String },
}

impl AuditError {
    /// Check if error is transient
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            AuditError::StorageError { .. }
                | AuditError::ServiceUnavailable { .. }
                | AuditError::Timeout { .. }
                | AuditError::CapacityExceeded { .. }
        )
    }

    /// Check if error is a compliance issue
    pub fn is_compliance_error(&self) -> bool {
        matches!(
            self,
            AuditError::RetentionViolation { .. }
                | AuditError::ComplianceViolation { .. }
                | AuditError::IntegrityError { .. }
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

// ============================================================================
// Default Implementations (Stubs)
// ============================================================================

/// Default implementation of AuditLogger
pub struct DefaultAuditLogger;

#[async_trait]
impl AuditLogger for DefaultAuditLogger {
    async fn log_event(&self, _event: AuditEvent) -> Result<AuditLogId, AuditError> {
        unimplemented!("Audit logger not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn log_webhook_processing(
        &self,
        _event_id: EventId,
        _session_id: SessionId,
        _repository: Repository,
        _action: WebhookProcessingAction,
        _result: AuditResult,
        _context: AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        unimplemented!("Audit logger not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn log_admin_action(
        &self,
        _actor: AuditActor,
        _resource: AuditResource,
        _action: AuditAction,
        _result: AuditResult,
        _context: AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        unimplemented!("Audit logger not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn log_security_event(
        &self,
        _security_event: SecurityAuditEvent,
        _context: AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        unimplemented!("Audit logger not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn log_events_batch(
        &self,
        _events: Vec<AuditEvent>,
    ) -> Result<Vec<AuditLogId>, AuditError> {
        unimplemented!("Audit logger not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn flush(&self) -> Result<(), AuditError> {
        unimplemented!("Audit logger not yet implemented - see specs/interfaces/audit-logging.md")
    }
}

/// Default implementation of AuditQuery
pub struct DefaultAuditQuery;

#[async_trait]
impl AuditQuery for DefaultAuditQuery {
    async fn query_events(
        &self,
        _query: AuditQuerySpec,
        _pagination: PaginationOptions,
    ) -> Result<AuditQueryResult, AuditError> {
        unimplemented!("Audit query not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn get_event(&self, _audit_id: AuditLogId) -> Result<Option<AuditEvent>, AuditError> {
        unimplemented!("Audit query not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn get_resource_trail(
        &self,
        _resource: AuditResource,
        _time_range: TimeRange,
    ) -> Result<Vec<AuditEvent>, AuditError> {
        unimplemented!("Audit query not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn get_session_trail(
        &self,
        _session_id: SessionId,
    ) -> Result<Vec<AuditEvent>, AuditError> {
        unimplemented!("Audit query not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn generate_compliance_report(
        &self,
        _report_spec: ComplianceReportSpec,
    ) -> Result<ComplianceReport, AuditError> {
        unimplemented!("Audit query not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn verify_chain_integrity(
        &self,
        _start_time: Timestamp,
        _end_time: Timestamp,
    ) -> Result<IntegrityVerificationResult, AuditError> {
        unimplemented!("Audit query not yet implemented - see specs/interfaces/audit-logging.md")
    }

    async fn get_statistics(
        &self,
        _time_range: TimeRange,
        _group_by: Option<StatisticsGroupBy>,
    ) -> Result<AuditStatistics, AuditError> {
        unimplemented!("Audit query not yet implemented - see specs/interfaces/audit-logging.md")
    }
}

/// Default implementation of AuditRetention
pub struct DefaultAuditRetention;

#[async_trait]
impl AuditRetention for DefaultAuditRetention {
    async fn archive_logs(
        &self,
        _before_date: Timestamp,
        _archive_location: String,
    ) -> Result<ArchiveResult, AuditError> {
        unimplemented!(
            "Audit retention not yet implemented - see specs/interfaces/audit-logging.md"
        )
    }

    async fn delete_expired_logs(
        &self,
        _retention_policy: RetentionPolicy,
    ) -> Result<DeletionResult, AuditError> {
        unimplemented!(
            "Audit retention not yet implemented - see specs/interfaces/audit-logging.md"
        )
    }

    async fn get_retention_status(&self) -> Result<RetentionStatus, AuditError> {
        unimplemented!(
            "Audit retention not yet implemented - see specs/interfaces/audit-logging.md"
        )
    }

    async fn compress_logs(
        &self,
        _before_date: Timestamp,
        _compression_level: CompressionLevel,
    ) -> Result<CompressionResult, AuditError> {
        unimplemented!(
            "Audit retention not yet implemented - see specs/interfaces/audit-logging.md"
        )
    }

    async fn restore_archived_logs(
        &self,
        _archive_location: String,
        _time_range: TimeRange,
    ) -> Result<RestoreResult, AuditError> {
        unimplemented!(
            "Audit retention not yet implemented - see specs/interfaces/audit-logging.md"
        )
    }

    async fn validate_compliance(
        &self,
        _compliance_rules: Vec<ComplianceRule>,
    ) -> Result<ComplianceValidationResult, AuditError> {
        unimplemented!(
            "Audit retention not yet implemented - see specs/interfaces/audit-logging.md"
        )
    }
}

#[cfg(test)]
#[path = "audit_logging_tests.rs"]
mod tests;
