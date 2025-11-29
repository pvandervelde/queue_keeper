//! # Queue-Keeper Core
//!
//! Core business logic for the Queue-Keeper webhook intake and routing service.
//!
//! This crate contains the domain logic for processing GitHub webhooks, validating
//! signatures, normalizing events, and routing them to appropriate bot queues.
//!
//! ## Architecture
//!
//! The core follows clean architecture principles:
//! - Business logic depends only on trait abstractions
//! - Infrastructure implementations are injected at runtime
//! - All external dependencies are abstracted behind traits
//!
//! ## Usage
//!
//! ```rust
//! use queue_keeper_core::{EventId, SessionId};
//!
//! // Core types are available for use across the system
//! let event_id = EventId::new();
//! let session_id = SessionId::from_parts("owner", "repo", "pull_request", "123");
//! ```

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

// Re-export commonly used types
pub use ulid::Ulid;
pub use uuid::Uuid;

/// Standard result type for queue-keeper operations
pub type QueueKeeperResult<T> = Result<T, QueueKeeperError>;

// ============================================================================
// Domain Identifier Types
// ============================================================================

/// Unique identifier for webhook events and normalized events
///
/// Uses ULID for lexicographic sorting and global uniqueness.
/// See specs/interfaces/shared-types.md for full specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(Ulid);

impl EventId {
    /// Generate a new unique event ID
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    /// Get string representation of event ID
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for EventId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ulid = s.parse::<Ulid>().map_err(|_| ParseError::InvalidFormat {
            expected: "ULID format".to_string(),
            actual: s.to_string(),
        })?;
        Ok(Self(ulid))
    }
}

/// Identifier for grouping related events for ordered processing
///
/// Format: `{owner}/{repo}/{entity_type}/{entity_id}`
/// See specs/interfaces/shared-types.md for full specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Create new session ID with validation
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(ValidationError::Required {
                field: "session_id".to_string(),
            });
        }

        if value.len() > 128 {
            return Err(ValidationError::TooLong {
                field: "session_id".to_string(),
                max_length: 128,
            });
        }

        // Validate characters (ASCII printable, no consecutive slashes)
        if !value.chars().all(|c| c.is_ascii_graphic() && c != ' ') {
            return Err(ValidationError::InvalidCharacters {
                field: "session_id".to_string(),
                invalid_chars: "non-ASCII or whitespace".to_string(),
            });
        }

        if value.contains("//") || value.starts_with('/') || value.ends_with('/') {
            return Err(ValidationError::InvalidFormat {
                field: "session_id".to_string(),
                message: "consecutive, leading, or trailing slashes not allowed".to_string(),
            });
        }

        Ok(Self(value))
    }

    /// Create session ID from component parts
    pub fn from_parts(owner: &str, repo: &str, entity_type: &str, entity_id: &str) -> Self {
        let value = format!("{}/{}/{}/{}", owner, repo, entity_type, entity_id);
        // This is guaranteed to be valid if inputs are valid
        Self(value)
    }

    /// Get string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

/// GitHub repository identifier (numeric ID from GitHub API)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(u64);

impl RepositoryId {
    /// Create new repository ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get numeric value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RepositoryId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse::<u64>().map_err(|_| ParseError::InvalidFormat {
            expected: "positive integer".to_string(),
            actual: s.to_string(),
        })?;
        Ok(Self::new(id))
    }
}

/// GitHub user identifier for attribution and access control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(u64);

impl UserId {
    /// Create new user ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get numeric value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for UserId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse::<u64>().map_err(|_| ParseError::InvalidFormat {
            expected: "positive integer".to_string(),
            actual: s.to_string(),
        })?;
        Ok(Self::new(id))
    }
}

/// Bot name identifier for configuration and routing
///
/// Represents a bot that consumes events from Queue-Keeper.
/// Must be unique within a configuration and follow naming conventions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BotName(String);

