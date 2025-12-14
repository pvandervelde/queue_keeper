//! # In-Memory Key Vault Implementation
//!
//! Thread-safe in-memory implementation for testing and development.
//! Provides full KeyVaultProvider interface with caching and rotation support.

use crate::key_vault::{
    CacheStatistics, CachedSecret, KeyVaultConfiguration, KeyVaultError, KeyVaultProvider,
    SecretCache, SecretName, SecretValue,
};
use crate::Timestamp;
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

/// Thread-safe in-memory secret cache
///
/// Uses RwLock for concurrent access with minimal contention.
/// Suitable for testing and development scenarios.
#[derive(Clone)]
pub struct InMemorySecretCache {
    secrets: Arc<RwLock<HashMap<SecretName, CachedSecret>>>,
    stats: Arc<RwLock<CacheStats>>,
}

#[derive(Debug, Clone, Default)]
struct CacheStats {
    hits: u64,
    misses: u64,
    expired_removed: u64,
}

impl InMemorySecretCache {
    /// Create new empty cache
    pub fn new() -> Self {
        Self {
            secrets: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Create cache pre-populated with secrets
    pub fn with_secrets(secrets: HashMap<SecretName, SecretValue>) -> Self {
        let cache = Self::new();
        let now = Timestamp::now();

        {
            let mut cache_map = cache.secrets.write().unwrap();
            for (name, value) in secrets {
                let cached = CachedSecret {
                    name: name.clone(),
                    value,
                    cached_at: now,
                    expires_at: now.add_seconds(300), // 5 minutes
                    extended_expires_at: now.add_seconds(3600), // 1 hour
                    version: Some("v1".to_string()),
                };
                cache_map.insert(name, cached);
            }
        } // Lock dropped here

        cache
    }
}

impl Default for InMemorySecretCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretCache for InMemorySecretCache {
    async fn get(&self, name: &SecretName) -> Option<CachedSecret> {
        let mut secrets = self.secrets.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        if let Some(cached) = secrets.get(name) {
            if cached.is_extended_expired() {
                // Remove expired secret
                secrets.remove(name);
                stats.expired_removed += 1;
                stats.misses += 1;
                None
            } else {
                stats.hits += 1;
                Some(cached.clone())
            }
        } else {
            stats.misses += 1;
            None
        }
    }

    async fn put(
        &self,
        name: SecretName,
        value: SecretValue,
        ttl: Duration,
    ) -> Result<(), KeyVaultError> {
        let now = Timestamp::now();
        let ttl_secs = ttl.as_secs();
        let cached = CachedSecret {
            name: name.clone(),
            value,
            cached_at: now,
            expires_at: now.add_seconds(ttl_secs),
            extended_expires_at: now.add_seconds(ttl_secs * 2),
            version: Some("v1".to_string()),
        };

        self.secrets.write().unwrap().insert(name, cached);
        Ok(())
    }

    async fn put_with_version(
        &self,
        name: SecretName,
        value: SecretValue,
        version: String,
        ttl: Duration,
    ) -> Result<(), KeyVaultError> {
        let now = Timestamp::now();
        let ttl_secs = ttl.as_secs();
        let cached = CachedSecret {
            name: name.clone(),
            value,
            cached_at: now,
            expires_at: now.add_seconds(ttl_secs),
            extended_expires_at: now.add_seconds(ttl_secs * 2),
            version: Some(version),
        };

        self.secrets.write().unwrap().insert(name, cached);
        Ok(())
    }

    async fn remove(&self, name: &SecretName) -> Result<(), KeyVaultError> {
        self.secrets.write().unwrap().remove(name);
        Ok(())
    }

    async fn clear(&self) -> Result<(), KeyVaultError> {
        self.secrets.write().unwrap().clear();
        self.stats.write().unwrap().hits = 0;
        self.stats.write().unwrap().misses = 0;
        self.stats.write().unwrap().expired_removed = 0;
        Ok(())
    }

    async fn get_expiring_secrets(
        &self,
        threshold: Duration,
    ) -> Result<Vec<SecretName>, KeyVaultError> {
        let secrets = self.secrets.read().unwrap();
        let now = Timestamp::now();
        let threshold_time = now.add_seconds(threshold.as_secs());

        Ok(secrets
            .values()
            .filter(|cached| cached.expires_at < threshold_time)
            .map(|cached| cached.name.clone())
            .collect())
    }

    async fn cleanup_expired(&self) -> Result<usize, KeyVaultError> {
        let mut secrets = self.secrets.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        let expired: Vec<SecretName> = secrets
            .values()
            .filter(|cached| cached.is_extended_expired())
            .map(|cached| cached.name.clone())
            .collect();

        let count = expired.len();
        for name in expired {
            secrets.remove(&name);
        }

        stats.expired_removed += count as u64;
        Ok(count)
    }

    async fn get_statistics(&self) -> Result<CacheStatistics, KeyVaultError> {
        let secrets = self.secrets.read().unwrap();
        let stats = self.stats.read().unwrap();

        let total_requests = stats.hits + stats.misses;
        let hit_ratio = if total_requests > 0 {
            stats.hits as f64 / total_requests as f64
        } else {
            0.0
        };

        let estimated_memory = secrets
            .values()
            .map(|cached| {
                cached.name.as_str().len()
                    + cached.value.len()
                    + cached.version.as_ref().map_or(0, |v| v.len())
                    + 64 // Overhead for CachedSecret struct
            })
            .sum();

        Ok(CacheStatistics {
            cached_secrets_count: secrets.len(),
            hit_ratio,
            total_hits: stats.hits,
            total_misses: stats.misses,
            expired_secrets_removed: stats.expired_removed,
            estimated_memory_usage: estimated_memory,
            avg_retrieval_time_ms: 0.1, // In-memory is near-instant
            active_refresh_operations: 0,
            collected_at: Timestamp::now(),
        })
    }
}

/// In-memory Key Vault provider for testing and development
///
/// Provides full KeyVaultProvider interface backed by HashMap.
/// Thread-safe and suitable for unit/integration tests.
pub struct InMemoryKeyVaultProvider {
    config: KeyVaultConfiguration,
    cache: InMemorySecretCache,
    backend: Arc<RwLock<HashMap<SecretName, (SecretValue, String)>>>,
}

impl InMemoryKeyVaultProvider {
    /// Create new in-memory provider with default configuration
    pub fn new() -> Self {
        Self {
            config: KeyVaultConfiguration::default(),
            cache: InMemorySecretCache::new(),
            backend: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create provider with custom configuration
    pub fn with_config(config: KeyVaultConfiguration) -> Self {
        Self {
            config,
            cache: InMemorySecretCache::new(),
            backend: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create provider pre-populated with secrets
    pub fn with_secrets(secrets: HashMap<SecretName, SecretValue>) -> Self {
        let mut backend_secrets = HashMap::new();
        for (name, value) in secrets.iter() {
            backend_secrets.insert(name.clone(), (value.clone(), "v1".to_string()));
        }

        Self {
            config: KeyVaultConfiguration::default(),
            cache: InMemorySecretCache::with_secrets(secrets),
            backend: Arc::new(RwLock::new(backend_secrets)),
        }
    }

    /// Add secret to backend storage (simulates Key Vault)
    pub fn add_secret(&self, name: SecretName, value: SecretValue) {
        self.backend
            .write()
            .unwrap()
            .insert(name, (value, "v1".to_string()));
    }

    /// Update secret version (simulates secret rotation)
    pub fn rotate_secret(&self, name: &SecretName, new_value: SecretValue, new_version: String) {
        self.backend
            .write()
            .unwrap()
            .insert(name.clone(), (new_value, new_version));
    }

    /// Remove secret from backend
    pub fn remove_secret(&self, name: &SecretName) {
        self.backend.write().unwrap().remove(name);
    }
}

impl Default for InMemoryKeyVaultProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl KeyVaultProvider for InMemoryKeyVaultProvider {
    async fn get_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        // Check cache first
        if let Some(cached) = self.cache.get(name).await {
            if !cached.is_expired() {
                return Ok(cached.value);
            }
        }

        // Fetch from backend
        let value_clone = {
            let backend = self.backend.read().unwrap();
            if let Some((value, _version)) = backend.get(name) {
                value.clone()
            } else {
                return Err(KeyVaultError::SecretNotFound { name: name.clone() });
            }
        }; // Lock released here

        // Update cache
        let ttl = Duration::from_secs(self.config.cache_ttl_seconds);
        self.cache
            .put(name.clone(), value_clone.clone(), ttl)
            .await?;

        Ok(value_clone)
    }

    async fn get_secret_with_version(
        &self,
        name: &SecretName,
    ) -> Result<(SecretValue, String), KeyVaultError> {
        // Check cache for version
        if let Some(cached) = self.cache.get(name).await {
            if !cached.is_expired() {
                if let Some(version) = cached.version {
                    return Ok((cached.value, version));
                }
            }
        }

        // Fetch from backend
        let (value_clone, version_clone) = {
            let backend = self.backend.read().unwrap();
            if let Some((value, version)) = backend.get(name) {
                (value.clone(), version.clone())
            } else {
                return Err(KeyVaultError::SecretNotFound { name: name.clone() });
            }
        }; // Lock released here

        // Update cache
        let ttl = Duration::from_secs(self.config.cache_ttl_seconds);
        self.cache
            .put_with_version(name.clone(), value_clone.clone(), version_clone.clone(), ttl)
            .await?;

        Ok((value_clone, version_clone))
    }

    async fn refresh_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        // Clear cache entry
        self.cache.remove(name).await?;

        // Fetch fresh value
        self.get_secret(name).await
    }

    async fn secret_exists(&self, name: &SecretName) -> Result<bool, KeyVaultError> {
        Ok(self.backend.read().unwrap().contains_key(name))
    }

    async fn list_secret_names(&self) -> Result<Vec<SecretName>, KeyVaultError> {
        Ok(self.backend.read().unwrap().keys().cloned().collect())
    }

    async fn clear_cache(&self, name: &SecretName) -> Result<(), KeyVaultError> {
        self.cache.remove(name).await
    }

    async fn clear_all_cache(&self) -> Result<(), KeyVaultError> {
        self.cache.clear().await
    }

    async fn get_cache_stats(&self) -> Result<CacheStatistics, KeyVaultError> {
        self.cache.get_statistics().await
    }
}

#[cfg(test)]
#[path = "memory_key_vault_tests.rs"]
mod tests;
