# Bot Configuration Interface

**Architectural Layer**: Core Domain
**Module Path**: `src/bot_config.rs`
**Responsibilities** (from RDD):

- Knows: Bot subscription rules, queue configurations, event filtering logic
- Does: Determines event routing, validates bot configurations, provides runtime configuration access

## Dependencies

- Types: `EventEnvelope`, `QueueName`, `BotName` (shared-types.md)
- Shared: `Result<T, E>`, `ValidationError` (shared-types.md)
- External: Configuration files, environment variables

## Overview

The Bot Configuration Interface defines how Queue-Keeper determines which normalized events should be routed to which bot queues. This system implements REQ-010 (Bot Subscription Configuration) by providing static configuration that maps event types to bot queues with ordering requirements.

**Critical Design Principles:**

- **Static Configuration**: All bot subscriptions defined at startup, no hot-reloading
- **Event Type Filtering**: Bots specify exact GitHub event types they want to receive
- **One-to-Many Routing**: Single event can be delivered to multiple bot queues
- **Ordering Constraints**: Bots specify whether they need session-based ordered delivery
- **Fail-Fast Validation**: Invalid configuration prevents application startup

## Types

### BotConfiguration

Core configuration structure defining all bot subscriptions and routing rules.

```rust
/// Complete bot configuration loaded at startup
///
/// Contains all bot subscription definitions and routing rules.
/// Configuration is immutable after loading and validation.
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
    pub fn load_from_file(path: &Path) -> Result<Self, BotConfigError>;

    /// Load configuration from environment variables
    ///
    /// Expected format: JSON string in `BOT_CONFIGURATION` environment variable
    pub fn load_from_env() -> Result<Self, BotConfigError>;

    /// Validate configuration structure and constraints
    ///
    /// Checks for duplicate bot names, invalid queue names, unknown event types
    pub fn validate(&self) -> Result<(), BotConfigError>;

    /// Get all bots that should receive the given event
    pub fn get_target_bots(&self, event: &EventEnvelope) -> Vec<&BotSubscription>;
}
```

### BotSubscription

Individual bot subscription definition specifying which events the bot wants to receive.

```rust
/// Bot subscription configuration
///
/// Defines a single bot's event subscriptions and processing requirements.
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
    pub fn matches_event(&self, event: &EventEnvelope) -> bool;

    /// Get the effective queue name for this bot
    pub fn get_queue_name(&self) -> &QueueName;

    /// Check if this bot requires session-based ordering
    pub fn requires_ordering(&self) -> bool;
}
```

### EventTypePattern

Pattern matching for GitHub event types with support for wildcards and exclusions.

```rust
/// Event type pattern for bot subscriptions
///
/// Supports exact matches, wildcards, and exclusion patterns.
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
    /// Create from string representation
    ///
    /// # Examples
    /// - "issues.opened" → Exact("issues.opened")
    /// - "issues.*" → Wildcard("issues.*")
    /// - "pull_request" → EntityAll("pull_request")
    /// - "!push" → Exclude("push")
    pub fn from_str(pattern: &str) -> Result<Self, BotConfigError>;

    /// Check if this pattern matches the given event type
    pub fn matches(&self, event_type: &str) -> bool;

    /// Get the base entity type (pull_request, issues, etc.)
    pub fn get_entity_type(&self) -> Option<&str>;
}
```

### RepositoryFilter

Optional filtering based on repository characteristics.

```rust
/// Repository-based filtering for bot subscriptions
///
/// Allows bots to subscribe only to events from specific repositories
/// or repositories matching certain criteria.
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
    pub fn matches(&self, repository: &Repository) -> bool;

    /// Validate filter patterns (especially regex)
    pub fn validate(&self) -> Result<(), BotConfigError>;
}
```

### BotSpecificConfig

Bot-specific configuration options that are passed through to the bot.

```rust
/// Bot-specific configuration options
///
/// Opaque configuration data that is passed to bots without interpretation
/// by Queue-Keeper. Allows bots to receive custom configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotSpecificConfig {
    /// Free-form configuration data
    pub settings: HashMap<String, serde_json::Value>,
}

impl BotSpecificConfig {
    /// Create empty configuration
    pub fn new() -> Self;

    /// Add configuration value
    pub fn with_setting(mut self, key: String, value: serde_json::Value) -> Self;

    /// Get configuration value by key
    pub fn get(&self, key: &str) -> Option<&serde_json::Value>;

    /// Check if configuration is empty
    pub fn is_empty(&self) -> bool;
}
```

