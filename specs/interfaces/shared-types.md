# Shared Types Specification

**Module Path**: `crates/queue-keeper-core/src/lib.rs`, `crates/github-bot-sdk/src/lib.rs`, `crates/queue-runtime/src/lib.rs`

**Architectural Layer**: Core Domain Types

**Responsibilities**: Provides fundamental types used across all queue-keeper components

## Dependencies

- Standard library: `std::fmt`, `std::error::Error`
- Serialization: `serde::{Serialize, Deserialize}`
- Time: `chrono::{DateTime, Utc}`
- IDs: `uuid::Uuid`, `ulid::Ulid`

## Common Result Types

### Result<T, E>

Standard result type for all fallible operations across the system.

```rust
pub type Result<T, E = Box<dyn std::error::Error + Send + Sync>> = std::result::Result<T, E>;
```

**Usage**: All domain operations, I/O operations, and external service calls return this type.

### QueueKeeperResult<T>

Application-specific result type for queue-keeper operations.

```rust
pub type QueueKeeperResult<T> = Result<T, QueueKeeperError>;
```

**Purpose**: Provides consistent error handling across queue-keeper core components.

## Domain Identifier Types

### EventId

Unique identifier for webhook events and normalized events.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(Ulid);

impl EventId {
    pub fn new() -> Self;
    pub fn from_str(s: &str) -> Result<Self, ParseError>;
    pub fn as_str(&self) -> &str;
}
```

**Properties**:

- Lexicographically sortable (timestamp-based)
- Globally unique across all instances
- URL-safe string representation
- 26 characters in Crockford Base32

### SessionId

Identifier for grouping related events for ordered processing.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(value: String) -> Result<Self, ValidationError>;
    pub fn from_parts(owner: &str, repo: &str, entity_type: &str, entity_id: &str) -> Self;
    pub fn as_str(&self) -> &str;
}
```

**Format**: `{owner}/{repo}/{entity_type}/{entity_id}`

**Examples**:

- `microsoft/vscode/pull_request/1234`
- `github/docs/issue/5678`
- `owner/repo/branch/main`

**Validation Rules**:

- Maximum 128 characters
- ASCII alphanumeric, hyphens, underscores, slashes only
- No consecutive slashes or leading/trailing slashes

### RepositoryId

GitHub repository identifier (numeric ID from GitHub API).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(u64);

impl RepositoryId {
    pub fn new(id: u64) -> Self;
    pub fn as_u64(&self) -> u64;
}
```

**Purpose**: Stable identifier for repositories that doesn't change when repositories are renamed.

### UserId

GitHub user identifier for attribution and access control.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(u64);

impl UserId {
    pub fn new(id: u64) -> Self;
    pub fn as_u64(&self) -> u64;
}
```

**Purpose**: Identifies GitHub users across username changes and account transfers.

## Repository and User Types

### Repository

Repository information extracted from GitHub events.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: RepositoryId,
    pub name: String,
    pub full_name: String,
    pub owner: User,
    pub private: bool,
}

impl Repository {
    pub fn new(id: RepositoryId, name: String, full_name: String, owner: User, private: bool) -> Self;
    pub fn owner_name(&self) -> &str;
    pub fn repo_name(&self) -> &str;
}
```

**Validation**:

- `name` must match GitHub repository name format (alphanumeric, hyphens, underscores)
- `full_name` must be in format `{owner}/{name}`
- Names limited to 100 characters each

### User

GitHub user information from events and API responses.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub login: String,
    pub user_type: UserType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserType {
    User,
    Bot,
    Organization,
}
```

**Properties**:

- `login` is the current GitHub username (can change)
- `id` is stable across username changes
- `user_type` distinguishes between users, bots, and organizations

## Time and Metadata Types

### Timestamp

UTC timestamp with microsecond precision.

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    pub fn now() -> Self;
    pub fn from_rfc3339(s: &str) -> Result<Self, ParseError>;
    pub fn to_rfc3339(&self) -> String;
    pub fn as_datetime(&self) -> &DateTime<Utc>;
}
```

**Usage**: All event timestamps, processing timestamps, and audit records use this type.

### CorrelationId

Identifier for tracing requests across system boundaries.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(Uuid);

impl CorrelationId {
    pub fn new() -> Self;
    pub fn from_str(s: &str) -> Result<Self, ParseError>;
    pub fn as_str(&self) -> String;
}
```