impl BotName {
    /// Create new bot name with validation
    ///
    /// # Validation Rules
    /// - Must be 1-64 characters
    /// - Must contain only alphanumeric characters and hyphens
    /// - Must not start or end with hyphen
    /// - Must not contain consecutive hyphens
    pub fn new(name: impl Into<String>) -> Result<Self, ValidationError> {
        let name = name.into();

        if name.is_empty() {
            return Err(ValidationError::Required {
                field: "bot_name".to_string(),
            });
        }

        if name.len() > 64 {
            return Err(ValidationError::TooLong {
                field: "bot_name".to_string(),
                max_length: 64,
            });
        }

        // Check character restrictions
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(ValidationError::InvalidCharacters {
                field: "bot_name".to_string(),
                invalid_chars: "non-alphanumeric except hyphens".to_string(),
            });
        }

        // Check hyphen placement
        if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
            return Err(ValidationError::InvalidFormat {
                field: "bot_name".to_string(),
                message: "cannot start/end with hyphen or contain consecutive hyphens".to_string(),
            });
        }

        Ok(Self(name))
    }

    /// Get string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BotName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for BotName {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

/// Queue name identifier for Service Bus queues
///
/// Represents a Service Bus queue where events are delivered to bots.
/// Must follow Azure Service Bus naming conventions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueueName(String);

impl QueueName {
    /// Create new queue name with validation
    ///
    /// # Validation Rules
    /// - Must be 1-260 characters
    /// - Must contain only alphanumeric characters, hyphens, and periods
    /// - Must not start or end with period or hyphen
    /// - Must follow pattern: queue-keeper-{bot-name}
    pub fn new(name: impl Into<String>) -> Result<Self, ValidationError> {
        let name = name.into();

        if name.is_empty() {
            return Err(ValidationError::Required {
                field: "queue_name".to_string(),
            });
        }

        if name.len() > 260 {
            return Err(ValidationError::TooLong {
                field: "queue_name".to_string(),
                max_length: 260,
            });
        }

        // Check character restrictions
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        {
            return Err(ValidationError::InvalidCharacters {
                field: "queue_name".to_string(),
                invalid_chars: "non-alphanumeric except hyphens and periods".to_string(),
            });
        }

        // Check naming convention
        if !name.starts_with("queue-keeper-") {
            return Err(ValidationError::InvalidFormat {
                field: "queue_name".to_string(),
                message: "must start with 'queue-keeper-'".to_string(),
            });
        }

        // Check period/hyphen placement
        if name.starts_with('.')
            || name.ends_with('.')
            || name.starts_with('-')
            || name.ends_with('-')
        {
            return Err(ValidationError::InvalidFormat {
                field: "queue_name".to_string(),
                message: "cannot start/end with period or hyphen".to_string(),
            });
        }

        Ok(Self(name))
    }

    /// Get string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract bot name from queue name
    ///
    /// Assumes queue follows convention: queue-keeper-{bot-name}
    pub fn extract_bot_name(&self) -> Option<String> {
        self.0.strip_prefix("queue-keeper-").map(|s| s.to_string())
    }
}

impl fmt::Display for QueueName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for QueueName {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

// ============================================================================
// Repository and User Types
// ============================================================================

/// Repository information extracted from GitHub events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: RepositoryId,
    pub name: String,
    pub full_name: String,
    pub owner: User,
    pub private: bool,
}

impl Repository {
    /// Create new repository
    pub fn new(
        id: RepositoryId,
        name: String,
        full_name: String,
        owner: User,
        private: bool,
    ) -> Self {
        Self {
            id,
            name,
            full_name,
            owner,
            private,
        }
    }

    /// Get owner name
    pub fn owner_name(&self) -> &str {
        &self.owner.login
    }

    /// Get repository name
    pub fn repo_name(&self) -> &str {
        &self.name
    }
}

/// GitHub user information from events and API responses
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub login: String,
    pub user_type: UserType,
}

/// GitHub user type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserType {
    User,
    Bot,
    Organization,
}

// ============================================================================
// Time and Metadata Types
// ============================================================================

