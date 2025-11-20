//! Session management for ordered message processing.
//!
//! This module provides a generic framework for session key generation that enables
//! ordered message processing for any domain. Session keys group related messages
//! to ensure they are processed in FIFO order.
//!
//! # Design Philosophy
//!
//! This module is intentionally **domain-agnostic**. It provides the infrastructure
//! for session-based ordering without assuming any specific message structure or
//! business domain (GitHub events, e-commerce orders, IoT telemetry, etc.).
//!
//! # Core Concepts
//!
//! - **SessionKeyGenerator**: Trait for extracting session keys from messages
//! - **Session Keys**: Strings that group related messages for ordered processing
//! - **Message Metadata**: Messages provide metadata via the `SessionKeyExtractor` trait
//!
//! # Usage Pattern
//!
//! 1. Implement `SessionKeyExtractor` for your message type
//! 2. Implement `SessionKeyGenerator` for your domain-specific strategy
//! 3. Use the generator to produce session IDs when sending messages to queues
//!
//! # Example
//!
//! ```rust
//! use queue_runtime::sessions::{SessionKeyGenerator, SessionKeyExtractor};
//! use queue_runtime::message::SessionId;
//! use std::collections::HashMap;
//!
//! // Your domain message type
//! struct OrderEvent {
//!     order_id: String,
//!     customer_id: String,
//! }
//!
//! // Implement metadata extraction
//! impl SessionKeyExtractor for OrderEvent {
//!     fn get_metadata(&self, key: &str) -> Option<String> {
//!         match key {
//!             "order_id" => Some(self.order_id.clone()),
//!             "customer_id" => Some(self.customer_id.clone()),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! // Implement your session strategy
//! struct OrderSessionStrategy;
//!
//! impl SessionKeyGenerator for OrderSessionStrategy {
//!     fn generate_key(&self, extractor: &dyn SessionKeyExtractor) -> Option<SessionId> {
//!         extractor.get_metadata("order_id")
//!             .and_then(|id| SessionId::new(format!("order-{}", id)).ok())
//!     }
//! }
//! ```

use crate::message::SessionId;
use std::collections::HashMap;

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;

// ============================================================================
// Session Key Extractor Trait
// ============================================================================

/// Trait for extracting metadata from messages for session key generation.
///
/// This trait provides a completely generic interface for messages to expose
/// metadata that can be used to generate session keys. It makes no assumptions
/// about the message structure or domain.
///
/// # Design
///
/// The trait uses a key-value interface where messages expose named metadata
/// fields. Session key generators query for the metadata they need.
///
/// # Example
///
/// ```rust
/// use queue_runtime::sessions::SessionKeyExtractor;
///
/// struct MyMessage {
///     user_id: String,
///     resource_id: String,
/// }
///
/// impl SessionKeyExtractor for MyMessage {
///     fn get_metadata(&self, key: &str) -> Option<String> {
///         match key {
///             "user_id" => Some(self.user_id.clone()),
///             "resource_id" => Some(self.resource_id.clone()),
///             _ => None,
///         }
///     }
///
///     fn list_metadata_keys(&self) -> Vec<String> {
///         vec!["user_id".to_string(), "resource_id".to_string()]
///     }
/// }
/// ```
pub trait SessionKeyExtractor {
    /// Get a metadata value by key.
    ///
    /// Returns `None` if the key doesn't exist or has no value for this message.
    ///
    /// # Arguments
    ///
    /// * `key` - The metadata key to retrieve
    ///
    /// # Returns
    ///
    /// Optional string value for the requested key
    fn get_metadata(&self, key: &str) -> Option<String>;

    /// List all available metadata keys for this message.
    ///
    /// This is useful for debugging and introspection. Default implementation
    /// returns an empty list.
    ///
    /// # Returns
    ///
    /// Vector of available metadata key names
    fn list_metadata_keys(&self) -> Vec<String> {
        Vec::new()
    }

    /// Get all metadata as a map (optional, for bulk operations).
    ///
    /// Default implementation iterates over `list_metadata_keys()` and calls
    /// `get_metadata()` for each key.
    ///
    /// # Returns
    ///
    /// HashMap of all available metadata
    fn get_all_metadata(&self) -> HashMap<String, String> {
        self.list_metadata_keys()
            .into_iter()
            .filter_map(|key| self.get_metadata(&key).map(|value| (key, value)))
            .collect()
    }
}

