//! # Bot Configuration Module
//!
//! Defines how Queue-Keeper routes normalized events to specific bot queues based on
//! static subscription configuration. Implements REQ-010 (Bot Subscription Configuration).
//!
//! See specs/interfaces/bot-configuration.md for complete specification.

use crate::{BotName, EventEnvelope, EventId, QueueName, Repository, Timestamp};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path, str::FromStr};

// ============================================================================
// Core Configuration Types
// ============================================================================

/// Complete bot configuration loaded at startup
///
/// Contains all bot subscription definitions and routing rules.
/// Configuration is immutable after loading and validation.
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotConfiguration {
    /// List of bot subscription definitions
    pub bots: Vec<BotSubscription>,

    /// Global configuration options
    pub settings: BotConfigurationSettings,
}

impl BotConfiguration {
    /// Load configuration from file path
    ///
    /// # Errors
    /// - `BotConfigError::FileNotFound` - Configuration file missing
    /// - `BotConfigError::ParseError` - Invalid YAML/JSON syntax
    /// - `BotConfigError::ValidationError` - Invalid configuration structure
    pub fn load_from_file(_path: &Path) -> Result<Self, BotConfigError> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    /// Load configuration from environment variables
    ///
    /// Expected format: JSON string in `BOT_CONFIGURATION` environment variable
    pub fn load_from_env() -> Result<Self, BotConfigError> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    /// Validate configuration structure and constraints
    ///
    /// Checks for duplicate bot names, invalid queue names, unknown event types
    pub fn validate(&self) -> Result<(), BotConfigError> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    /// Get all bots that should receive the given event
    pub fn get_target_bots(&self, _event: &EventEnvelope) -> Vec<&BotSubscription> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }
}

/// Individual bot subscription definition specifying which events the bot wants to receive.
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotSubscription {
    /// Unique bot identifier (used for logging and debugging)
    pub name: BotName,

    /// Target Service Bus queue name
    pub queue: QueueName,

    /// GitHub event types this bot subscribes to
    pub events: Vec<EventTypePattern>,

    /// Whether this bot requires ordered processing
    pub ordered: bool,

    /// Optional repository filters
    pub repository_filter: Option<RepositoryFilter>,

    /// Bot-specific configuration options
    pub config: BotSpecificConfig,
}

impl BotSubscription {
    /// Check if this bot should receive the given event
    pub fn matches_event(&self, _event: &EventEnvelope) -> bool {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    /// Get the effective queue name for this bot
    pub fn get_queue_name(&self) -> &QueueName {
        &self.queue
    }

    /// Check if this bot requires session-based ordering
    pub fn requires_ordering(&self) -> bool {
        self.ordered
    }
}

/// Event type pattern for bot subscriptions
///
/// Supports exact matches, wildcards, and exclusion patterns.
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventTypePattern {
    /// Exact event type match (e.g., "issues.opened")
    Exact(String),

    /// Wildcard pattern (e.g., "issues.*" matches all issue events)
    Wildcard(String),

    /// All events for an entity type (e.g., "pull_request")
    EntityAll(String),

    /// Exclude specific event types from broader patterns
    Exclude(String),
}

impl EventTypePattern {
    /// Check if this pattern matches the given event type
    pub fn matches(&self, _event_type: &str) -> bool {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    /// Get the base entity type (pull_request, issues, etc.)
    pub fn get_entity_type(&self) -> Option<&str> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }
}

impl FromStr for EventTypePattern {
    type Err = BotConfigError;

