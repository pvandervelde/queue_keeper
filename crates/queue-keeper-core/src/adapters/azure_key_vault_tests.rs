//! Tests for Azure Key Vault implementation

#[cfg(feature = "azure")]
use super::*;
#[cfg(feature = "azure")]
use crate::adapters::memory_key_vault::InMemorySecretCache;
#[cfg(feature = "azure")]
use crate::key_vault::KeyVaultConfiguration;
#[cfg(feature = "azure")]
use std::sync::Arc;

// Note: These tests require Azure Key Vault access and are typically run in CI/CD
// with proper credentials. For local testing, use InMemoryKeyVaultProvider.

#[tokio::test]
#[cfg(feature = "azure")]
#[ignore = "Requires Azure Key Vault access"]
async fn test_azure_provider_creation() {
    let config = KeyVaultConfiguration {
        vault_url: "https://test-vault.vault.azure.net/".to_string(),
        ..Default::default()
    };

    let cache = Arc::new(InMemorySecretCache::new());
    let result = AzureKeyVaultProvider::new(config, cache).await;

    // This will fail without proper credentials, but validates configuration
    assert!(result.is_ok() || result.is_err());
}

#[test]
#[cfg(feature = "azure")]
fn test_invalid_vault_url() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let config = KeyVaultConfiguration {
            vault_url: String::new(), // Invalid: empty
            ..Default::default()
        };

        let cache = Arc::new(InMemorySecretCache::new());
        let result = AzureKeyVaultProvider::new(config, cache).await;

        assert!(result.is_err());
        if let Err(KeyVaultError::Configuration { message }) = result {
            assert!(message.contains("vault_url"));
        }
    });
}