// ============================================================================
// Session Key Generator Trait
// ============================================================================

/// Strategy trait for generating session keys from messages.
///
/// Implementations define how messages are grouped for ordered processing.
/// The generator extracts relevant metadata from messages and produces
/// session keys that group related messages together.
///
/// # Design Principles
///
/// - **Domain-Agnostic**: Works with any message structure via `SessionKeyExtractor`
/// - **Strategy Pattern**: Different strategies provide different ordering semantics
/// - **Composable**: Strategies can be combined or chained
/// - **Optional Ordering**: Returning `None` allows concurrent processing
///
/// # Common Patterns
///
/// - **Entity-based**: Group by entity ID (order-123, user-456)
/// - **Hierarchical**: Group by parent/child relationships
/// - **Temporal**: Group by time windows
/// - **Custom**: Domain-specific grouping logic
///
/// # Example
///
/// ```rust
/// use queue_runtime::sessions::{SessionKeyGenerator, SessionKeyExtractor};
/// use queue_runtime::message::SessionId;
///
/// struct ResourceIdStrategy;
///
/// impl SessionKeyGenerator for ResourceIdStrategy {
///     fn generate_key(&self, extractor: &dyn SessionKeyExtractor) -> Option<SessionId> {
///         extractor.get_metadata("resource_id")
///             .and_then(|id| SessionId::new(format!("resource-{}", id)).ok())
///     }
/// }
/// ```
pub trait SessionKeyGenerator: Send + Sync {
    /// Generate a session key for the given message.
    ///
    /// Returns `None` if the message should not be session-ordered, allowing
    /// it to be processed concurrently without ordering constraints.
    ///
    /// # Arguments
    ///
    /// * `extractor` - Message implementing SessionKeyExtractor trait
    ///
    /// # Returns
    ///
    /// Optional session ID for grouping related messages
    fn generate_key(&self, extractor: &dyn SessionKeyExtractor) -> Option<SessionId>;
}

// ============================================================================
// Composite Key Strategy
// ============================================================================

/// Generates session keys by composing multiple metadata fields.
///
/// This strategy builds session keys from a list of metadata fields in order,
/// joining them with a separator. This is useful for creating hierarchical
/// or compound session keys.
///
/// # Example
///
/// ```rust
/// use queue_runtime::sessions::CompositeKeyStrategy;
///
/// // Create session keys like "tenant-123-resource-456"
/// let strategy = CompositeKeyStrategy::new(vec![
///     "tenant_id".to_string(),
///     "resource_id".to_string(),
/// ], "-");
/// ```
pub struct CompositeKeyStrategy {
    fields: Vec<String>,
    separator: String,
}

impl CompositeKeyStrategy {
    /// Create a new composite key strategy.
    ///
    /// # Arguments
    ///
    /// * `fields` - Ordered list of metadata field names to compose
    /// * `separator` - String to join field values with
    ///
    /// # Example
    ///
    /// ```rust
    /// use queue_runtime::sessions::CompositeKeyStrategy;
    ///
    /// let strategy = CompositeKeyStrategy::new(
    ///     vec!["region".to_string(), "customer_id".to_string()],
    ///     "-"
    /// );
    /// ```
    pub fn new(fields: Vec<String>, separator: &str) -> Self {
        Self {
            fields,
            separator: separator.to_string(),
        }
    }
}

impl SessionKeyGenerator for CompositeKeyStrategy {
    fn generate_key(&self, extractor: &dyn SessionKeyExtractor) -> Option<SessionId> {
        // Return None if no fields specified
        if self.fields.is_empty() {
            return None;
        }

        // Collect all field values
        let values: Vec<String> = self
            .fields
            .iter()
            .filter_map(|field| extractor.get_metadata(field))
            .collect();

        // Return None if any required field is missing
        if values.len() != self.fields.len() {
            return None;
        }

        // Join values with separator
        let key = values.join(&self.separator);

        // Create session ID
        SessionId::new(key).ok()
    }
}

// ============================================================================
// Single Field Strategy
// ============================================================================

