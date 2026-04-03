//! Tests for response types and health checker implementations.

use super::*;
use crate::provider_registry::{ProviderId, ProviderRegistry};
use async_trait::async_trait;
use queue_keeper_core::{
    webhook::{
        NormalizationError, ProcessingOutput, StorageError, StorageReference, ValidationStatus,
        WebhookError, WebhookRequest, WrappedEvent,
    },
    ValidationError,
};
use std::sync::Arc;

// ============================================================================
// Minimal mock WebhookProcessor for building a populated ProviderRegistry
// ============================================================================

struct NoopWebhookProcessor;

#[async_trait]
impl queue_keeper_core::webhook::WebhookProcessor for NoopWebhookProcessor {
    async fn process_webhook(
        &self,
        _request: WebhookRequest,
    ) -> Result<ProcessingOutput, WebhookError> {
        unimplemented!("not used in health checker tests")
    }

    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _event_type: &str,
    ) -> Result<(), ValidationError> {
        unimplemented!("not used in health checker tests")
    }

    async fn store_raw_payload(
        &self,
        _request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        unimplemented!("not used in health checker tests")
    }

    async fn normalize_event(
        &self,
        _request: &WebhookRequest,
    ) -> Result<WrappedEvent, NormalizationError> {
        unimplemented!("not used in health checker tests")
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn empty_registry() -> Arc<ProviderRegistry> {
    Arc::new(ProviderRegistry::new())
}

fn populated_registry() -> Arc<ProviderRegistry> {
    let mut registry = ProviderRegistry::new();
    registry.register(
        ProviderId::new("github").unwrap(),
        Arc::new(NoopWebhookProcessor),
    );
    Arc::new(registry)
}

// ============================================================================
// ServiceHealthChecker tests
// ============================================================================

mod service_health_checker_tests {
    use super::*;

    /// Verify that check_readiness() returns false when no providers are registered.
    #[tokio::test]
    async fn test_check_readiness_returns_false_for_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let ready = checker.check_readiness().await;

        assert!(!ready, "empty registry must not report ready");
    }

    /// Verify that check_readiness() returns true when at least one provider is registered.
    #[tokio::test]
    async fn test_check_readiness_returns_true_with_registered_provider() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let ready = checker.check_readiness().await;

        assert!(ready, "registry with 1+ providers must report ready");
    }

    /// Verify that check_basic_health() reports is_healthy = false when no providers
    /// are registered — the "providers" component must be unhealthy and the overall
    /// status must reflect that.
    #[tokio::test]
    async fn test_check_basic_health_is_unhealthy_for_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let status = checker.check_basic_health().await;

        assert!(
            !status.is_healthy,
            "basic health must be unhealthy when no providers are registered"
        );
        let providers_check = status
            .checks
            .get("providers")
            .expect("'providers' check must be present");
        assert!(
            !providers_check.healthy,
            "'providers' check must report unhealthy for empty registry"
        );
    }

    /// Verify that check_basic_health() reports is_healthy = true when at least one
    /// provider is registered.
    #[tokio::test]
    async fn test_check_basic_health_is_healthy_with_registered_provider() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let status = checker.check_basic_health().await;

        assert!(
            status.is_healthy,
            "basic health must be healthy when 1+ providers are registered"
        );
        let providers_check = status
            .checks
            .get("providers")
            .expect("'providers' check must be present");
        assert!(
            providers_check.healthy,
            "'providers' check must report healthy"
        );
    }

    /// Verify that check_deep_health() reports is_healthy = false when no providers
    /// are registered.
    #[tokio::test]
    async fn test_check_deep_health_is_unhealthy_for_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let status = checker.check_deep_health().await;

        assert!(
            !status.is_healthy,
            "deep health must be unhealthy when no providers are registered"
        );
        let providers_check = status
            .checks
            .get("providers")
            .expect("'providers' check must be present");
        assert!(
            !providers_check.healthy,
            "'providers' check must report unhealthy for empty registry"
        );
    }

    /// Verify that check_deep_health() reports is_healthy = true when at least one
    /// provider is registered.
    #[tokio::test]
    async fn test_check_deep_health_is_healthy_with_registered_provider() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let status = checker.check_deep_health().await;

        assert!(
            status.is_healthy,
            "deep health must be healthy when 1+ providers are registered"
        );
    }

    /// Verify that the "service" component is always reported healthy (process-level
    /// liveness), regardless of provider count.
    #[tokio::test]
    async fn test_check_basic_health_service_component_always_healthy() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let status = checker.check_basic_health().await;

        let service_check = status
            .checks
            .get("service")
            .expect("'service' check must be present");
        assert!(
            service_check.healthy,
            "'service' check must always be healthy while the process is running"
        );
    }

    /// Verify consistent is_healthy semantics between check_basic_health and
    /// check_deep_health: both must agree when providers are absent.
    #[tokio::test]
    async fn test_basic_and_deep_health_agree_on_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let basic = checker.check_basic_health().await;
        let deep = checker.check_deep_health().await;

        assert_eq!(
            basic.is_healthy, deep.is_healthy,
            "check_basic_health and check_deep_health must agree on is_healthy"
        );
    }

    /// Verify consistent is_healthy semantics between check_basic_health and
    /// check_deep_health: both must agree when providers are present.
    #[tokio::test]
    async fn test_basic_and_deep_health_agree_on_populated_registry() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let basic = checker.check_basic_health().await;
        let deep = checker.check_deep_health().await;

        assert_eq!(
            basic.is_healthy, deep.is_healthy,
            "check_basic_health and check_deep_health must agree on is_healthy"
        );
    }
}