    fn from_str(_pattern: &str) -> Result<Self, Self::Err> {
        // TODO: Implement pattern parsing
        // Examples:
        // - "issues.opened" → Exact("issues.opened")
        // - "issues.*" → Wildcard("issues.*")
        // - "pull_request" → EntityAll("pull_request")
        // - "!push" → Exclude("push")
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }
}

/// Repository-based filtering for bot subscriptions
///
/// Allows bots to subscribe only to events from specific repositories
/// or repositories matching certain criteria.
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositoryFilter {
    /// Specific repository (owner/name format)
    Exact { owner: String, name: String },

    /// All repositories owned by specific user/organization
    Owner(String),

    /// Repositories matching naming pattern
    NamePattern(String), // Regex pattern

    /// Multiple repository filters (OR logic)
    AnyOf(Vec<RepositoryFilter>),

    /// Multiple repository filters (AND logic)
    AllOf(Vec<RepositoryFilter>),
}

impl RepositoryFilter {
    /// Check if this filter matches the given repository
    pub fn matches(&self, _repository: &Repository) -> bool {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    /// Validate filter patterns (especially regex)
    pub fn validate(&self) -> Result<(), BotConfigError> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }
}

/// Bot-specific configuration options
///
/// Opaque configuration data that is passed to bots without interpretation
/// by Queue-Keeper. Allows bots to receive custom configuration.
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotSpecificConfig {
    /// Free-form configuration data
    pub settings: HashMap<String, serde_json::Value>,
}

impl BotSpecificConfig {
    /// Create empty configuration
    pub fn new() -> Self {
        Self {
            settings: HashMap::new(),
        }
    }

    /// Add configuration value
    pub fn with_setting(mut self, key: String, value: serde_json::Value) -> Self {
        self.settings.insert(key, value);
        self
    }

    /// Get configuration value by key
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.settings.get(key)
    }

    /// Check if configuration is empty
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }
}

impl Default for BotSpecificConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Global bot configuration settings
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotConfigurationSettings {
    /// Maximum number of concurrent bot subscriptions
    pub max_bots: usize,

    /// Default queue message TTL in seconds
    pub default_message_ttl: u64,

    /// Enable configuration validation on startup
    pub validate_on_startup: bool,

    /// Log configuration details on startup
    pub log_configuration: bool,
}

impl Default for BotConfigurationSettings {
    fn default() -> Self {
        Self {
            max_bots: 50,
            default_message_ttl: 24 * 60 * 60, // 24 hours
            validate_on_startup: true,
            log_configuration: true,
        }
    }
}

// ============================================================================
// Interface Traits
// ============================================================================

/// Interface for bot configuration management
///
/// Provides access to bot subscriptions and routing decisions.
/// Implementation is typically a singleton loaded at startup.
///
/// See specs/interfaces/bot-configuration.md
#[async_trait]
pub trait BotConfigurationProvider: Send + Sync {
    /// Get complete bot configuration
    async fn get_configuration(&self) -> Result<&BotConfiguration, BotConfigError>;

    /// Get all bots that should receive the given event
    async fn get_target_bots(
        &self,
        event: &EventEnvelope,
    ) -> Result<Vec<BotSubscription>, BotConfigError>;

    /// Get specific bot subscription by name
    async fn get_bot_subscription(
        &self,
        bot_name: &BotName,
    ) -> Result<Option<BotSubscription>, BotConfigError>;

    /// List all configured bot names
    async fn list_bot_names(&self) -> Result<Vec<BotName>, BotConfigError>;

    /// Validate that all configured queues exist and are accessible
    async fn validate_queue_connectivity(&self) -> Result<(), BotConfigError>;
}

/// Interface for loading bot configuration
///
/// Abstracts configuration source (files, environment, remote config)
/// to enable testing and different deployment scenarios.
///
/// See specs/interfaces/bot-configuration.md
#[async_trait]
pub trait ConfigurationLoader: Send + Sync {
    /// Load configuration from the configured source
    async fn load_configuration(&self) -> Result<BotConfiguration, BotConfigError>;

    /// Check if configuration source is available
    async fn is_available(&self) -> bool;

    /// Get configuration source description for logging
    fn get_source_description(&self) -> String;
}