### BotConfigurationSettings

Global configuration settings that affect all bots.

```rust
/// Global bot configuration settings
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
```

## Core Interfaces

### BotConfigurationProvider

Main interface for accessing bot configuration and routing logic.

```rust
/// Interface for bot configuration management
///
/// Provides access to bot subscriptions and routing decisions.
/// Implementation is typically a singleton loaded at startup.
#[async_trait::async_trait]
pub trait BotConfigurationProvider: Send + Sync {
    /// Get complete bot configuration
    async fn get_configuration(&self) -> Result<&BotConfiguration, BotConfigError>;

    /// Get all bots that should receive the given event
    async fn get_target_bots(&self, event: &EventEnvelope) -> Result<Vec<BotSubscription>, BotConfigError>;

    /// Get specific bot subscription by name
    async fn get_bot_subscription(&self, bot_name: &BotName) -> Result<Option<BotSubscription>, BotConfigError>;

    /// List all configured bot names
    async fn list_bot_names(&self) -> Result<Vec<BotName>, BotConfigError>;

    /// Validate that all configured queues exist and are accessible
    async fn validate_queue_connectivity(&self) -> Result<(), BotConfigError>;
}
```

### ConfigurationLoader

Interface for loading configuration from various sources.

```rust
/// Interface for loading bot configuration
///
/// Abstracts configuration source (files, environment, remote config)
/// to enable testing and different deployment scenarios.
#[async_trait::async_trait]
pub trait ConfigurationLoader: Send + Sync {
    /// Load configuration from the configured source
    async fn load_configuration(&self) -> Result<BotConfiguration, BotConfigError>;

    /// Check if configuration source is available
    async fn is_available(&self) -> bool;

    /// Get configuration source description for logging
    fn get_source_description(&self) -> String;
}
```

### EventMatcher

Interface for event pattern matching logic.

```rust
/// Interface for event matching logic
///
/// Determines whether events match bot subscription patterns.
/// Separated for testability and potential future customization.
pub trait EventMatcher: Send + Sync {
    /// Check if event matches the given subscription
    fn matches_subscription(&self, event: &EventEnvelope, subscription: &BotSubscription) -> bool;

    /// Check if event type matches the given pattern
    fn matches_pattern(&self, event_type: &str, pattern: &EventTypePattern) -> bool;

    /// Check if repository matches the given filter
    fn matches_repository(&self, repository: &Repository, filter: &RepositoryFilter) -> bool;
}
```

## Supporting Types

### QueueDestination

Represents a target queue for event routing.

```rust
/// Queue destination for event routing
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
    ) -> Self;

    /// Check if this destination requires ordered processing
    pub fn requires_ordering(&self) -> bool;
}
```

### RoutingDecision

Result of routing decision for a given event.

```rust
/// Result of event routing decision
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
    pub fn new(event_id: EventId, destinations: Vec<QueueDestination>) -> Self;

    /// Check if any destinations were found
    pub fn has_destinations(&self) -> bool;

    /// Get destinations requiring ordered processing
    pub fn get_ordered_destinations(&self) -> Vec<&QueueDestination>;

    /// Get destinations allowing parallel processing
    pub fn get_parallel_destinations(&self) -> Vec<&QueueDestination>;
}
```

### RoutingMetadata

Debugging and observability information about routing decisions.

```rust
/// Metadata about routing decisions for observability
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

/// Reason why a bot subscription was excluded from routing
#[derive(Debug, Clone)]
pub struct ExclusionReason {
    pub bot_name: BotName,
    pub reason: String,
    pub pattern_tested: Option<String>,
}
```

## Error Types

### BotConfigError

Comprehensive error type for bot configuration operations.

```rust
/// Errors that can occur during bot configuration operations
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

    #[error("Configuration source unavailable: {source}")]
    SourceUnavailable { source: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl BotConfigError {
    /// Check if this error is transient and might succeed on retry
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            BotConfigError::SourceUnavailable { .. } |
            BotConfigError::QueueConnectivityFailed { .. }
        )
    }

    /// Get user-friendly error description
    pub fn get_user_message(&self) -> String {
        match self {
            BotConfigError::FileNotFound { .. } => {
                "Configuration file not found. Check file path and permissions.".to_string()
            }
            BotConfigError::ValidationError { .. } => {
                "Configuration contains errors. Check bot names, queue names, and event patterns.".to_string()
            }
            _ => self.to_string(),
        }
    }
}
```

