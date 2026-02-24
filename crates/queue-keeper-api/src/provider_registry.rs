//! Provider registry for multi-provider webhook routing.
//!
//! This module provides [`ProviderRegistry`] for associating named providers
//! (e.g. `"github"`, `"jira"`) with their [`WebhookProcessor`] implementations.
//! The registry is built once at startup and used read-only during request handling.
//!
//! # URL Structure
//!
//! Each registered provider is reachable at:
//! ```text
//! POST /webhook/{provider_id}
//! ```
//!
//! For example, after registering `"github"` the endpoint becomes
//! `POST /webhook/github`.

use queue_keeper_core::webhook::WebhookProcessor;
use std::{collections::HashMap, sync::Arc};

// ============================================================================
// ProviderId
// ============================================================================

/// URL-safe identifier for a webhook provider.
///
/// A provider ID must consist entirely of lowercase ASCII letters, digits,
/// hyphens (`-`), or underscores (`_`). It must not be empty.
///
/// Provider IDs appear verbatim as URL path segments:
/// `POST /webhook/{provider_id}`
///
/// # Examples
///
/// ```rust
/// use queue_keeper_api::provider_registry::ProviderId;
///
/// let id = ProviderId::new("github").unwrap();
/// assert_eq!(id.as_str(), "github");
///
/// let id = ProviderId::new("my-cool-app").unwrap();
/// assert_eq!(id.as_str(), "my-cool-app");
///
/// assert!(ProviderId::new("GitHub").is_err()); // uppercase not allowed
/// assert!(ProviderId::new("").is_err());       // empty not allowed
/// assert!(ProviderId::new("../escape").is_err()); // slashes not allowed
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    /// Create a new `ProviderId`, validating it contains only URL-safe characters.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidProviderIdError::Empty`] if the value is empty.
    /// Returns [`InvalidProviderIdError::InvalidChars`] if the value contains
    /// characters outside `[a-z0-9\-_]`.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidProviderIdError> {
        let s = value.into();
        if s.is_empty() {
            return Err(InvalidProviderIdError::Empty);
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(InvalidProviderIdError::InvalidChars { value: s });
        }
        Ok(Self(s))
    }

    /// Return the provider ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ============================================================================
// InvalidProviderIdError
// ============================================================================

/// Error returned when a [`ProviderId`] cannot be created.
#[derive(Debug, thiserror::Error)]
pub enum InvalidProviderIdError {
    /// Provider ID must not be empty.
    #[error("Provider ID must not be empty")]
    Empty,

    /// Provider ID contains characters outside `[a-z0-9\\-_]`.
    #[error(
        "Provider ID '{value}' contains invalid characters; \
         use lowercase alphanumeric, hyphens, or underscores"
    )]
    InvalidChars { value: String },
}

// ============================================================================
// ProviderRegistry
// ============================================================================

/// Registry mapping provider IDs to their webhook processors.
///
/// Built once at service startup and used read-only during request handling.
/// All values are stored as `Arc<dyn WebhookProcessor>` to allow sharing
/// across async tasks and threads.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_api::provider_registry::{ProviderId, ProviderRegistry};
/// use std::sync::Arc;
///
/// let mut registry = ProviderRegistry::new();
/// // registry.register(ProviderId::new("github").unwrap(), Arc::new(processor));
/// assert!(!registry.contains("github")); // nothing registered yet
/// ```
#[derive(Clone)]
pub struct ProviderRegistry {
    processors: HashMap<String, Arc<dyn WebhookProcessor>>,
}

impl ProviderRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            processors: HashMap::new(),
        }
    }

    /// Register a provider with its webhook processor.
    ///
    /// If a provider with the same ID is already registered it is replaced.
    /// Returns `&mut Self` to allow method chaining.
    pub fn register(&mut self, id: ProviderId, processor: Arc<dyn WebhookProcessor>) -> &mut Self {
        self.processors.insert(id.0, processor);
        self
    }

    /// Look up a processor by provider name.
    ///
    /// Returns `None` if the provider is not registered.
    pub fn get(&self, provider: &str) -> Option<Arc<dyn WebhookProcessor>> {
        self.processors.get(provider).cloned()
    }

    /// Check whether a provider is registered.
    pub fn contains(&self, provider: &str) -> bool {
        self.processors.contains_key(provider)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "provider_registry_tests.rs"]
mod tests;