**Purpose**: Enables distributed tracing and debugging across microservices.

## Validation Types

### ValidationError

Error type for input validation failures.

```rust
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
    InvalidCharacters { field: String, invalid_chars: String },
}
```

**Usage**: All input validation across webhook processing, configuration, and API operations.

### ParseError

Error type for string parsing failures.

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid format: expected {expected}, got '{actual}'")]
    InvalidFormat { expected: String, actual: String },

    #[error("Invalid character at position {position}: '{character}'")]
    InvalidCharacter { position: usize, character: char },

    #[error("Value too long: maximum {max_length} characters, got {actual_length}")]
    TooLong { max_length: usize, actual_length: usize },
}
```

## Configuration Types

### Environment

Deployment environment enumeration.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Environment {
    pub fn from_str(s: &str) -> Result<Self, ParseError>;
    pub fn as_str(&self) -> &str;
}
```

**Usage**: Environment-specific configuration, logging levels, and feature flags.

### LogLevel

Logging level configuration.

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Result<Self, ParseError>;
    pub fn as_str(&self) -> &str;
}
```

## Error Classification Types

### ErrorCategory

High-level error categorization for retry and alerting decisions.

```rust
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
```

**Usage**: Error handling policies, retry decisions, and alerting rules.

### RetryPolicy

Configuration for retry behavior.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter_enabled: bool,
}

impl RetryPolicy {
    pub fn exponential() -> Self;
    pub fn linear() -> Self;
    pub fn fixed(delay: Duration) -> Self;
    pub fn calculate_delay(&self, attempt: u32) -> Duration;
}
```

**Default Values**:

- `max_attempts`: 5
- `base_delay`: 100ms
- `max_delay`: 30 seconds
- `backoff_multiplier`: 2.0
- `jitter_enabled`: true

## Usage Examples

### Event Processing

```rust
use queue_keeper_core::{EventId, SessionId, Timestamp, Repository, User, UserId, RepositoryId, UserType};

// Create new event identifier
let event_id = EventId::new();

// Generate session ID for ordered processing
let session_id = SessionId::from_parts("microsoft", "vscode", "pull_request", "1234");

// Create repository information
let owner = User {
    id: UserId::new(1),
    login: "microsoft".to_string(),
    user_type: UserType::Organization,
};

let repository = Repository::new(
    RepositoryId::new(41881900),
    "vscode".to_string(),
    "microsoft/vscode".to_string(),
    owner,
    false,
);

// Record processing timestamp
let processed_at = Timestamp::now();
```

### Error Handling

```rust
use queue_keeper_core::{ValidationError, ErrorCategory, RetryPolicy};

// Validate input field
fn validate_repository_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::Required {
            field: "repository_name".to_string(),
        });
    }

    if name.len() > 100 {
        return Err(ValidationError::TooLong {
            field: "repository_name".to_string(),
            max_length: 100,
        });
    }

    Ok(())
}

// Configure retry policy
let retry_policy = RetryPolicy::exponential();
let delay = retry_policy.calculate_delay(3); // Third retry attempt
```

### Configuration

```rust
use queue_keeper_core::{Environment, LogLevel, CorrelationId};

// Environment-based configuration
let env = Environment::from_str("production")?;
let log_level = match env {
    Environment::Development => LogLevel::Debug,
    Environment::Staging => LogLevel::Info,
    Environment::Production => LogLevel::Warn,
};

// Distributed tracing
let correlation_id = CorrelationId::new();
tracing::info!(
    correlation_id = %correlation_id.as_str(),
    "Processing webhook event"
);
```

## Implementation Notes

### Performance Considerations

- All types implement `Clone` for efficient copying
- String-based identifiers use `Arc<str>` internally for memory efficiency
- Serialization optimized for JSON with flattened structures where appropriate
- Hash implementations provided for use in HashMap and HashSet collections

### Security Considerations

- No sensitive data (secrets, tokens) in these shared types
- All string inputs validated for length and character restrictions
- Timestamp comparisons use UTC to prevent timezone issues
- User inputs sanitized to prevent injection attacks

### Testing Support

All shared types provide:

- Deterministic test constructors
- Arbitrary implementations for property-based testing
- Mock implementations where appropriate
- JSON round-trip serialization tests

This shared type system ensures consistency across all queue-keeper components while maintaining type safety and providing comprehensive error handling capabilities.
