//! GitHub webhook event types and processing.
//!
//! This module provides type-safe event parsing, validation, and normalization
//! for GitHub webhook events. It bridges the gap between raw GitHub webhook
//! payloads and the bot processing system.
//!
//! # Overview
//!
//! The events module defines:
//! - Event envelope types for normalized event representation
//! - Typed event structures for different GitHub event types
//! - Event processing pipeline for webhook conversion
//! - Session management for ordered event processing
//!
//! # Examples
//!
//! ## Processing a Webhook Event
//!
//! ```rust,no_run
//! use github_bot_sdk::events::{EventProcessor, ProcessorConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ProcessorConfig::default();
//! let processor = EventProcessor::new(config);
//!
//! // Process incoming webhook
//! let payload_bytes = b"{\"action\": \"opened\", \"repository\": {}}";
//! let envelope = processor.process_webhook(
//!     "pull_request",
//!     payload_bytes,
//!     Some("12345-67890-abcdef"),
//! ).await?;
//!
//! println!("Event ID: {}", envelope.event_id);
//! println!("Repository: {}", envelope.repository.full_name);
//! # Ok(())
//! # }
//! ```
//!
//! ## Typed Event Handling
//!
//! ```rust,no_run
//! # use github_bot_sdk::events::EventEnvelope;
//! # fn handle_event(envelope: EventEnvelope) -> Result<(), Box<dyn std::error::Error>> {
//! match envelope.event_type.as_str() {
//!     "pull_request" => {
//!         let pr_event = envelope.payload.parse_pull_request()?;
//!         println!("PR #{} was {}", pr_event.number, pr_event.action);
//!     }
//!     "issues" => {
//!         let issue_event = envelope.payload.parse_issue()?;
//!         println!("Issue #{} was {}", issue_event.issue.number, issue_event.action);
//!     }
//!     _ => println!("Unhandled event type"),
//! }
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::client::Repository;
use crate::error::EventError;

pub mod github_events;
pub mod processor;
pub mod session;

// Re-export GitHub event types
pub use github_events::*;
pub use processor::{EventProcessor, ProcessorConfig, SessionIdStrategy};
pub use session::SessionManager;

// ============================================================================
// Event Envelope
// ============================================================================

/// Primary event container that wraps all GitHub events in a normalized format.
///
/// The EventEnvelope provides a consistent structure for all webhook events,
/// regardless of their source event type. It includes metadata for routing,
/// correlation, and session-based ordering.
///
/// # Examples
///
/// ```rust
/// use github_bot_sdk::events::{EventEnvelope, EventPayload, EntityType};
/// use github_bot_sdk::client::{Repository, RepositoryOwner, OwnerType};
/// use serde_json::json;
/// use chrono::Utc;
///
/// # let repository = Repository {
/// #     id: 12345,
/// #     name: "repo".to_string(),
/// #     full_name: "owner/repo".to_string(),
/// #     owner: RepositoryOwner {
/// #         login: "owner".to_string(),
/// #         id: 1,
/// #         avatar_url: "https://example.com/avatar.png".to_string(),
/// #         owner_type: OwnerType::User,
/// #     },
/// #     private: false,
/// #     description: None,
/// #     default_branch: "main".to_string(),
/// #     html_url: "https://github.com/owner/repo".to_string(),
/// #     clone_url: "https://github.com/owner/repo.git".to_string(),
/// #     ssh_url: "git@github.com:owner/repo.git".to_string(),
/// #     created_at: Utc::now(),
/// #     updated_at: Utc::now(),
/// # };
/// let payload = EventPayload::new(json!({"action": "opened"}));
///
/// let envelope = EventEnvelope::new(
///     "pull_request".to_string(),
///     repository,
///     payload,
/// );
///
/// assert_eq!(envelope.event_type, "pull_request");
/// assert_eq!(envelope.entity_type, EntityType::PullRequest);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Unique identifier for this event
    pub event_id: EventId,

    /// GitHub event type (e.g., "pull_request", "issues", "push")
    pub event_type: String,

    /// Repository where the event occurred
    pub repository: Repository,

    /// Primary entity type involved in the event
    pub entity_type: EntityType,

    /// Identifier of the primary entity (e.g., PR number, issue number)
    pub entity_id: Option<String>,

    /// Session ID for ordered processing of related events
    pub session_id: Option<String>,

    /// Raw event payload from GitHub
    pub payload: EventPayload,

    /// Processing and routing metadata
    pub metadata: EventMetadata,

    /// Distributed tracing context
    pub trace_context: Option<TraceContext>,
}

