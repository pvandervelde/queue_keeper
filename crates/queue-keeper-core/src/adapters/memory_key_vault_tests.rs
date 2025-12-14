//! Tests for in-memory Key Vault implementation

use super::*;
use crate::key_vault::{SecretName, SecretValue};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_in_memory_cache_basic_operations() {
    let cache = InMemorySecretCache::new();
    let name = SecretName::new("test-secret").unwrap();
    let value = SecretValue::from_string("secret-value".to_string());

    // Initially empty
    assert!(cache.get(&name).await.is_none());

    // Store secret
    cache
        .put(name.clone(), value, Duration::from_secs(300))
        .await
        .unwrap();

    // Retrieve secret
    let cached = cache.get(&name).await.unwrap();
    assert_eq!(cached.value.expose_secret(), "secret-value");
    assert!(!cached.is_expired());

    // Remove secret
    cache.remove(&name).await.unwrap();
    assert!(cache.get(&name).await.is_none());
}

#[tokio::test]
async fn test_in_memory_provider_get_secret() {
    let provider = InMemoryKeyVaultProvider::new();
    let name = SecretName::new("test-secret").unwrap();
    let value = SecretValue::from_string("secret-value".to_string());

    // Initially not found
    assert!(provider.get_secret(&name).await.is_err());

    // Add secret
    provider.add_secret(name.clone(), value);

    // Retrieve secret
    let retrieved = provider.get_secret(&name).await.unwrap();
    assert_eq!(retrieved.expose_secret(), "secret-value");

    // Cached on second retrieval
    let cached = provider.get_secret(&name).await.unwrap();
    assert_eq!(cached.expose_secret(), "secret-value");
}

#[tokio::test]
async fn test_in_memory_provider_with_secrets() {
    let mut secrets = HashMap::new();
    let name1 = SecretName::new("secret-1").unwrap();
    let name2 = SecretName::new("secret-2").unwrap();
    secrets.insert(
        name1.clone(),
        SecretValue::from_string("value-1".to_string()),
    );
    secrets.insert(
        name2.clone(),
        SecretValue::from_string("value-2".to_string()),
    );

    let provider = InMemoryKeyVaultProvider::with_secrets(secrets);

    let val1 = provider.get_secret(&name1).await.unwrap();
    let val2 = provider.get_secret(&name2).await.unwrap();

    assert_eq!(val1.expose_secret(), "value-1");
    assert_eq!(val2.expose_secret(), "value-2");
}

#[tokio::test]
async fn test_secret_rotation() {
    let provider = InMemoryKeyVaultProvider::new();
    let name = SecretName::new("rotatable-secret").unwrap();
    let value1 = SecretValue::from_string("original-value".to_string());

    provider.add_secret(name.clone(), value1);

    // Get initial version
    let (val, version) = provider.get_secret_with_version(&name).await.unwrap();
    assert_eq!(val.expose_secret(), "original-value");
    assert_eq!(version, "v1");

    // Rotate secret
    let value2 = SecretValue::from_string("rotated-value".to_string());
    provider.rotate_secret(&name, value2, "v2".to_string());

    // Clear cache to force fresh fetch with new version
    provider.clear_cache(&name).await.unwrap();

    // Get new version after rotation
    let (val, new_version) = provider.get_secret_with_version(&name).await.unwrap();
    assert_eq!(val.expose_secret(), "rotated-value");
    assert_eq!(new_version, "v2");
}

#[tokio::test]
async fn test_cache_statistics() {
    let cache = InMemorySecretCache::new();
    let name = SecretName::new("test-secret").unwrap();
    let value = SecretValue::from_string("secret-value".to_string());

    // Trigger miss
    cache.get(&name).await;

    // Store and trigger hit
    cache
        .put(name.clone(), value, Duration::from_secs(300))
        .await
        .unwrap();
    cache.get(&name).await;

    let stats = cache.get_statistics().await.unwrap();
    assert_eq!(stats.total_hits, 1);
    assert_eq!(stats.total_misses, 1);
    assert_eq!(stats.cached_secrets_count, 1);
    assert!((stats.hit_ratio - 0.5).abs() < 0.01); // 1 hit, 1 miss = 50%
}

#[tokio::test]
async fn test_list_secret_names() {
    let mut secrets = HashMap::new();
    secrets.insert(
        SecretName::new("secret-1").unwrap(),
        SecretValue::from_string("value-1".to_string()),
    );
    secrets.insert(
        SecretName::new("secret-2").unwrap(),
        SecretValue::from_string("value-2".to_string()),
    );

    let provider = InMemoryKeyVaultProvider::with_secrets(secrets);
    let names = provider.list_secret_names().await.unwrap();

    assert_eq!(names.len(), 2);
    assert!(names.iter().any(|n| n.as_str() == "secret-1"));
    assert!(names.iter().any(|n| n.as_str() == "secret-2"));
}

#[tokio::test]
async fn test_secret_exists() {
    let provider = InMemoryKeyVaultProvider::new();
    let name = SecretName::new("existing-secret").unwrap();
    let value = SecretValue::from_string("value".to_string());

    assert!(!provider.secret_exists(&name).await.unwrap());

    provider.add_secret(name.clone(), value);

    assert!(provider.secret_exists(&name).await.unwrap());
}

#[tokio::test]
async fn test_cleanup_expired() {
    let cache = InMemorySecretCache::new();

    // Add secret with very short TTL
    let name = SecretName::new("expiring-secret").unwrap();
    let value = SecretValue::from_string("value".to_string());
    cache
        .put(name.clone(), value, Duration::from_millis(1))
        .await
        .unwrap();

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Cleanup should remove expired secret
    let removed = cache.cleanup_expired().await.unwrap();
    assert_eq!(removed, 1);

    // Secret should be gone
    assert!(cache.get(&name).await.is_none());
}