/// UTC timestamp with microsecond precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Create timestamp for current moment
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Parse timestamp from RFC3339 string
    pub fn from_rfc3339(s: &str) -> Result<Self, ParseError> {
        let dt = DateTime::parse_from_rfc3339(s)
            .map_err(|_| ParseError::InvalidFormat {
                expected: "RFC3339 datetime".to_string(),
                actual: s.to_string(),
            })?
            .with_timezone(&Utc);
        Ok(Self(dt))
    }

    /// Convert to RFC3339 string
    pub fn to_rfc3339(&self) -> String {
        self.0.to_rfc3339()
    }

    /// Get underlying DateTime
    pub fn as_datetime(&self) -> &DateTime<Utc> {
        &self.0
    }

    /// Add seconds to timestamp
    pub fn add_seconds(&self, seconds: u64) -> Self {
        let duration = chrono::Duration::seconds(seconds as i64);
        Self(self.0 + duration)
    }

    /// Subtract duration from timestamp
    pub fn subtract_duration(&self, duration: Duration) -> Self {
        let chrono_duration = chrono::Duration::from_std(duration).unwrap_or_default();
        Self(self.0 - chrono_duration)
    }

    /// Get year component
    pub fn year(&self) -> i32 {
        self.0.year()
    }

    /// Get month component (1-12)
    pub fn month(&self) -> u32 {
        self.0.month()
    }

    /// Get day component (1-31)
    pub fn day(&self) -> u32 {
        self.0.day()
    }

    /// Get hour component (0-23)
    pub fn hour(&self) -> u32 {
        self.0.hour()
    }

    /// Get duration since another timestamp
    pub fn duration_since(&self, other: Self) -> Duration {
        let chrono_duration = self.0.signed_duration_since(other.0);
        chrono_duration.to_std().unwrap_or_default()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

/// Identifier for tracing requests across system boundaries
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(Uuid);

impl CorrelationId {
    /// Generate new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get string representation
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for CorrelationId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = s.parse::<Uuid>().map_err(|_| ParseError::InvalidFormat {
            expected: "UUID format".to_string(),
            actual: s.to_string(),
        })?;
        Ok(Self(uuid))
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Deployment environment enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Environment {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

impl FromStr for Environment {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(ParseError::InvalidFormat {
                expected: "development, staging, or production".to_string(),
                actual: s.to_string(),
            }),
        }
    }
}

/// Logging level configuration
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// Get string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

impl FromStr for LogLevel {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" | "warning" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(ParseError::InvalidFormat {
                expected: "error, warn, info, debug, or trace".to_string(),
                actual: s.to_string(),
            }),
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// High-level error categorization for retry and alerting decisions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Temporary failures that should be retried
    Transient,
    /// Permanent failures that won't succeed on retry
    Permanent,
    /// Security-related failures requiring immediate attention
    Security,
    /// Configuration errors preventing startup
    Configuration,
}

/// Configuration for retry behavior
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter_enabled: bool,
}

impl RetryPolicy {
    /// Create exponential backoff retry policy
    pub fn exponential() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter_enabled: true,
        }
    }

    /// Create linear backoff retry policy
    pub fn linear() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 1.0,
            jitter_enabled: true,
        }
    }

    /// Create fixed delay retry policy
    pub fn fixed(delay: Duration) -> Self {
        Self {
            max_attempts: 5,
            base_delay: delay,
            max_delay: delay,
            backoff_multiplier: 1.0,
            jitter_enabled: false,
        }
    }

    /// Calculate delay for specific attempt number
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let mut delay = self.base_delay.as_millis() as f64;

        // Apply backoff multiplier
        for _ in 1..attempt {
            delay *= self.backoff_multiplier;
        }

        // Apply jitter if enabled
        if self.jitter_enabled {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            attempt.hash(&mut hasher);
            let hash = hasher.finish();

            // ±25% jitter
            let jitter_factor = 0.75 + (hash % 500) as f64 / 2000.0;
            delay *= jitter_factor;
        }

        // Cap at maximum delay
        let delay_ms = delay.min(self.max_delay.as_millis() as f64) as u64;
        Duration::from_millis(delay_ms)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::exponential()
    }
}

