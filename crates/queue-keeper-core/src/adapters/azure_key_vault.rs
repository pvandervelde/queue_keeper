//! # Azure Key Vault Implementation
//!
//! Production Azure Key Vault integration using Azure SDK.
//! Provides secure secret management with managed identity authentication.

#[cfg(feature = "azure")]
use crate::key_vault::{
    CacheStatistics, KeyVaultConfiguration, KeyVaultError, KeyVaultProvider, SecretCache,
    SecretName, SecretValue,
};
#[cfg(feature = "azure")]
use async_trait::async_trait;
#[cfg(feature = "azure")]
use azure_core::auth::TokenCredential;
#[cfg(feature = "azure")]
use azure_identity::DefaultAzureCredential;
#[cfg(feature = "azure")]
use azure_security_keyvault::SecretClient;
#[cfg(feature = "azure")]
use futures::StreamExt;
#[cfg(feature = "azure")]
use std::sync::Arc;
#[cfg(feature = "azure")]
use tracing::{debug, error, info, instrument, warn};

/// Azure Key Vault provider with managed identity authentication
///
/// Uses DefaultAzureCredential for authentication, supporting:
/// - Managed Identity (production in Azure)
/// - Azure CLI (local development)
/// - Environment variables
/// - Visual Studio Code
#[cfg(feature = "azure")]
pub struct AzureKeyVaultProvider {
    client: SecretClient,
    config: KeyVaultConfiguration,
    cache: Arc<dyn SecretCache>,
}

#[cfg(feature = "azure")]
impl AzureKeyVaultProvider {
    /// Create new Azure Key Vault provider
    ///
    /// # Arguments
    /// - `config`: Key Vault configuration with vault URL
    /// - `cache`: Secret cache implementation
    ///
    /// # Errors
    /// Returns error if vault URL is invalid or authentication fails
    #[instrument(skip(cache))]
    pub async fn new(
        config: KeyVaultConfiguration,
        cache: Arc<dyn SecretCache>,
    ) -> Result<Self, KeyVaultError> {
        if config.vault_url.is_empty() {
            return Err(KeyVaultError::Configuration {
                message: "vault_url is required".to_string(),
            });
        }

        info!(vault_url = %config.vault_url, "Initializing Azure Key Vault provider");

        // Create credential using DefaultAzureCredential
        let credential = Arc::new(DefaultAzureCredential::default());

        // Create Key Vault client
        let client = SecretClient::new(&config.vault_url, credential).map_err(|e| {
            KeyVaultError::Configuration {
                message: format!("Failed to create Key Vault client: {}", e),
            }
        })?;

        Ok(Self {
            client,
            config,
            cache,
        })
    }

    /// Create provider with custom credential
    ///
    /// Useful for testing or custom authentication scenarios
    #[instrument(skip(credential, cache))]
    pub fn with_credential(
        config: KeyVaultConfiguration,
        credential: Arc<dyn TokenCredential>,
        cache: Arc<dyn SecretCache>,
    ) -> Result<Self, KeyVaultError> {
        if config.vault_url.is_empty() {
            return Err(KeyVaultError::Configuration {
                message: "vault_url is required".to_string(),
            });
        }

        let client = SecretClient::new(&config.vault_url, credential).map_err(|e| {
            KeyVaultError::Configuration {
                message: format!("Failed to create Key Vault client: {}", e),
            }
        })?;

        Ok(Self {
            client,
            config,
            cache,
        })
    }