/// Interface for event matching logic
///
/// Determines whether events match bot subscription patterns.
/// Separated for testability and potential future customization.
///
/// See specs/interfaces/bot-configuration.md
pub trait EventMatcher: Send + Sync {
    /// Check if event matches the given subscription
    fn matches_subscription(&self, event: &EventEnvelope, subscription: &BotSubscription) -> bool;

    /// Check if event type matches the given pattern
    fn matches_pattern(&self, event_type: &str, pattern: &EventTypePattern) -> bool;

    /// Check if repository matches the given filter
    fn matches_repository(&self, repository: &Repository, filter: &RepositoryFilter) -> bool;
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Queue destination for event routing
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueDestination {
    /// Bot that will receive the event
    pub bot_name: BotName,

    /// Target queue name
    pub queue_name: QueueName,

    /// Whether to use session-based ordering
    pub ordered: bool,

    /// Bot-specific configuration to include with event
    pub bot_config: BotSpecificConfig,
}

impl QueueDestination {
    /// Create new queue destination
    pub fn new(
        bot_name: BotName,
        queue_name: QueueName,
        ordered: bool,
        bot_config: BotSpecificConfig,
    ) -> Self {
        Self {
            bot_name,
            queue_name,
            ordered,
            bot_config,
        }
    }

    /// Check if this destination requires ordered processing
    pub fn requires_ordering(&self) -> bool {
        self.ordered
    }
}

/// Result of event routing decision
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Event being routed
    pub event_id: EventId,

    /// Target queue destinations
    pub destinations: Vec<QueueDestination>,

    /// Routing metadata for debugging
    pub metadata: RoutingMetadata,
}

impl RoutingDecision {
    /// Create new routing decision
    pub fn new(event_id: EventId, destinations: Vec<QueueDestination>) -> Self {
        Self {
            event_id,
            destinations,
            metadata: RoutingMetadata::new(),
        }
    }

    /// Check if any destinations were found
    pub fn has_destinations(&self) -> bool {
        !self.destinations.is_empty()
    }

    /// Get destinations requiring ordered processing
    pub fn get_ordered_destinations(&self) -> Vec<&QueueDestination> {
        self.destinations.iter().filter(|d| d.ordered).collect()
    }

    /// Get destinations allowing parallel processing
    pub fn get_parallel_destinations(&self) -> Vec<&QueueDestination> {
        self.destinations.iter().filter(|d| !d.ordered).collect()
    }
}

/// Metadata about routing decisions for observability
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone)]
pub struct RoutingMetadata {
    /// Timestamp when routing decision was made
    pub decided_at: Timestamp,

    /// Number of bots evaluated
    pub bots_evaluated: usize,

    /// Number of matching subscriptions found
    pub subscriptions_matched: usize,

    /// Reasons why certain bots were excluded
    pub exclusion_reasons: Vec<ExclusionReason>,
}

impl RoutingMetadata {
    /// Create new routing metadata
    pub fn new() -> Self {
        Self {
            decided_at: Timestamp::now(),
            bots_evaluated: 0,
            subscriptions_matched: 0,
            exclusion_reasons: Vec::new(),
        }
    }
}

impl Default for RoutingMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// Reason why a bot subscription was excluded from routing
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, Clone)]
pub struct ExclusionReason {
    pub bot_name: BotName,
    pub reason: String,
    pub pattern_tested: Option<String>,
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during bot configuration operations
///
/// See specs/interfaces/bot-configuration.md
#[derive(Debug, thiserror::Error)]
pub enum BotConfigError {
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },

    #[error("Failed to parse configuration: {message}")]
    ParseError { message: String },

    #[error("Configuration validation failed: {errors:?}")]
    ValidationError { errors: Vec<String> },

    #[error("Duplicate bot name: {name}")]
    DuplicateBotName { name: BotName },

    #[error("Invalid queue name format: {queue}")]
    InvalidQueueName { queue: String },

    #[error("Unknown event type pattern: {pattern}")]
    UnknownEventType { pattern: String },

