//! Circuit breaker protection for Key Vault operations.
//!
//! Wraps KeyVaultProvider with circuit breaker protection and graceful fallback
//! to cached secrets when the circuit is open.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::circuit_breaker::{
    key_vault_circuit_breaker_config, CircuitBreaker, CircuitBreakerError, CircuitBreakerFactory,
    DefaultCircuitBreaker, DefaultCircuitBreakerFactory,
};
use crate::key_vault::{
    KeyVaultConfiguration, KeyVaultError, KeyVaultProvider, SecretCache, SecretName, SecretValue,
};

/// Key Vault provider with circuit breaker protection.
///
/// Wraps a KeyVaultProvider with circuit breaker protection and implements
/// graceful degradation by falling back to cached secrets when the circuit
/// is open.
#[derive(Clone)]
pub struct CircuitBreakerKeyVaultProvider {
    /// Underlying Key Vault provider
    inner: Arc<dyn KeyVaultProvider>,
    /// Circuit breaker for protecting Key Vault operations
    circuit_breaker: DefaultCircuitBreaker<SecretValue, KeyVaultError>,
    /// Cache for fallback when circuit is open
    cache: Arc<dyn SecretCache>,
}

impl CircuitBreakerKeyVaultProvider {
    /// Create new circuit breaker protected Key Vault provider.
    ///
    /// # Arguments
    /// - `inner`: Underlying KeyVaultProvider to protect
    /// - `cache`: Secret cache for graceful fallback
    /// - `config`: Key Vault configuration
    pub fn new(
        inner: Arc<dyn KeyVaultProvider>,
        cache: Arc<dyn SecretCache>,
        _config: KeyVaultConfiguration,
    ) -> Self {
        let factory = DefaultCircuitBreakerFactory;
        let circuit_breaker_config = key_vault_circuit_breaker_config();
        let circuit_breaker = factory.create_typed_circuit_breaker(circuit_breaker_config);

        Self {
            inner,
            circuit_breaker,
            cache,
        }
    }

    /// Attempt graceful fallback to cached secret when circuit is open.
    ///
    /// Returns cached secret if available, even if expired, for degraded operation.
    async fn fallback_to_cache(&self, name: &SecretName) -> Option<SecretValue> {
        match self.cache.get(name).await {
            Some(cached) => {
                if cached.is_expired() {
                    warn!(
                        secret_name = %name,
                        "Using expired cached secret due to circuit breaker open"
                    );
                } else {
                    debug!(
                        secret_name = %name,
                        "Using cached secret due to circuit breaker open"
                    );
                }
                Some(cached.value)
            }
            None => {
                warn!(
                    secret_name = %name,
                    "No cached secret available for fallback"
                );
                None
            }
        }
    }
}

#[async_trait]
impl KeyVaultProvider for CircuitBreakerKeyVaultProvider {
    async fn get_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        let name_clone = name.clone();
        let inner = Arc::clone(&self.inner);