    /// Fetch secret from Azure Key Vault (bypass cache)
    #[instrument(skip(self))]
    async fn fetch_from_vault(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        debug!(secret_name = %name, "Fetching secret from Azure Key Vault");

        let result = self.client.get(name.as_str()).await;

        match result {
            Ok(secret) => {
                let value = secret.value().ok_or_else(|| KeyVaultError::Internal {
                    message: "Secret has no value".to_string(),
                })?;

                info!(secret_name = %name, "Successfully retrieved secret from Key Vault");
                Ok(SecretValue::from_string(value.to_string()))
            }
            Err(e) => {
                let error_string = e.to_string();
                error!(secret_name = %name, error = %error_string, "Failed to retrieve secret from Key Vault");

                // Map Azure errors to KeyVaultError
                if error_string.contains("404") || error_string.contains("NotFound") {
                    Err(KeyVaultError::SecretNotFound { name: name.clone() })
                } else if error_string.contains("403")
                    || error_string.contains("Forbidden")
                    || error_string.contains("Unauthorized")
                {
                    Err(KeyVaultError::AccessDenied {
                        name: name.clone(),
                        reason: error_string,
                    })
                } else if error_string.contains("timeout")
                    || error_string.contains("Timeout")
                    || error_string.contains("deadline")
                {
                    Err(KeyVaultError::Timeout {
                        timeout_seconds: self.config.request_timeout_seconds,
                    })
                } else if error_string.contains("429")
                    || error_string.contains("TooManyRequests")
                    || error_string.contains("throttl")
                {
                    Err(KeyVaultError::RateLimitExceeded {
                        retry_after_seconds: 60, // Azure typically uses 60s
                    })
                } else if error_string.contains("503")
                    || error_string.contains("ServiceUnavailable")
                    || error_string.contains("unavailable")
                {
                    Err(KeyVaultError::ServiceUnavailable {
                        message: error_string,
                    })
                } else {
                    Err(KeyVaultError::Internal {
                        message: error_string,
                    })
                }
            }
        }
    }

    /// Fetch secret with version from Azure Key Vault
    #[instrument(skip(self))]
    async fn fetch_with_version(
        &self,
        name: &SecretName,
    ) -> Result<(SecretValue, String), KeyVaultError> {
        debug!(secret_name = %name, "Fetching secret with version from Azure Key Vault");

        let result = self.client.get(name.as_str()).await;

        match result {
            Ok(secret) => {
                let value = secret.value().ok_or_else(|| KeyVaultError::Internal {
                    message: "Secret has no value".to_string(),
                })?;

                let version = secret
                    .properties()
                    .version()
                    .unwrap_or("unknown")
                    .to_string();

                info!(secret_name = %name, version = %version, "Successfully retrieved secret with version");

                Ok((SecretValue::from_string(value.to_string()), version))
            }
            Err(e) => {
                error!(secret_name = %name, error = %e, "Failed to retrieve secret with version");
                Err(self.map_azure_error(name, e))
            }
        }
    }

    /// Map Azure SDK error to KeyVaultError
    fn map_azure_error(&self, name: &SecretName, error: azure_core::Error) -> KeyVaultError {
        let error_string = error.to_string();

        if error_string.contains("404") || error_string.contains("NotFound") {
            KeyVaultError::SecretNotFound { name: name.clone() }
        } else if error_string.contains("403")
            || error_string.contains("Forbidden")
            || error_string.contains("Unauthorized")
        {
            KeyVaultError::AccessDenied {
                name: name.clone(),
                reason: error_string,
            }
        } else if error_string.contains("timeout")
            || error_string.contains("Timeout")
            || error_string.contains("deadline")
        {
            KeyVaultError::Timeout {
                timeout_seconds: self.config.request_timeout_seconds,
            }
        } else if error_string.contains("429")
            || error_string.contains("TooManyRequests")
            || error_string.contains("throttl")
        {
            KeyVaultError::RateLimitExceeded {
                retry_after_seconds: 60,
            }
        } else if error_string.contains("503")
            || error_string.contains("ServiceUnavailable")
            || error_string.contains("unavailable")
        {
            KeyVaultError::ServiceUnavailable {
                message: error_string,
            }
        } else {
            KeyVaultError::Internal {
                message: error_string,
            }
        }
    }
}