impl EventEnvelope {
    /// Create a new event envelope.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use github_bot_sdk::events::{EventEnvelope, EventPayload};
    /// # use github_bot_sdk::client::{Repository, RepositoryOwner, OwnerType};
    /// # use serde_json::json;
    /// # use chrono::Utc;
    /// # let repository = Repository {
    /// #     id: 1,
    /// #     name: "repo".to_string(),
    /// #     full_name: "owner/repo".to_string(),
    /// #     owner: RepositoryOwner {
    /// #         login: "owner".to_string(),
    /// #         id: 1,
    /// #         avatar_url: "https://example.com/avatar.png".to_string(),
    /// #         owner_type: OwnerType::User,
    /// #     },
    /// #     private: false,
    /// #     description: None,
    /// #     default_branch: "main".to_string(),
    /// #     html_url: "https://github.com/owner/repo".to_string(),
    /// #     clone_url: "https://github.com/owner/repo.git".to_string(),
    /// #     ssh_url: "git@github.com:owner/repo.git".to_string(),
    /// #     created_at: Utc::now(),
    /// #     updated_at: Utc::now(),
    /// # };
    /// let payload = EventPayload::new(json!({"action": "opened"}));
    /// let envelope = EventEnvelope::new("pull_request".to_string(), repository, payload);
    /// ```
    pub fn new(event_type: String, repository: Repository, payload: EventPayload) -> Self {
        let entity_type = EntityType::from_event_type(&event_type);

        Self {
            event_id: EventId::new(),
            event_type,
            repository,
            entity_type,
            entity_id: None,
            session_id: None,
            payload,
            metadata: EventMetadata::default(),
            trace_context: None,
        }
    }

    /// Add a session ID for ordered processing.
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Add trace context for distributed tracing.
    pub fn with_trace_context(mut self, context: TraceContext) -> Self {
        self.trace_context = Some(context);
        self
    }

    /// Get a unique key for the primary entity.
    ///
    /// Returns a string in the format "repo:owner/name:entity_type:entity_id"
    /// for entities with IDs, or "repo:owner/name" for repository-level events.
    pub fn entity_key(&self) -> String {
        if let Some(ref entity_id) = self.entity_id {
            format!(
                "repo:{}:{:?}:{}",
                self.repository.full_name, self.entity_type, entity_id
            )
        } else {
            format!("repo:{}", self.repository.full_name)
        }
    }

    /// Get the correlation ID for this event.
    ///
    /// Returns the event ID as a string for correlation across system boundaries.
    pub fn correlation_id(&self) -> &str {
        self.event_id.as_str()
    }
}

// ============================================================================
// Event ID
// ============================================================================

/// Unique identifier for events, ensuring idempotency and deduplication.
///
/// Event IDs can be generated from GitHub delivery IDs or created as new UUIDs.
/// They are used for event deduplication and correlation across the system.
///
/// # Examples
///
/// ```rust
/// use github_bot_sdk::events::EventId;
///
/// // Create from GitHub delivery ID
/// let id = EventId::from_github_delivery("12345-67890-abcdef");
/// assert_eq!(id.as_str(), "gh-12345-67890-abcdef");
///
/// // Create new random ID
/// let id = EventId::new();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(String);

impl EventId {
    /// Create a new random event ID using UUID v4.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create an event ID from a GitHub delivery ID.
    ///
    /// GitHub delivery IDs are prefixed with "gh-" to distinguish them
    /// from internally generated IDs.
    pub fn from_github_delivery(delivery_id: &str) -> Self {
        Self(format!("gh-{}", delivery_id))
    }

    /// Get the event ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
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

// ============================================================================
// Entity Type
// ============================================================================

/// Classifies the primary entity involved in the event for session correlation.
///
/// Entity types are used to group related events for ordered processing and
/// to determine session ID generation strategies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    /// Repository-level event
    Repository,
    /// Pull request event
    PullRequest,
    /// Issue event
    Issue,
    /// Branch event (push, create, delete)
    Branch,
    /// Release event
    Release,
    /// User event
    User,
    /// Organization event
    Organization,
    /// Check run event
    CheckRun,
    /// Check suite event
    CheckSuite,
    /// Deployment event
    Deployment,
    /// Unknown or unsupported entity type
    Unknown,
}