        match self
            .circuit_breaker
            .call(|| async move { inner.get_secret(&name_clone).await })
            .await
        {
            Ok(value) => Ok(value),
            Err(CircuitBreakerError::CircuitOpen) => {
                info!(
                    secret_name = %name,
                    "Circuit breaker open for Key Vault, attempting cache fallback"
                );

                // Graceful degradation: use cached value if available
                self.fallback_to_cache(name).await.ok_or_else(|| {
                    KeyVaultError::ServiceUnavailable {
                        message: format!(
                            "Key Vault circuit breaker open and no cached value for {}",
                            name.as_str()
                        ),
                    }
                })
            }
            Err(CircuitBreakerError::Timeout { timeout_ms }) => {
                warn!(
                    secret_name = %name,
                    timeout_ms = timeout_ms,
                    "Key Vault operation timed out, attempting cache fallback"
                );

                // Timeout: try cache fallback
                self.fallback_to_cache(name)
                    .await
                    .ok_or_else(|| KeyVaultError::Internal {
                        message: format!("Key Vault timeout after {}ms", timeout_ms),
                    })
            }
            Err(CircuitBreakerError::OperationFailed(e)) => Err(e),
            Err(CircuitBreakerError::TooManyConcurrentRequests) => {
                // Circuit in half-open, too many concurrent requests
                warn!(
                    secret_name = %name,
                    "Too many concurrent requests in half-open state"
                );
                self.fallback_to_cache(name).await.ok_or_else(|| {
                    KeyVaultError::ServiceUnavailable {
                        message: "Key Vault circuit breaker limiting requests".to_string(),
                    }
                })
            }
            Err(CircuitBreakerError::InternalError { message }) => {
                Err(KeyVaultError::Internal { message })
            }
        }
    }

    async fn get_secret_with_version(
        &self,
        name: &SecretName,
    ) -> Result<(SecretValue, String), KeyVaultError> {
        let name_clone = name.clone();
        let inner = Arc::clone(&self.inner);

        match self
            .circuit_breaker
            .call(|| async move { inner.get_secret(&name_clone).await })
            .await
        {
            Ok(value) => {
                // Fetch version separately (not protected by circuit breaker)
                let (_, version) = self.inner.get_secret_with_version(name).await?;
                Ok((value, version))
            }
            Err(CircuitBreakerError::CircuitOpen) => {
                info!(
                    secret_name = %name,
                    "Circuit breaker open for Key Vault, attempting cache fallback"
                );

                // Graceful degradation: use cached value with version if available
                match self.cache.get(name).await {
                    Some(cached) => {
                        let version = cached
                            .version
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string());
                        if cached.is_expired() {
                            warn!(
                                secret_name = %name,
                                "Using expired cached secret with version due to circuit breaker"
                            );
                        }
                        Ok((cached.value, version))
                    }
                    None => Err(KeyVaultError::ServiceUnavailable {
                        message: format!(
                            "Key Vault circuit breaker open and no cached value for {}",
                            name.as_str()
                        ),
                    }),
                }
            }
            Err(CircuitBreakerError::Timeout { timeout_ms }) => {
                warn!(
                    secret_name = %name,
                    timeout_ms = timeout_ms,
                    "Key Vault operation timed out"
                );
                Err(KeyVaultError::Internal {
                    message: format!("Key Vault timeout after {}ms", timeout_ms),
                })
            }
            Err(CircuitBreakerError::OperationFailed(e)) => Err(e),
            Err(CircuitBreakerError::TooManyConcurrentRequests) => {
                Err(KeyVaultError::ServiceUnavailable {
                    message: "Key Vault circuit breaker limiting requests".to_string(),
                })
            }
            Err(CircuitBreakerError::InternalError { message }) => {
                Err(KeyVaultError::Internal { message })
            }
        }
    }

    async fn refresh_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        // Refresh bypasses circuit breaker (administrative operation)
        self.inner.refresh_secret(name).await
    }

    async fn secret_exists(&self, name: &SecretName) -> Result<bool, KeyVaultError> {
        // Existence check is lightweight, don't protect with circuit breaker
        self.inner.secret_exists(name).await
    }

    async fn list_secret_names(&self) -> Result<Vec<SecretName>, KeyVaultError> {
        // List operation is administrative, don't protect with circuit breaker
        self.inner.list_secret_names().await
    }

    async fn clear_cache(&self, name: &SecretName) -> Result<(), KeyVaultError> {
        self.cache.remove(name).await
    }

    async fn clear_all_cache(&self) -> Result<(), KeyVaultError> {
        self.cache.clear().await
    }

    async fn get_cache_stats(&self) -> Result<crate::key_vault::CacheStatistics, KeyVaultError> {
        // Delegate to inner provider
        self.inner.get_cache_stats().await
    }
}

#[cfg(test)]
#[path = "circuit_breaker_key_vault_tests.rs"]
mod tests;