    #[error("Invalid repository filter: {filter} - {reason}")]
    InvalidRepositoryFilter { filter: String, reason: String },

    #[error("Bot configuration not found: {bot_name}")]
    BotNotFound { bot_name: BotName },

    #[error("Queue connectivity check failed: {queue} - {message}")]
    QueueConnectivityFailed { queue: QueueName, message: String },

    #[error("Configuration source unavailable: {0}")]
    SourceUnavailable(String),

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl BotConfigError {
    /// Check if this error is transient and might succeed on retry
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            BotConfigError::SourceUnavailable(_) | BotConfigError::QueueConnectivityFailed { .. }
        )
    }

    /// Get user-friendly error description
    pub fn get_user_message(&self) -> String {
        match self {
            BotConfigError::FileNotFound { .. } => {
                "Configuration file not found. Check file path and permissions.".to_string()
            }
            BotConfigError::ValidationError { .. } => {
                "Configuration contains errors. Check bot names, queue names, and event patterns."
                    .to_string()
            }
            _ => self.to_string(),
        }
    }
}

// ============================================================================
// Default Implementations (Stubs)
// ============================================================================

/// Default bot configuration provider implementation
///
/// See specs/interfaces/bot-configuration.md
pub struct DefaultBotConfigurationProvider {
    configuration: BotConfiguration,
    #[allow(dead_code)]
    event_matcher: Box<dyn EventMatcher>,
}

impl DefaultBotConfigurationProvider {
    /// Create new provider with configuration
    pub fn new(configuration: BotConfiguration, event_matcher: Box<dyn EventMatcher>) -> Self {
        Self {
            configuration,
            event_matcher,
        }
    }
}

#[async_trait]
impl BotConfigurationProvider for DefaultBotConfigurationProvider {
    async fn get_configuration(&self) -> Result<&BotConfiguration, BotConfigError> {
        Ok(&self.configuration)
    }

    async fn get_target_bots(
        &self,
        _event: &EventEnvelope,
    ) -> Result<Vec<BotSubscription>, BotConfigError> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    async fn get_bot_subscription(
        &self,
        _bot_name: &BotName,
    ) -> Result<Option<BotSubscription>, BotConfigError> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    async fn list_bot_names(&self) -> Result<Vec<BotName>, BotConfigError> {
        Ok(self
            .configuration
            .bots
            .iter()
            .map(|b| b.name.clone())
            .collect())
    }

    async fn validate_queue_connectivity(&self) -> Result<(), BotConfigError> {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }
}

/// Default configuration loader implementation
///
/// See specs/interfaces/bot-configuration.md
pub struct FileConfigurationLoader {
    file_path: std::path::PathBuf,
}

impl FileConfigurationLoader {
    /// Create new file-based configuration loader
    pub fn new(file_path: std::path::PathBuf) -> Self {
        Self { file_path }
    }
}

#[async_trait]
impl ConfigurationLoader for FileConfigurationLoader {
    async fn load_configuration(&self) -> Result<BotConfiguration, BotConfigError> {
        BotConfiguration::load_from_file(&self.file_path)
    }

    async fn is_available(&self) -> bool {
        self.file_path.exists()
    }

    fn get_source_description(&self) -> String {
        format!("file://{}", self.file_path.display())
    }
}

/// Default event matcher implementation
///
/// See specs/interfaces/bot-configuration.md
pub struct DefaultEventMatcher;

impl EventMatcher for DefaultEventMatcher {
    fn matches_subscription(
        &self,
        _event: &EventEnvelope,
        _subscription: &BotSubscription,
    ) -> bool {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    fn matches_pattern(&self, _event_type: &str, _pattern: &EventTypePattern) -> bool {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }

    fn matches_repository(&self, _repository: &Repository, _filter: &RepositoryFilter) -> bool {
        unimplemented!("See specs/interfaces/bot-configuration.md")
    }
}

#[cfg(test)]
#[path = "bot_config_tests.rs"]
mod tests;