#[cfg(feature = "azure")]
#[async_trait]
impl KeyVaultProvider for AzureKeyVaultProvider {
    #[instrument(skip(self))]
    async fn get_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        // Check cache first
        if let Some(cached) = self.cache.get(name).await {
            if !cached.is_expired() {
                debug!(secret_name = %name, "Cache hit for secret");
                return Ok(cached.value);
            } else {
                debug!(secret_name = %name, "Cache expired for secret");
            }
        } else {
            debug!(secret_name = %name, "Cache miss for secret");
        }

        // Fetch from Azure Key Vault
        let value = self.fetch_from_vault(name).await?;

        // Update cache
        let ttl = std::time::Duration::from_secs(self.config.cache_ttl_seconds);
        self.cache
            .put(name.clone(), value.clone(), ttl)
            .await
            .map_err(|e| {
                warn!(secret_name = %name, error = %e, "Failed to cache secret");
                e
            })?;

        Ok(value)
    }

    #[instrument(skip(self))]
    async fn get_secret_with_version(
        &self,
        name: &SecretName,
    ) -> Result<(SecretValue, String), KeyVaultError> {
        // Check cache for version
        if let Some(cached) = self.cache.get(name).await {
            if !cached.is_expired() {
                if let Some(version) = cached.version {
                    debug!(secret_name = %name, version = %version, "Cache hit with version");
                    return Ok((cached.value, version));
                }
            }
        }

        // Fetch from Azure Key Vault with version
        let (value, version) = self.fetch_with_version(name).await?;

        // Update cache with version
        let ttl = std::time::Duration::from_secs(self.config.cache_ttl_seconds);
        self.cache
            .put_with_version(name.clone(), value.clone(), version.clone(), ttl)
            .await
            .map_err(|e| {
                warn!(secret_name = %name, error = %e, "Failed to cache secret with version");
                e
            })?;

        Ok((value, version))
    }

    #[instrument(skip(self))]
    async fn refresh_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        // Clear cache entry to force fresh fetch
        self.cache.remove(name).await?;

        // Fetch fresh value
        self.get_secret(name).await
    }

    #[instrument(skip(self))]
    async fn secret_exists(&self, name: &SecretName) -> Result<bool, KeyVaultError> {
        debug!(secret_name = %name, "Checking if secret exists");

        match self.client.get(name.as_str()).await {
            Ok(_) => Ok(true),
            Err(e) => {
                let error_string = e.to_string();
                if error_string.contains("404") || error_string.contains("NotFound") {
                    Ok(false)
                } else {
                    Err(self.map_azure_error(name, e))
                }
            }
        }
    }

    #[instrument(skip(self))]
    async fn list_secret_names(&self) -> Result<Vec<SecretName>, KeyVaultError> {
        debug!("Listing secret names from Azure Key Vault");

        let mut names = Vec::new();

        // Azure SDK uses streams for listing
        let mut stream = self.client.list_secrets();

        while let Some(result) = stream.next().await {
            match result {
                Ok(properties) => {
                    if let Ok(name) = SecretName::new(properties.name()) {
                        names.push(name);
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to list secrets");
                    return Err(KeyVaultError::Internal {
                        message: format!("Failed to list secrets: {}", e),
                    });
                }
            }
        }

        info!(count = names.len(), "Successfully listed secrets");
        Ok(names)
    }

    #[instrument(skip(self))]
    async fn clear_cache(&self, name: &SecretName) -> Result<(), KeyVaultError> {
        self.cache.remove(name).await
    }

    #[instrument(skip(self))]
    async fn clear_all_cache(&self) -> Result<(), KeyVaultError> {
        self.cache.clear().await
    }

    #[instrument(skip(self))]
    async fn get_cache_stats(&self) -> Result<CacheStatistics, KeyVaultError> {
        self.cache.get_statistics().await
    }
}

#[cfg(test)]
#[path = "azure_key_vault_tests.rs"]
mod tests;
