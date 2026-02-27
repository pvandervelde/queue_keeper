//! Tests for [`ProviderId`] and [`ProviderRegistry`].

use super::*;
use async_trait::async_trait;
use queue_keeper_core::{
    webhook::{
        EventEntity, EventEnvelope, NormalizationError, StorageError, StorageReference,
        ValidationStatus, WebhookError, WebhookRequest,
    },
    Repository, RepositoryId, Timestamp, User, UserId, UserType, ValidationError,
};
use std::sync::Arc;

// ============================================================================
// Minimal mock WebhookProcessor
// ============================================================================

struct NoopWebhookProcessor;

#[async_trait]
impl WebhookProcessor for NoopWebhookProcessor {
    async fn process_webhook(
        &self,
        _request: WebhookRequest,
    ) -> Result<EventEnvelope, WebhookError> {
        Ok(test_event_envelope())
    }

    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _event_type: &str,
    ) -> Result<(), ValidationError> {
        Ok(())
    }

    async fn store_raw_payload(
        &self,
        _request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        Ok(StorageReference {
            blob_path: "test/path.json".to_string(),
            stored_at: Timestamp::now(),
            size_bytes: 0,
        })
    }

    async fn normalize_event(
        &self,
        _request: &WebhookRequest,
    ) -> Result<EventEnvelope, NormalizationError> {
        Ok(test_event_envelope())
    }
}

fn test_event_envelope() -> EventEnvelope {
    let user = User {
        id: UserId::new(1),
        login: "owner".to_string(),
        user_type: UserType::User,
    };
    let repo = Repository::new(
        RepositoryId::new(1),
        "repo".to_string(),
        "owner/repo".to_string(),
        user,
        false,
    );
    EventEnvelope::new(
        "ping".to_string(),
        None,
        repo,
        EventEntity::Repository,
        serde_json::json!({}),
    )
}

// ============================================================================
// ProviderId tests
// ============================================================================

mod provider_id_tests {
    use super::*;

    /// Verify that a simple lowercase name is accepted.
    #[test]
    fn test_valid_simple_name() {
        let id = ProviderId::new("github").unwrap();
        assert_eq!(id.as_str(), "github");
    }

    /// Verify that names with hyphens and underscores are accepted.
    #[test]
    fn test_valid_name_with_separators() {
        let id = ProviderId::new("my-cool_app").unwrap();
        assert_eq!(id.as_str(), "my-cool_app");
    }

    /// Verify that names with digits are accepted.
    #[test]
    fn test_valid_name_with_digits() {
        let id = ProviderId::new("provider2").unwrap();
        assert_eq!(id.as_str(), "provider2");
    }

    /// Verify that an empty string is rejected.
    #[test]
    fn test_empty_name_rejected() {
        assert!(matches!(
            ProviderId::new(""),
            Err(InvalidProviderIdError::Empty)
        ));
    }

    /// Verify that uppercase letters are rejected.
    #[test]
    fn test_uppercase_rejected() {
        assert!(matches!(
            ProviderId::new("GitHub"),
            Err(InvalidProviderIdError::InvalidChars { .. })
        ));
    }

    /// Verify that path traversal characters (slash) are rejected.
    #[test]
    fn test_slash_rejected() {
        assert!(matches!(
            ProviderId::new("../escape"),
            Err(InvalidProviderIdError::InvalidChars { .. })
        ));
    }

    /// Verify that spaces are rejected.
    #[test]
    fn test_space_rejected() {
        assert!(matches!(
            ProviderId::new("my provider"),
            Err(InvalidProviderIdError::InvalidChars { .. })
        ));
    }

    /// Verify Display formatting matches the inner string.
    #[test]
    fn test_display() {
        let id = ProviderId::new("github").unwrap();
        assert_eq!(id.to_string(), "github");
    }
}

// ============================================================================
// ProviderRegistry tests
// ============================================================================

mod provider_registry_tests {
    use super::*;

    /// Verify that a newly created registry contains no providers.
    #[test]
    fn test_new_registry_is_empty() {
        let registry = ProviderRegistry::new();
        assert!(!registry.contains("github"));
    }

    /// Verify that `get` returns `None` for an unregistered provider.
    #[test]
    fn test_get_unregistered_returns_none() {
        let registry = ProviderRegistry::new();
        assert!(registry.get("github").is_none());
    }

    /// Verify that a registered provider can be retrieved by name.
    #[test]
    fn test_register_then_get() {
        let mut registry = ProviderRegistry::new();
        registry.register(
            ProviderId::new("github").unwrap(),
            Arc::new(NoopWebhookProcessor),
        );
        assert!(registry.get("github").is_some());
        assert!(registry.contains("github"));
    }

    /// Verify that registering a second provider under the same id replaces the first.
    #[test]
    fn test_register_replaces_existing() {
        let mut registry = ProviderRegistry::new();
        let processor1: Arc<dyn WebhookProcessor> = Arc::new(NoopWebhookProcessor);
        let processor2: Arc<dyn WebhookProcessor> = Arc::new(NoopWebhookProcessor);

        registry.register(ProviderId::new("github").unwrap(), processor1);
        registry.register(ProviderId::new("github").unwrap(), processor2.clone());

        // The second registration should replace the first
        let retrieved = registry.get("github").unwrap();
        // Both are NoopWebhookProcessor so we just verify one is returned
        assert!(Arc::ptr_eq(&retrieved, &processor2));
    }

    /// Verify that looking up a different provider does not return the registered one.
    #[test]
    fn test_different_provider_lookup_returns_none() {
        let mut registry = ProviderRegistry::new();
        registry.register(
            ProviderId::new("github").unwrap(),
            Arc::new(NoopWebhookProcessor),
        );
        assert!(registry.get("gitlab").is_none());
    }

    /// Verify that multiple providers can be registered and retrieved independently.
    #[test]
    fn test_multiple_providers_independent() {
        let mut registry = ProviderRegistry::new();
        registry.register(
            ProviderId::new("github").unwrap(),
            Arc::new(NoopWebhookProcessor),
        );
        registry.register(
            ProviderId::new("jira").unwrap(),
            Arc::new(NoopWebhookProcessor),
        );
        assert!(registry.contains("github"));
        assert!(registry.contains("jira"));
        assert!(!registry.contains("gitlab"));
    }

    /// Verify that the default constructor produces an empty registry.
    #[test]
    fn test_default_is_empty() {
        let registry = ProviderRegistry::default();
        assert!(!registry.contains("github"));
    }

    /// Verify that a cloned registry shares the same provider entries.
    #[test]
    fn test_clone_shares_entries() {
        let mut registry = ProviderRegistry::new();
        registry.register(
            ProviderId::new("github").unwrap(),
            Arc::new(NoopWebhookProcessor),
        );
        let cloned = registry.clone();
        assert!(cloned.contains("github"));
    }
}
