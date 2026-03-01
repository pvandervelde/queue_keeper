//! Processing output types for multi-mode webhook processing.
//!
//! This module provides two output types:
//!
//! - [`WrappedEvent`]: The provider-agnostic normalised event envelope produced
//!   by **wrap-mode** providers. All provider-specific details remain in the
//!   original [`payload`](WrappedEvent::payload) field; the envelope carries only
//!   the minimum metadata needed for routing, ordering, and observability.
//!
//! - [`ProcessingOutput`]: The top-level result enum for any provider, covering
//!   both wrap mode and direct (raw-forward) mode.
//!
//! # Design Rationale
//!
//! All provider-specific structured data is preserved verbatim
//! in `payload`, and consumers extract what they need.
//!
//! For GitHub events the `session_id` field encodes the repository and entity
//! (e.g. `"owner/repo/pull_request/123"`), so ordered processing still works
//! without requiring GitHub-specific types in the common envelope.
//!
//! # Modes
//!
//! | Mode       | Output variant                  | Use case                              |
//! |------------|---------------------------------|---------------------------------------|
//! | **Wrap**   | `Wrapped(WrappedEvent)`         | Normalise into provider-agnostic form |
//! | **Direct** | `Direct { payload, metadata }`  | Forward raw payload as-is             |

use crate::{CorrelationId, EventId, SessionId, Timestamp};
use serde::{Deserialize, Serialize};

// ============================================================================
// WrappedEvent
// ============================================================================

/// Provider-agnostic normalised webhook event envelope.
///
/// Produced when a provider runs in **wrap mode**. Carries the minimum
/// metadata needed for routing, deduplication, ordering, and distributed
/// tracing, plus the original payload verbatim.
///
/// # Provider Identity
///
/// The `provider` field records which provider generated this event
/// (e.g. `"github"`, `"jira"`, `"gitlab"`). It is set by the concrete
/// [`WebhookProcessor`](crate::webhook::WebhookProcessor) implementation,
/// not by the generic processing pipeline.
///
/// # Session-Based Ordering
///
/// `session_id` is `Some` when the provider supports ordered processing (e.g.
/// GitHub events on the same pull request share a session ID derived from
/// repository and entity). It is `None` when no ordering concept applies.
///
/// # Payload Completeness
///
/// All provider-specific details (GitHub: `repository`, `entity`; Jira:
/// project, issue; etc.) are preserved in `payload`. Consumers parse what
/// they need from the original JSON body.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::WrappedEvent;
///
/// let event = WrappedEvent::new(
///     "github".to_string(),
///     "push".to_string(),
///     None,
///     None,
///     serde_json::json!({ "ref": "refs/heads/main" }),
/// );
///
/// assert_eq!(event.provider, "github");
/// assert_eq!(event.event_type, "push");
/// assert!(event.session_id.is_none());
/// assert!(!event.event_id.as_str().is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedEvent {
    /// Unique identifier for this event (auto-generated ULID).
    pub event_id: EventId,

    /// The provider that generated this event (e.g. `"github"`, `"jira"`).
    pub provider: String,

    /// The event type (e.g. `"push"`, `"pull_request"`, `"issue_updated"`).
    pub event_type: String,

    /// Optional action within the event type (e.g. `"opened"`, `"closed"`).
    pub action: Option<String>,

    /// Session identifier for ordered processing.
    ///
    /// `Some` when the provider or event warrants ordered processing.
    /// For GitHub, this encodes the repository and affected entity
    /// (e.g. `"owner/repo/pull_request/123"`).
    /// `None` for providers or events without an ordering requirement.
    pub session_id: Option<SessionId>,

    /// Correlation identifier for distributed tracing.
    pub correlation_id: CorrelationId,

    /// UTC time when the webhook was received by Queue-Keeper.
    pub received_at: Timestamp,

    /// UTC time when processing of this event completed.
    pub processed_at: Timestamp,

    /// The original webhook payload, preserved verbatim.
    ///
    /// All provider-specific structured data lives here. Consumers
    /// extract what they need using the fields appropriate for their
    /// provider (e.g. `payload["repository"]["full_name"]` for GitHub).
    pub payload: serde_json::Value,
}