/// Error type for input validation failures
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum ValidationError {
    #[error("Field '{field}' is required")]
    Required { field: String },

    #[error("Field '{field}' has invalid format: {message}")]
    InvalidFormat { field: String, message: String },

    #[error("Field '{field}' exceeds maximum length of {max_length}")]
    TooLong { field: String, max_length: usize },

    #[error("Field '{field}' is below minimum length of {min_length}")]
    TooShort { field: String, min_length: usize },

    #[error("Field '{field}' contains invalid characters: {invalid_chars}")]
    InvalidCharacters {
        field: String,
        invalid_chars: String,
    },
}

/// Error type for string parsing failures
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid format: expected {expected}, got '{actual}'")]
    InvalidFormat { expected: String, actual: String },

    #[error("Invalid character at position {position}: '{character}'")]
    InvalidCharacter { position: usize, character: char },

    #[error("Value too long: maximum {max_length} characters, got {actual_length}")]
    TooLong {
        max_length: usize,
        actual_length: usize,
    },
}

/// Top-level error type for queue-keeper operations
#[derive(Debug, thiserror::Error)]
pub enum QueueKeeperError {
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("External service error: {service} - {message}")]
    ExternalService { service: String, message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl QueueKeeperError {
    /// Check if error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        match self {
            Self::ExternalService { .. } => true,
            Self::Internal { .. } => true,
            Self::Validation(_) => false,
            Self::Parse(_) => false,
            Self::Configuration { .. } => false,
        }
    }

    /// Get error category for monitoring and alerting
    pub fn error_category(&self) -> ErrorCategory {
        match self {
            Self::Validation(_) => ErrorCategory::Permanent,
            Self::Parse(_) => ErrorCategory::Permanent,
            Self::Configuration { .. } => ErrorCategory::Configuration,
            Self::ExternalService { .. } => ErrorCategory::Transient,
            Self::Internal { .. } => ErrorCategory::Transient,
        }
    }
}

// ============================================================================
// Module declarations
// ============================================================================

/// Webhook processing module for GitHub webhooks
pub mod webhook;

/// Bot configuration module for event routing
pub mod bot_config;

/// Key Vault module for secure secret management
pub mod key_vault;

/// Event replay module for administrative reprocessing
pub mod event_replay;

/// Audit logging module for compliance and security
pub mod audit_logging;

/// Queue integration module for event routing
pub mod queue_integration;

/// Blob storage module for webhook payload persistence
pub mod blob_storage;

/// Storage adapters module for infrastructure implementations
pub mod adapters;

// Re-export key types for convenience
pub use adapters::FilesystemBlobStorage;
pub use audit_logging::{
    AuditActor, AuditContext, AuditError, AuditEvent, AuditEventType, AuditLogId, AuditLogger,
    AuditQuery, AuditResource, AuditResult, SecurityAuditEvent, WebhookProcessingAction,
};
pub use blob_storage::{
    BlobMetadata, BlobStorage, BlobStorageError, DateRange, PayloadFilter, PayloadMetadata,
    StorageHealthStatus, StorageMetrics, StoredWebhook, WebhookPayload,
};
pub use bot_config::{
    BotConfigError, BotConfiguration, BotConfigurationProvider, BotSubscription,
    ConfigurationLoader, EventMatcher, EventTypePattern, QueueDestination, RepositoryFilter,
    RoutingDecision,
};
pub use event_replay::{
    EventFilter, EventReplayService, EventRetriever, ProcessingStatus, ReplayError, ReplayExecutor,
    ReplayId, ReplayRequest, ReplayState, ReplayStatus, ReplayType, StoredEvent,
};
pub use key_vault::{
    CachedSecret, KeyVaultConfiguration, KeyVaultError, KeyVaultProvider, SecretCache, SecretName,
    SecretRotationHandler, SecretValue, StandardSecrets,
};
pub use queue_integration::{
    DefaultEventRouter, DeliveryResult, EventRouter, FailedDelivery, QueueDeliveryError,
    SuccessfulDelivery,
};
pub use webhook::{EventEntity, EventEnvelope, WebhookError, WebhookProcessor};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