impl EntityType {
    /// Determine entity type from GitHub event type string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use github_bot_sdk::events::EntityType;
    ///
    /// assert_eq!(EntityType::from_event_type("pull_request"), EntityType::PullRequest);
    /// assert_eq!(EntityType::from_event_type("issues"), EntityType::Issue);
    /// assert_eq!(EntityType::from_event_type("push"), EntityType::Branch);
    /// assert_eq!(EntityType::from_event_type("unknown"), EntityType::Unknown);
    /// ```
    pub fn from_event_type(event_type: &str) -> Self {
        match event_type {
            "pull_request" | "pull_request_review" | "pull_request_review_comment" => {
                Self::PullRequest
            }
            "issues" | "issue_comment" => Self::Issue,
            "push" | "create" | "delete" => Self::Branch,
            "release" | "release_published" => Self::Release,
            "check_run" => Self::CheckRun,
            "check_suite" => Self::CheckSuite,
            "deployment" | "deployment_status" => Self::Deployment,
            "repository" => Self::Repository,
            "organization" | "member" | "membership" => Self::Organization,
            _ => Self::Unknown,
        }
    }

    /// Check if this entity type supports ordered processing.
    ///
    /// Returns true for entity types where event ordering matters
    /// (pull requests, issues, branches).
    pub fn supports_ordering(&self) -> bool {
        matches!(self, Self::PullRequest | Self::Issue | Self::Branch)
    }
}

// ============================================================================
// Event Payload
// ============================================================================

/// Container for the actual GitHub webhook payload data.
///
/// The payload stores the raw JSON value and provides typed parsing methods
/// for different event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    inner: serde_json::Value,
}

impl EventPayload {
    /// Create a new event payload from a JSON value.
    pub fn new(value: serde_json::Value) -> Self {
        Self { inner: value }
    }

    /// Get the raw JSON value.
    pub fn raw(&self) -> &serde_json::Value {
        &self.inner
    }

    /// Parse as a pull request event.
    pub fn parse_pull_request(&self) -> Result<PullRequestEvent, EventError> {
        Ok(serde_json::from_value(self.inner.clone())?)
    }

    /// Parse as an issue event.
    pub fn parse_issue(&self) -> Result<IssueEvent, EventError> {
        Ok(serde_json::from_value(self.inner.clone())?)
    }

    /// Parse as a push event.
    pub fn parse_push(&self) -> Result<PushEvent, EventError> {
        Ok(serde_json::from_value(self.inner.clone())?)
    }

    /// Parse as a check run event.
    pub fn parse_check_run(&self) -> Result<CheckRunEvent, EventError> {
        Ok(serde_json::from_value(self.inner.clone())?)
    }

    /// Parse as a check suite event.
    pub fn parse_check_suite(&self) -> Result<CheckSuiteEvent, EventError> {
        Ok(serde_json::from_value(self.inner.clone())?)
    }
}

// ============================================================================
// Event Metadata
// ============================================================================

/// Additional metadata about event processing and routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// When the event was received by our system
    pub received_at: DateTime<Utc>,

    /// When processing completed (if applicable)
    pub processed_at: Option<DateTime<Utc>>,

    /// Source of this event
    pub source: EventSource,

    /// GitHub delivery ID from X-GitHub-Delivery header
    pub delivery_id: Option<String>,

    /// Whether the webhook signature was valid
    pub signature_valid: bool,

    /// Number of times this event has been retried
    pub retry_count: u32,

    /// Names of routing rules that matched this event
    pub routing_rules: Vec<String>,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            received_at: Utc::now(),
            processed_at: None,
            source: EventSource::GitHub,
            delivery_id: None,
            signature_valid: false,
            retry_count: 0,
            routing_rules: Vec::new(),
        }
    }
}

/// Source of an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSource {
    /// Event from GitHub webhook
    GitHub,
    /// Event from replay operation
    Replay,
    /// Event from test/development
    Test,
}

// ============================================================================
// Trace Context
// ============================================================================

/// Distributed tracing context for event correlation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Trace ID for distributed tracing
    pub trace_id: String,

    /// Span ID for this specific operation
    pub span_id: String,

    /// Parent span ID if applicable
    pub parent_span_id: Option<String>,
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
