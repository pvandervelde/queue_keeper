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
    pub fn load_from_file(path: &Path) -> Result<Self, BotConfigError> {
        // Check if file exists
        if !path.exists() {
            return Err(BotConfigError::FileNotFound {
                path: path.display().to_string(),
            });
        }

        // Read file contents
        let contents = std::fs::read_to_string(path).map_err(|e| BotConfigError::ParseError {
            message: format!("Failed to read file: {}", e),
        })?;

        // Determine file type from extension
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Parse based on file extension
        let config: BotConfiguration = match extension.to_lowercase().as_str() {
            "yaml" | "yml" => {
                serde_yaml::from_str(&contents).map_err(|e| BotConfigError::ParseError {
                    message: format!("Invalid YAML: {}", e),
                })?
            }
            "json" => serde_json::from_str(&contents).map_err(|e| BotConfigError::ParseError {
                message: format!("Invalid JSON: {}", e),
            })?,
            _ => {
                // Try JSON first, then YAML
                serde_json::from_str(&contents)
                    .or_else(|_| serde_yaml::from_str(&contents))
                    .map_err(|e| BotConfigError::ParseError {
                        message: format!("Failed to parse as JSON or YAML: {}", e),
                    })?
            }
        };

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Load configuration from environment variables
    ///
    /// Expected format: JSON string in `BOT_CONFIGURATION` environment variable
    pub fn load_from_env() -> Result<Self, BotConfigError> {
        let config_str = std::env::var("BOT_CONFIGURATION").map_err(|_| {
            BotConfigError::SourceUnavailable(
                "BOT_CONFIGURATION environment variable not set".to_string(),
            )
        })?;

        let config: BotConfiguration =
            serde_json::from_str(&config_str).map_err(|e| BotConfigError::ParseError {
                message: format!("Invalid JSON in BOT_CONFIGURATION: {}", e),
            })?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration structure and constraints
    ///
    /// Checks for duplicate bot names, invalid queue names, unknown event types
    pub fn validate(&self) -> Result<(), BotConfigError> {
        let mut errors = Vec::new();

        // Check maximum number of bots
        if self.bots.len() > self.settings.max_bots {
            errors.push(format!(
                "Too many bots configured: {} (max: {})",
                self.bots.len(),
                self.settings.max_bots
            ));
        }

        // Check for duplicate bot names
        let mut seen_names = std::collections::HashSet::new();
        for bot in &self.bots {
            if !seen_names.insert(bot.name.as_str()) {
                errors.push(format!("Duplicate bot name: {}", bot.name.as_str()));
            }
        }

        // Validate each bot subscription
        for bot in &self.bots {
            // Validate queue name format
            if !bot.queue.as_str().starts_with("queue-keeper-") {
                errors.push(format!(
                    "Bot '{}': Queue name must start with 'queue-keeper-'",
                    bot.name.as_str()
                ));
            }

            // Validate event patterns
            if bot.events.is_empty() {
                errors.push(format!(
                    "Bot '{}': Must have at least one event subscription",
                    bot.name.as_str()
                ));
            }

            // Validate repository filters if present
            if let Some(ref filter) = bot.repository_filter {
                if let Err(e) = filter.validate() {
                    errors.push(format!(
                        "Bot '{}': Invalid repository filter: {}",
                        bot.name.as_str(),
                        e
                    ));
                }
            }
        }

        if !errors.is_empty() {
            return Err(BotConfigError::ValidationError { errors });
        }

        Ok(())
    }

    /// Get all bots that should receive the given event
    pub fn get_target_bots(&self, event: &EventEnvelope) -> Vec<&BotSubscription> {
        self.bots
            .iter()
            .filter(|bot| bot.matches_event(event))
            .collect()
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
    pub fn matches_event(&self, event: &EventEnvelope) -> bool {
        // Check if event type matches any of the bot's subscribed patterns
        let event_matches = self.events.iter().any(|pattern| {
            match pattern {
                EventTypePattern::Exclude(_) => false, // Exclusions handled separately
                _ => pattern.matches(&event.event_type),
            }
        });

        if !event_matches {
            return false;
        }

        // Check if any exclusion patterns apply
        let excluded = self.events.iter().any(|pattern| {
            if let EventTypePattern::Exclude(excluded_type) = pattern {
                &event.event_type == excluded_type
            } else {
                false
            }
        });

        if excluded {
            return false;
        }

        // Check repository filter if specified
        if let Some(ref filter) = self.repository_filter {
            if !filter.matches(&event.repository) {
                return false;
            }
        }

        true
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
    pub fn matches(&self, event_type: &str) -> bool {
        match self {
            EventTypePattern::Exact(exact) => event_type == exact,
            EventTypePattern::Wildcard(wildcard) => {
                // Simple wildcard matching (*.suffix or prefix.*)
                if wildcard.ends_with('*') {
                    let prefix = &wildcard[..wildcard.len() - 1];
                    event_type.starts_with(prefix)
                } else if wildcard.starts_with('*') {
                    let suffix = &wildcard[1..];
                    event_type.ends_with(suffix)
                } else {
                    false
                }
            }
            EventTypePattern::EntityAll(entity) => {
                event_type.starts_with(&format!("{}.", entity)) || event_type == entity
            }
            EventTypePattern::Exclude(_) => {
                // Exclusions are handled by the subscription logic
                false
            }
        }
    }

    /// Get the base entity type (pull_request, issues, etc.)
    pub fn get_entity_type(&self) -> Option<&str> {
        match self {
            EventTypePattern::EntityAll(entity) => Some(entity.as_str()),
            EventTypePattern::Wildcard(wildcard) => {
                // Extract entity from wildcard pattern (e.g., "issues.*" -> "issues")
                if wildcard.ends_with(".*") {
                    Some(&wildcard[..wildcard.len() - 2])
                } else {
                    None
                }
            }
            EventTypePattern::Exact(_) => None,
            EventTypePattern::Exclude(_) => None,
        }
    }
}

impl FromStr for EventTypePattern {
    type Err = BotConfigError;

    fn from_str(pattern: &str) -> Result<Self, Self::Err> {
        if pattern.is_empty() {
            return Err(BotConfigError::UnknownEventType {
                pattern: pattern.to_string(),
            });
        }

        // Handle exclusion pattern (starts with !)
        if let Some(excluded) = pattern.strip_prefix('!') {
            return Ok(EventTypePattern::Exclude(excluded.to_string()));
        }

        // Handle wildcard pattern (contains *)
        if pattern.contains('*') {
            return Ok(EventTypePattern::Wildcard(pattern.to_string()));
        }

        // Check if it's an entity-all pattern (no dot in name, common entity types)
        if !pattern.contains('.') {
            // Known entity types that should use EntityAll
            let known_entities = [
                "pull_request",
                "issues",
                "push",
                "release",
                "repository",
                "create",
                "delete",
            ];
            if known_entities.contains(&pattern) {
                return Ok(EventTypePattern::EntityAll(pattern.to_string()));
            }
        }

        // Otherwise, treat as exact match
        Ok(EventTypePattern::Exact(pattern.to_string()))
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
    pub fn matches(&self, repository: &Repository) -> bool {
        match self {
            RepositoryFilter::Exact { owner, name } => {
                repository.owner.login == *owner && repository.name == *name
            }
            RepositoryFilter::Owner(filter_owner) => repository.owner.login == *filter_owner,
            RepositoryFilter::NamePattern(pattern) => {
                // Use regex matching for name patterns
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(&repository.name)
                } else {
                    false
                }
            }
            RepositoryFilter::AnyOf(filters) => {
                // OR logic - any filter matches
                filters.iter().any(|f| f.matches(repository))
            }
            RepositoryFilter::AllOf(filters) => {
                // AND logic - all filters must match
                filters.iter().all(|f| f.matches(repository))
            }
        }
    }

    /// Validate filter patterns (especially regex)
    pub fn validate(&self) -> Result<(), BotConfigError> {
        match self {
            RepositoryFilter::Exact { owner, name } => {
                if owner.is_empty() || name.is_empty() {
                    return Err(BotConfigError::InvalidRepositoryFilter {
                        filter: format!("Exact({}/{})", owner, name),
                        reason: "Owner and name cannot be empty".to_string(),
                    });
                }
                Ok(())
            }
            RepositoryFilter::Owner(owner) => {
                if owner.is_empty() {
                    return Err(BotConfigError::InvalidRepositoryFilter {
                        filter: format!("Owner({})", owner),
                        reason: "Owner cannot be empty".to_string(),
                    });
                }
                Ok(())
            }
            RepositoryFilter::NamePattern(pattern) => {
                // Validate regex pattern
                regex::Regex::new(pattern).map_err(|e| {
                    BotConfigError::InvalidRepositoryFilter {
                        filter: pattern.clone(),
                        reason: format!("Invalid regex: {}", e),
                    }
                })?;
                Ok(())
            }
            RepositoryFilter::AnyOf(filters) => {
                // Validate all nested filters
                for filter in filters {
                    filter.validate()?;
                }
                Ok(())
            }
            RepositoryFilter::AllOf(filters) => {
                // Validate all nested filters
                for filter in filters {
                    filter.validate()?;
                }
                Ok(())
            }
        }
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
        event: &EventEnvelope,
    ) -> Result<Vec<BotSubscription>, BotConfigError> {
        let matching_bots = self
            .configuration
            .bots
            .iter()
            .filter(|bot| self.event_matcher.matches_subscription(event, bot))
            .cloned()
            .collect();

        Ok(matching_bots)
    }

    async fn get_bot_subscription(
        &self,
        bot_name: &BotName,
    ) -> Result<Option<BotSubscription>, BotConfigError> {
        let subscription = self
            .configuration
            .bots
            .iter()
            .find(|bot| &bot.name == bot_name)
            .cloned();

        Ok(subscription)
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
        // This is a stub implementation that would need actual queue client integration
        // For now, we just return Ok since we don't have queue clients yet
        // Task 14.0 will integrate the actual queue validation
        Ok(())
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
    fn matches_subscription(&self, event: &EventEnvelope, subscription: &BotSubscription) -> bool {
        // Check if event type matches any subscription pattern
        let event_matches = subscription
            .events
            .iter()
            .any(|pattern| self.matches_pattern(&event.event_type, pattern));

        if !event_matches {
            return false;
        }

        // Check repository filter if present
        if let Some(ref filter) = subscription.repository_filter {
            if !self.matches_repository(&event.repository, filter) {
                return false;
            }
        }

        true
    }

    fn matches_pattern(&self, event_type: &str, pattern: &EventTypePattern) -> bool {
        pattern.matches(event_type)
    }

    fn matches_repository(&self, repository: &Repository, filter: &RepositoryFilter) -> bool {
        filter.matches(repository)
    }
}

#[cfg(test)]
#[path = "bot_config_tests.rs"]
mod tests;