impl WrappedEvent {
    /// Create a new [`WrappedEvent`] with auto-generated IDs and current timestamps.
    ///
    /// # Arguments
    ///
    /// * `provider` - The provider ID (e.g. `"github"`, `"jira"`).
    /// * `event_type` - The event type string extracted from headers or payload.
    /// * `action` - Optional action within the event type.
    /// * `session_id` - Optional session for ordered processing.
    /// * `payload` - The original request body as parsed JSON.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_core::webhook::WrappedEvent;
    /// use queue_keeper_core::SessionId;
    ///
    /// let session = SessionId::from_parts("owner", "repo", "pull_request", "42");
    /// let event = WrappedEvent::new(
    ///     "github".to_string(),
    ///     "pull_request".to_string(),
    ///     Some("opened".to_string()),
    ///     Some(session),
    ///     serde_json::json!({}),
    /// );
    /// assert_eq!(event.event_type, "pull_request");
    /// assert!(event.session_id.is_some());
    /// ```
    pub fn new(
        provider: String,
        event_type: String,
        action: Option<String>,
        session_id: Option<SessionId>,
        payload: serde_json::Value,
    ) -> Self {
        let now = Timestamp::now();
        Self {
            event_id: EventId::new(),
            provider,
            event_type,
            action,
            session_id,
            correlation_id: CorrelationId::new(),
            received_at: now,
            processed_at: now,
            payload,
        }
    }

    /// Create a new [`WrappedEvent`] with a caller-supplied `received_at` timestamp.
    ///
    /// Use this constructor when you have the actual receive time available
    /// (e.g. from [`WebhookRequest::received_at`]) so that latency metrics
    /// recorded in `received_at` reflect wall-clock receipt rather than the
    /// moment normalization began.
    ///
    /// # Arguments
    ///
    /// * `received_at` - The UTC timestamp at which the webhook was first
    ///   received by the HTTP layer.
    /// * Other arguments — same as [`Self::new`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_core::{webhook::WrappedEvent, Timestamp};
    ///
    /// let received = Timestamp::now();
    /// let event = WrappedEvent::with_received_at(
    ///     received,
    ///     "jira".to_string(),
    ///     "issue_updated".to_string(),
    ///     Some("created".to_string()),
    ///     None,
    ///     serde_json::json!({}),
    /// );
    /// assert_eq!(event.event_type, "issue_updated");
    /// ```
    pub fn with_received_at(
        received_at: Timestamp,
        provider: String,
        event_type: String,
        action: Option<String>,
        session_id: Option<SessionId>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            event_id: EventId::new(),
            provider,
            event_type,
            action,
            session_id,
            correlation_id: CorrelationId::new(),
            received_at,
            processed_at: Timestamp::now(),
            payload,
        }
    }
}

// ============================================================================
// ProcessingOutput
// ============================================================================

/// The result of processing a webhook through any provider.
///
/// Providers operating in **wrap** mode normalise the incoming payload into
/// a provider-agnostic [`WrappedEvent`], while providers operating in
/// **direct** mode forward the raw payload with lightweight tracking metadata.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::ProcessingOutput;
///
/// // Check which mode was used
/// # fn example(output: ProcessingOutput) {
/// if output.is_wrapped() {
///     // normalised WrappedEvent is available via as_wrapped()
/// } else {
///     // raw payload with metadata
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
pub enum ProcessingOutput {
    /// A provider-agnostic normalised event produced by wrap-mode providers.
    ///
    /// This is the standard output for any provider running in wrap mode.
    /// The original payload is preserved inside [`WrappedEvent::payload`]
    /// so consumers can extract provider-specific fields as needed.
    Wrapped(WrappedEvent),

    /// A raw payload forwarded without normalisation by direct-mode providers.
    ///
    /// The original request body is preserved verbatim. The accompanying
    /// [`DirectQueueMetadata`] provides the minimum tracking information
    /// needed for observability and deduplication.
    Direct {
        /// The unmodified webhook request body.
        payload: bytes::Bytes,

        /// Lightweight metadata generated by Queue-Keeper for tracking.
        metadata: DirectQueueMetadata,
    },
}

impl ProcessingOutput {
    /// Returns `true` if this is a [`Wrapped`](Self::Wrapped) output.
    pub fn is_wrapped(&self) -> bool {
        matches!(self, Self::Wrapped(_))
    }

    /// Returns `true` if this is a [`Direct`](Self::Direct) output.
    pub fn is_direct(&self) -> bool {
        matches!(self, Self::Direct { .. })
    }

