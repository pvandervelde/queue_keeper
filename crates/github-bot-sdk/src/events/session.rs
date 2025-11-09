//! Session management for ordered event processing.

use super::processor::SessionIdStrategy;
use super::{EntityType, EventEnvelope, EventPayload};

/// Manages session IDs for ordered event processing.
///
/// The session manager provides different strategies for grouping related
/// events to ensure they are processed in order. This is critical for
/// maintaining consistency when handling sequences of related events.
///
/// # Examples
///
/// ```rust
/// use github_bot_sdk::events::{SessionManager, SessionIdStrategy};
///
/// let manager = SessionManager::new(SessionIdStrategy::Entity);
/// // Use with event processing...
/// ```
pub struct SessionManager {
    strategy: SessionIdStrategy,
}

impl SessionManager {
    /// Create a new session manager with the given strategy.
    pub fn new(strategy: SessionIdStrategy) -> Self {
        Self { strategy }
    }

    /// Generate a session ID for an event envelope.
    ///
    /// Returns `None` if the strategy is `SessionIdStrategy::None` or if
    /// the event doesn't support session-based ordering.
    pub fn generate_session_id(&self, envelope: &EventEnvelope) -> Option<String> {
        todo!("Implement SessionManager::generate_session_id")
    }

    /// Extract an ordering key from an event envelope.
    ///
    /// The ordering key is used by queue systems to ensure events with
    /// the same key are processed in order.
    pub fn extract_ordering_key(&self, envelope: &EventEnvelope) -> Option<String> {
        todo!("Implement SessionManager::extract_ordering_key")
    }

    /// Get an entity-based session strategy.
    ///
    /// Creates session IDs in the format: `{entity_type}-{repo}-{entity_id}`
    /// For example: "pr-owner/repo-123" or "issue-owner/repo-456"
    pub fn entity_session_strategy() -> SessionIdStrategy {
        SessionIdStrategy::Custom(
            |envelope| match (&envelope.entity_type, &envelope.entity_id) {
                (EntityType::PullRequest, Some(id)) => {
                    Some(format!("pr-{}-{}", envelope.repository.full_name, id))
                }
                (EntityType::Issue, Some(id)) => {
                    Some(format!("issue-{}-{}", envelope.repository.full_name, id))
                }
                (EntityType::Branch, Some(id)) => {
                    Some(format!("branch-{}-{}", envelope.repository.full_name, id))
                }
                _ => None,
            },
        )
    }

    /// Get a repository-based session strategy.
    ///
    /// All events for a repository share the same session ID.
    /// Format: `repo-{owner}/{name}`
    pub fn repository_session_strategy() -> SessionIdStrategy {
        SessionIdStrategy::Custom(|envelope| {
            Some(format!("repo-{}", envelope.repository.full_name))
        })
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