/// Generates session keys from a single metadata field.
///
/// This is the simplest strategy: extract one field and use it as the session key.
/// Optionally adds a prefix for namespacing.
///
/// # Example
///
/// ```rust
/// use queue_runtime::sessions::SingleFieldStrategy;
///
/// // Create session keys from "user_id" like "user-12345"
/// let strategy = SingleFieldStrategy::new("user_id", Some("user"));
/// ```
pub struct SingleFieldStrategy {
    field_name: String,
    prefix: Option<String>,
}

impl SingleFieldStrategy {
    /// Create a new single field strategy.
    ///
    /// # Arguments
    ///
    /// * `field_name` - The metadata field to use for the session key
    /// * `prefix` - Optional prefix to add before the field value
    ///
    /// # Example
    ///
    /// ```rust
    /// use queue_runtime::sessions::SingleFieldStrategy;
    ///
    /// let strategy = SingleFieldStrategy::new("order_id", Some("order"));
    /// // Produces keys like "order-123"
    /// ```
    pub fn new(field_name: &str, prefix: Option<&str>) -> Self {
        Self {
            field_name: field_name.to_string(),
            prefix: prefix.map(|s| s.to_string()),
        }
    }
}

impl SessionKeyGenerator for SingleFieldStrategy {
    fn generate_key(&self, extractor: &dyn SessionKeyExtractor) -> Option<SessionId> {
        // Get the field value
        let value = extractor.get_metadata(&self.field_name)?;

        // Build key with optional prefix
        let key = if let Some(ref prefix) = self.prefix {
            format!("{}-{}", prefix, value)
        } else {
            value
        };

        // Create session ID
        SessionId::new(key).ok()
    }
}

// ============================================================================
// No Ordering Strategy
// ============================================================================

/// Strategy that disables session-based ordering.
///
/// Always returns `None`, allowing concurrent message processing without
/// ordering guarantees. Use for stateless operations that don't require
/// message ordering.
///
/// # Example
///
/// ```rust
/// use queue_runtime::sessions::NoOrderingStrategy;
///
/// let strategy = NoOrderingStrategy;
/// // All messages can be processed concurrently
/// ```
pub struct NoOrderingStrategy;

impl SessionKeyGenerator for NoOrderingStrategy {
    fn generate_key(&self, _extractor: &dyn SessionKeyExtractor) -> Option<SessionId> {
        None
    }
}

// ============================================================================
// Fallback Strategy
// ============================================================================

/// Strategy that tries multiple generators in order, using the first success.
///
/// This implements a fallback chain: try the primary strategy first, and if it
/// returns `None`, try the next strategy, and so on. Useful for providing
/// fine-grained ordering when possible, with coarser fallbacks.
///
/// # Example
///
/// ```rust
/// use queue_runtime::sessions::{FallbackStrategy, SingleFieldStrategy, CompositeKeyStrategy};
///
/// // Try specific entity ID first, fall back to tenant-level ordering
/// let primary = SingleFieldStrategy::new("entity_id", Some("entity"));
/// let fallback = SingleFieldStrategy::new("tenant_id", Some("tenant"));
///
/// let strategy = FallbackStrategy::new(vec![
///     Box::new(primary),
///     Box::new(fallback),
/// ]);
/// ```
pub struct FallbackStrategy {
    strategies: Vec<Box<dyn SessionKeyGenerator>>,
}

impl FallbackStrategy {
    /// Create a new fallback strategy with ordered generators.
    ///
    /// # Arguments
    ///
    /// * `strategies` - Ordered list of generators to try
    ///
    /// # Example
    ///
    /// ```rust
    /// use queue_runtime::sessions::{FallbackStrategy, SingleFieldStrategy, NoOrderingStrategy};
    ///
    /// let strategy = FallbackStrategy::new(vec![
    ///     Box::new(SingleFieldStrategy::new("user_id", Some("user"))),
    ///     Box::new(NoOrderingStrategy), // Ultimate fallback: no ordering
    /// ]);
    /// ```
    pub fn new(strategies: Vec<Box<dyn SessionKeyGenerator>>) -> Self {
        Self { strategies }
    }
}

impl SessionKeyGenerator for FallbackStrategy {
    fn generate_key(&self, extractor: &dyn SessionKeyExtractor) -> Option<SessionId> {
        // Try each strategy in order until one succeeds
        for strategy in &self.strategies {
            if let Some(session_id) = strategy.generate_key(extractor) {
                return Some(session_id);
            }
        }

        // All strategies failed
        None
    }
}