    /// Returns the event ID regardless of mode.
    ///
    /// For wrapped outputs this is the event ID inside the [`WrappedEvent`].
    /// For direct outputs it is the auto-generated ID in the metadata.
    pub fn event_id(&self) -> EventId {
        match self {
            Self::Wrapped(event) => event.event_id,
            Self::Direct { metadata, .. } => metadata.event_id,
        }
    }

    /// Returns the correlation ID regardless of mode.
    ///
    /// For wrapped outputs this is the correlation ID inside the [`WrappedEvent`].
    /// For direct outputs it is the auto-generated ID in the metadata.
    pub fn correlation_id(&self) -> &CorrelationId {
        match self {
            Self::Wrapped(event) => &event.correlation_id,
            Self::Direct { metadata, .. } => &metadata.correlation_id,
        }
    }

    /// Returns the session ID if this is a [`Wrapped`](Self::Wrapped) output
    /// and the event has an associated session.
    ///
    /// Returns `None` for direct outputs or when the wrapped event has no
    /// session (e.g. providers without ordered-processing support).
    pub fn session_id(&self) -> Option<&SessionId> {
        match self {
            Self::Wrapped(event) => event.session_id.as_ref(),
            Self::Direct { .. } => None,
        }
    }

    /// Returns the event type string if this is a [`Wrapped`](Self::Wrapped) output.
    ///
    /// Returns `None` for direct outputs.
    pub fn event_type(&self) -> Option<&str> {
        match self {
            Self::Wrapped(event) => Some(&event.event_type),
            Self::Direct { .. } => None,
        }
    }

    /// Returns a reference to the inner [`WrappedEvent`] if this is a
    /// [`Wrapped`](Self::Wrapped) output.
    ///
    /// Returns `None` for direct outputs.
    pub fn as_wrapped(&self) -> Option<&WrappedEvent> {
        match self {
            Self::Wrapped(event) => Some(event),
            Self::Direct { .. } => None,
        }
    }
}

// ============================================================================
// DirectQueueMetadata
// ============================================================================

/// Lightweight metadata attached to directly-queued payloads.
///
/// When a provider operates in direct mode the original webhook body is
/// forwarded without transformation. This struct captures the minimum
/// information needed for observability, deduplication, and debugging.
///
/// # Auto-Generated Fields
///
/// Both `event_id` and `correlation_id` are generated automatically by
/// [`DirectQueueMetadata::new`] to ensure every message is traceable.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::DirectQueueMetadata;
///
/// let meta = DirectQueueMetadata::new("jira", "application/json");
/// assert_eq!(meta.provider_id(), "jira");
/// assert_eq!(meta.content_type(), "application/json");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectQueueMetadata {
    /// Unique event identifier for tracking and deduplication.
    event_id: EventId,

    /// Correlation identifier for distributed tracing.
    correlation_id: CorrelationId,

    /// UTC timestamp when the payload was received by Queue-Keeper.
    received_at: Timestamp,

    /// The provider ID that produced this output (e.g. `"jira"`, `"gitlab"`).
    provider_id: String,

    /// The `Content-Type` of the original request body.
    content_type: String,
}

impl DirectQueueMetadata {
    /// Create new metadata with auto-generated IDs and the current timestamp.
    ///
    /// # Arguments
    ///
    /// * `provider_id` - The provider that received the webhook (e.g. `"jira"`).
    /// * `content_type` - The `Content-Type` header of the original request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_core::webhook::DirectQueueMetadata;
    ///
    /// let meta = DirectQueueMetadata::new("gitlab", "application/json");
    /// assert!(!meta.event_id().as_str().is_empty());
    /// ```
    pub fn new(provider_id: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            event_id: EventId::new(),
            correlation_id: CorrelationId::new(),
            received_at: Timestamp::now(),
            provider_id: provider_id.into(),
            content_type: content_type.into(),
        }
    }

    /// The unique event ID generated for this payload.
    pub fn event_id(&self) -> EventId {
        self.event_id
    }

    /// The correlation ID generated for distributed tracing.
    pub fn correlation_id(&self) -> &CorrelationId {
        &self.correlation_id
    }

    /// The UTC time when the payload was received.
    pub fn received_at(&self) -> Timestamp {
        self.received_at
    }

    /// The provider that produced this output.
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// The `Content-Type` of the original request body.
    pub fn content_type(&self) -> &str {
        &self.content_type
    }
}

#[cfg(test)]
#[path = "processing_output_tests.rs"]
mod tests;
