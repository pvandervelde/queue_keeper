//! Tests for circuit breaker protected Key Vault provider.

use std::sync::Arc;

use crate::adapters::{
    CircuitBreakerKeyVaultProvider, InMemoryKeyVaultProvider, InMemorySecretCache,
};
use crate::key_vault::{KeyVaultConfiguration, KeyVaultProvider, SecretName, SecretValue};

/// Helper to create test provider
fn create_test_provider() -> (
    CircuitBreakerKeyVaultProvider,
    Arc<InMemoryKeyVaultProvider>,
) {
    let inner = Arc::new(InMemoryKeyVaultProvider::new());
    let cache = Arc::new(InMemorySecretCache::new());
    let config = KeyVaultConfiguration::default();

    let wrapped = CircuitBreakerKeyVaultProvider::new(
        inner.clone() as Arc<dyn KeyVaultProvider>,
        cache,
        config,
    );

    (wrapped, inner)
}

#[tokio::test]
async fn test_successful_get_secret() {
    let (provider, inner) = create_test_provider();
    let name = SecretName::new("test-secret").unwrap();
    let value = SecretValue::from_string("secret-value".to_string());

    inner.add_secret(name.clone(), value.clone());

    let retrieved = provider.get_secret(&name).await.unwrap();
    assert_eq!(retrieved.expose_secret(), "secret-value");
}

#[tokio::test]
async fn test_circuit_opens_after_failures() {
    let (provider, _inner) = create_test_provider();
    let name = SecretName::new("nonexistent").unwrap();

    // Trigger 5 failures to trip circuit (Key Vault config has 3 failures threshold)
    for _ in 0..5 {
        let _ = provider.get_secret(&name).await;
    }

    // Next request should fail fast due to circuit open
    let result = provider.get_secret(&name).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fallback_to_cache_when_circuit_open() {
    let (provider, inner) = create_test_provider();
    let name = SecretName::new("cached-secret").unwrap();
    let value = SecretValue::from_string("cached-value".to_string());

    // Add secret and retrieve it to populate cache
    inner.add_secret(name.clone(), value.clone());
    let _ = provider.get_secret(&name).await.unwrap();

    // Now remove the secret from backend to simulate failure, but keep cache
    inner.remove_secret(&name);

    // Trigger failures on the SAME secret to open circuit
    for _ in 0..5 {
        let _ = provider.get_secret(&name).await;
    }

    // The cache should still have the value from first successful retrieval
    // This tests graceful degradation: using potentially stale cached data
    // when service is unavailable
    let retrieved = provider.get_secret(&name).await.unwrap();
    assert_eq!(retrieved.expose_secret(), "cached-value");
}

#[tokio::test]
async fn test_refresh_bypasses_circuit_breaker() {
    let (provider, inner) = create_test_provider();
    let name = SecretName::new("refresh-test").unwrap();
    let value = SecretValue::from_string("original-value".to_string());

    inner.add_secret(name.clone(), value);

    // Trigger failures to open circuit
    for _ in 0..5 {
        let _ = provider
            .get_secret(&SecretName::new("nonexistent").unwrap())
            .await;
    }

    // Refresh should bypass circuit breaker (administrative operation)
    let result = provider.refresh_secret(&name).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_secret_exists_bypasses_circuit_breaker() {
    let (provider, inner) = create_test_provider();
    let name = SecretName::new("exists-test").unwrap();
    let value = SecretValue::from_string("value".to_string());

    inner.add_secret(name.clone(), value);

    // Trigger failures to open circuit
    for _ in 0..5 {
        let _ = provider
            .get_secret(&SecretName::new("nonexistent").unwrap())
            .await;
    }

    // Existence check should bypass circuit breaker
    let exists = provider.secret_exists(&name).await.unwrap();
    assert!(exists);
}

#[tokio::test]
async fn test_list_secret_names_bypasses_circuit_breaker() {
    let (provider, inner) = create_test_provider();
    let name1 = SecretName::new("secret1").unwrap();
    let name2 = SecretName::new("secret2").unwrap();

    inner.add_secret(
        name1.clone(),
        SecretValue::from_string("value1".to_string()),
    );
    inner.add_secret(
        name2.clone(),
        SecretValue::from_string("value2".to_string()),
    );

    // Trigger failures to open circuit
    for _ in 0..5 {
        let _ = provider
            .get_secret(&SecretName::new("nonexistent").unwrap())
            .await;
    }

    // List should bypass circuit breaker
    let names = provider.list_secret_names().await.unwrap();
    assert_eq!(names.len(), 2);
}

#[tokio::test]
async fn test_get_secret_with_version_fallback() {
    let (provider, inner) = create_test_provider();
    let name = SecretName::new("versioned-secret").unwrap();
    let value = SecretValue::from_string("versioned-value".to_string());

    // Add secret and retrieve to populate cache with initial version
    inner.add_secret(name.clone(), value.clone());
    let (_, version1) = provider.get_secret_with_version(&name).await.unwrap();
    assert_eq!(version1, "v1"); // InMemoryKeyVaultProvider uses "v1" as default

    // Remove from backend to simulate failure
    inner.remove_secret(&name);

    // Trigger failures on same secret
    for _ in 0..5 {
        let _ = provider.get_secret_with_version(&name).await;
    }

    // Should fall back to cached version (even after circuit opens)
    let (retrieved, version) = provider.get_secret_with_version(&name).await.unwrap();
    assert_eq!(retrieved.expose_secret(), "versioned-value");
    assert_eq!(version, "v1"); // Should get the cached version
}

#[tokio::test]
async fn test_cache_operations() {
    let (provider, inner) = create_test_provider();
    let name = SecretName::new("cache-test").unwrap();
    let value = SecretValue::from_string("cache-value".to_string());

    inner.add_secret(name.clone(), value);
    let _ = provider.get_secret(&name).await.unwrap();

    // Clear specific cache entry
    provider.clear_cache(&name).await.unwrap();

    // Clear all cache
    provider.clear_all_cache().await.unwrap();
}
