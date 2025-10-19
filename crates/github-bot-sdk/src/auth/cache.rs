//! Token caching implementation for GitHub App authentication.
//!
//! Provides thread-safe, TTL-based caching for JWT and installation tokens.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::{GitHubAppId, InstallationId, InstallationToken, JsonWebToken, TokenCache};
use crate::error::CacheError;

/// In-memory token cache with TTL support.
///
/// Provides thread-safe caching for both JWT and installation tokens with
/// automatic expiration handling.
pub struct InMemoryTokenCache {
    jwt_cache: Arc<RwLock<HashMap<GitHubAppId, CachedToken<JsonWebToken>>>>,
    installation_cache: Arc<RwLock<HashMap<InstallationId, CachedToken<InstallationToken>>>>,
}

/// Cached token with metadata.
struct CachedToken<T> {
    token: T,
}

impl<T> CachedToken<T> {
    fn new(token: T) -> Self {
        Self { token }
    }

    fn token(&self) -> &T {
        &self.token
    }

    fn is_valid(&self) -> bool
    where
        T: TokenExpiry,
    {
        !self.token.is_expired()
    }
}

/// Trait for tokens that have expiration.
trait TokenExpiry {
    fn is_expired(&self) -> bool;
}

impl TokenExpiry for JsonWebToken {
    fn is_expired(&self) -> bool {
        self.is_expired()
    }
}

impl TokenExpiry for InstallationToken {
    fn is_expired(&self) -> bool {
        self.is_expired()
    }
}

impl InMemoryTokenCache {
    /// Create a new in-memory token cache.
    pub fn new() -> Self {
        Self {
            jwt_cache: Arc::new(RwLock::new(HashMap::new())),
            installation_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryTokenCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenCache for InMemoryTokenCache {
    async fn get_jwt(&self, app_id: GitHubAppId) -> Result<Option<JsonWebToken>, CacheError> {
        let cache = self
            .jwt_cache
            .read()
            .map_err(|e| CacheError::OperationFailed {
                message: format!("Failed to acquire read lock: {}", e),
            })?;

        Ok(cache.get(&app_id).map(|cached| cached.token().clone()))
    }

    async fn store_jwt(&self, jwt: JsonWebToken) -> Result<(), CacheError> {
        let mut cache = self
            .jwt_cache
            .write()
            .map_err(|e| CacheError::OperationFailed {
                message: format!("Failed to acquire write lock: {}", e),
            })?;

        let app_id = jwt.app_id();
        cache.insert(app_id, CachedToken::new(jwt));

        Ok(())
    }

    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<Option<InstallationToken>, CacheError> {
        let cache = self
            .installation_cache
            .read()
            .map_err(|e| CacheError::OperationFailed {
                message: format!("Failed to acquire read lock: {}", e),
            })?;

        Ok(cache
            .get(&installation_id)
            .map(|cached| cached.token().clone()))
    }

    async fn store_installation_token(&self, token: InstallationToken) -> Result<(), CacheError> {
        let mut cache =
            self.installation_cache
                .write()
                .map_err(|e| CacheError::OperationFailed {
                    message: format!("Failed to acquire write lock: {}", e),
                })?;

        let installation_id = token.installation_id();
        cache.insert(installation_id, CachedToken::new(token));

        Ok(())
    }

    async fn invalidate_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<(), CacheError> {
        let mut cache =
            self.installation_cache
                .write()
                .map_err(|e| CacheError::OperationFailed {
                    message: format!("Failed to acquire write lock: {}", e),
                })?;

        cache.remove(&installation_id);

        Ok(())
    }

    fn cleanup_expired_tokens(&self) {
        // Cleanup JWT tokens
        if let Ok(mut jwt_cache) = self.jwt_cache.write() {
            jwt_cache.retain(|_, cached| cached.is_valid());
        }

        // Cleanup installation tokens
        if let Ok(mut inst_cache) = self.installation_cache.write() {
            inst_cache.retain(|_, cached| cached.is_valid());
        }
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