## Configuration Format

### YAML Configuration Example

```yaml
# Bot configuration example
# This configuration defines which bots receive which GitHub events

settings:
  max_bots: 10
  default_message_ttl: 86400  # 24 hours
  validate_on_startup: true
  log_configuration: true

bots:
  - name: "task-tactician"
    queue: "queue-keeper-task-tactician"
    ordered: true
    events:
      - "issues.opened"
      - "issues.closed"
      - "issues.labeled"
      - "issues.unlabeled"
      - "issues.assigned"
      - "issues.unassigned"
    config:
      settings:
        auto_assign_labels: ["task", "needs-triage"]
        ignore_draft_issues: true

  - name: "merge-warden"
    queue: "queue-keeper-merge-warden"
    ordered: true
    events:
      - "pull_request.opened"
      - "pull_request.synchronize"
      - "pull_request.closed"
      - "pull_request.ready_for_review"
      - "pull_request_review.submitted"
    repository_filter:
      owner: "my-organization"
    config:
      settings:
        require_reviews: 2
        check_ci_status: true
        auto_merge_dependabot: false

  - name: "spec-sentinel"
    queue: "queue-keeper-spec-sentinel"
    ordered: false  # Can process events in parallel
    events:
      - "push"
      - "pull_request.opened"
    repository_filter:
      name_pattern: ".*-specs$"  # Only repositories ending in "-specs"
    config:
      settings:
        validate_yaml: true
        check_schema_versions: true

  - name: "security-scanner"
    queue: "queue-keeper-security-scanner"
    ordered: false
    events:
      - "push"
      - "pull_request.opened"
      - "pull_request.synchronize"
    repository_filter:
      any_of:
        - owner: "security-team"
        - name_pattern: ".*-security-.*"
    config:
      settings:
        scan_dependencies: true
        check_secrets: true
        notify_security_team: true

  - name: "release-manager"
    queue: "queue-keeper-release-manager"
    ordered: true
    events:
      - "release.published"
      - "release.created"
      - "create"  # Tag creation
    config:
      settings:
        auto_deploy_staging: true
        notify_stakeholders: true
```

### Environment Variable Configuration

```bash
# Alternative: JSON configuration in environment variable
export BOT_CONFIGURATION='{
  "settings": {
    "max_bots": 10,
    "default_message_ttl": 86400,
    "validate_on_startup": true,
    "log_configuration": true
  },
  "bots": [
    {
      "name": "task-tactician",
      "queue": "queue-keeper-task-tactician",
      "ordered": true,
      "events": ["issues.opened", "issues.closed"],
      "config": {
        "settings": {
          "auto_assign_labels": ["task", "needs-triage"]
        }
      }
    }
  ]
}'
```

## Implementation Examples

### Routing Logic Example

```rust
// Example of how routing decisions are made
impl BotConfigurationProvider for DefaultBotConfigurationProvider {
    async fn get_target_bots(&self, event: &EventEnvelope) -> Result<Vec<BotSubscription>, BotConfigError> {
        let config = self.get_configuration().await?;
        let mut matching_bots = Vec::new();

        for bot in &config.bots {
            // Check event type patterns
            let event_matches = bot.events.iter().any(|pattern| {
                self.event_matcher.matches_pattern(&event.event_type, pattern)
            });

            if !event_matches {
                continue;
            }

            // Check repository filter if specified
            if let Some(ref filter) = bot.repository_filter {
                if !self.event_matcher.matches_repository(&event.repository, filter) {
                    continue;
                }
            }

            matching_bots.push(bot.clone());
        }

        Ok(matching_bots)
    }
}
```

### Event Pattern Matching Example

```rust
impl EventMatcher for DefaultEventMatcher {
    fn matches_pattern(&self, event_type: &str, pattern: &EventTypePattern) -> bool {
        match pattern {
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
}
```

## Validation Rules

### Configuration Validation

1. **Bot Names**: Must be unique, non-empty, contain only alphanumeric characters and hyphens
2. **Queue Names**: Must follow convention `queue-keeper-{bot-name}`, be valid Service Bus queue names
3. **Event Patterns**: Must be valid GitHub event types or valid wildcard patterns
4. **Repository Filters**: Regex patterns must be valid, repository names must follow GitHub conventions
5. **Ordering**: Bots requiring ordering must specify session-compatible event types
6. **Bot Count**: Total number of bots must not exceed `max_bots` setting

### Runtime Validation

1. **Queue Connectivity**: All configured queues must be reachable at startup
2. **Event Type Recognition**: All configured event patterns must match known GitHub event types
3. **Repository Access**: Repository filters should not reference inaccessible repositories

## Performance Considerations

### Routing Performance

- **Event Type Matching**: Use efficient string matching algorithms for pattern evaluation
- **Repository Filtering**: Cache regex compilation for repository name patterns
- **Configuration Caching**: Keep configuration in memory, avoid repeated file I/O
- **Routing Decision Caching**: Consider caching routing decisions for identical event types

### Memory Usage

- **Configuration Size**: Typical configuration with 10-20 bots should use <1MB memory
- **Pattern Compilation**: Pre-compile regex patterns at startup to avoid runtime compilation
- **Event Matching**: Minimize string allocations during pattern matching

## Security Considerations

### Configuration Security

- **File Permissions**: Configuration files should have restricted read permissions
- **Environment Variables**: Avoid logging configuration that might contain sensitive data
- **Bot Configuration**: Bot-specific config may contain sensitive data, handle appropriately

### Access Control

- **Configuration Loading**: Only authorized processes should load configuration
- **Runtime Modification**: Configuration is read-only after startup, no runtime modifications
- **Queue Access**: Validate that application has permission to access all configured queues

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_event_pattern_matching() {
        let pattern = EventTypePattern::Exact("issues.opened".to_string());
        assert!(pattern.matches("issues.opened"));
        assert!(!pattern.matches("issues.closed"));
    }

    #[test]
    fn test_wildcard_pattern_matching() {
        let pattern = EventTypePattern::Wildcard("issues.*".to_string());
        assert!(pattern.matches("issues.opened"));
        assert!(pattern.matches("issues.closed"));
        assert!(!pattern.matches("pull_request.opened"));
    }

    #[test]
    fn test_repository_filter_exact() {
        let filter = RepositoryFilter::Exact {
            owner: "owner".to_string(),
            name: "repo".to_string(),
        };

        let repo = Repository {
            owner: "owner".to_string(),
            name: "repo".to_string(),
            full_name: "owner/repo".to_string(),
        };

        assert!(filter.matches(&repo));
    }

    #[test]
    fn test_configuration_validation() {
        let mut config = BotConfiguration {
            bots: vec![
                BotSubscription {
                    name: BotName::new("test-bot").unwrap(),
                    queue: QueueName::new("queue-keeper-test-bot").unwrap(),
                    events: vec![EventTypePattern::Exact("issues.opened".to_string())],
                    ordered: true,
                    repository_filter: None,
                    config: BotSpecificConfig::new(),
                }
            ],
            settings: BotConfigurationSettings::default(),
        };

        assert!(config.validate().is_ok());

        // Test duplicate bot names
        config.bots.push(config.bots[0].clone());
        assert!(config.validate().is_err());
    }
}
```

### Integration Tests

1. **Configuration Loading**: Test loading from files and environment variables
2. **Queue Connectivity**: Test validation of queue accessibility
3. **End-to-End Routing**: Test complete routing flow with sample events
4. **Error Handling**: Test behavior with invalid configurations

### Contract Tests

1. **Event Matching**: Verify pattern matching against real GitHub webhook payloads
2. **Repository Filtering**: Test filters against actual repository structures
3. **Bot Integration**: Verify routed events match bot expectations

## Monitoring and Observability

### Metrics

- `bot_config_load_duration_seconds`: Time to load and validate configuration
- `bot_routing_decisions_total`: Count of routing decisions by result
- `bot_subscriptions_matched_total`: Count of subscription matches by bot
- `bot_config_validation_errors_total`: Count of configuration validation errors

### Logging

```rust
// Configuration loading
info!(
    config_source = %source,
    bots_loaded = %config.bots.len(),
    validation_time_ms = %validation_duration.as_millis(),
    "Bot configuration loaded successfully"
);

// Routing decisions
debug!(
    event_id = %event.event_id,
    event_type = %event.event_type,
    repository = %event.repository.full_name,
    target_bots = ?destinations.iter().map(|d| &d.bot_name).collect::<Vec<_>>(),
    routing_time_ms = %routing_duration.as_millis(),
    "Event routed to target bots"
);

// Configuration errors
error!(
    error = %error,
    config_source = %source,
    "Failed to load bot configuration"
);
```

This bot configuration interface provides comprehensive support for REQ-010 while enabling flexible event routing, clear validation, and strong observability for debugging and monitoring bot subscription behavior.
