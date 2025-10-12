# Key Vault Interface

**Architectural Layer**: Infrastructure Interface
**Module Path**: `src/key_vault.rs`
**Responsibilities** (from RDD):

- Knows: Secret naming conventions, caching policies, credential rotation patterns
- Does: Retrieves secrets securely, manages cache lifecycle, provides credential refresh

## Dependencies

- Types: `SecretName`, `SecretValue`, `CacheEntry` (key-vault-types.md)
- Shared: `Result<T, E>`, `Timestamp` (shared-types.md)
- External: Azure Key Vault SDK, AWS Secrets Manager SDK

## Overview

The Key Vault Interface defines how Queue-Keeper securely retrieves and manages secrets required for external service authentication. This system implements REQ-012 (Secret Management) by providing secure access to GitHub webhook secrets, database connection strings, and other sensitive configuration.

**Critical Design Principles:**

- **Security First**: Secrets never logged, use secure memory when possible
- **Short-lived Cache**: 5-minute maximum cache TTL for security/performance balance
- **Managed Identity**: Azure Managed Identity authentication (no connection strings)
- **Rotation Support**: Handle secret rotation without application restart
- **Graceful Degradation**: Use cached secrets beyond normal expiry during outages

## Types

### SecretName

Strongly-typed secret identifier with naming convention validation.

```rust
/// Secret identifier with naming convention validation
///
/// Enforces consistent naming for secrets across environments and services.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SecretName(String);

impl SecretName {
    /// Create new secret name with validation
    ///
    /// # Validation Rules
    /// - Must be 1-127 characters (Azure Key Vault limit)
    /// - Must contain only alphanumeric characters and hyphens
    /// - Must follow convention: {service}-{environment}-{purpose}
    /// - Examples: "queue-keeper-prod-github-webhook", "queue-keeper-dev-database-conn"
    pub fn new(name: impl Into<String>) -> Result<Self, KeyVaultError>;

    /// Create secret name from components
    ///
    /// # Arguments
    /// - service: Service name (e.g., "queue-keeper")
    /// - environment: Environment (e.g., "prod", "dev", "staging")
    /// - purpose: Secret purpose (e.g., "github-webhook", "database-conn")
    pub fn from_components(service: &str, environment: &str, purpose: &str) -> Result<Self, KeyVaultError>;

    /// Get string representation
    pub fn as_str(&self) -> &str;

    /// Get components (service, environment, purpose)
    pub fn get_components(&self) -> Option<(String, String, String)>;
}
```

### SecretValue

Secure container for secret data with automatic cleanup.

```rust
/// Secure container for secret values
///
/// Provides secure handling of secret data with automatic cleanup.
/// Secret values are never included in Debug output or logs.
#[derive(Clone)]
pub struct SecretValue {
    // Internal: Uses secure string or zeroized buffer
    inner: SecretValueInner,
}

impl SecretValue {
    /// Create secret value from string
    ///
    /// # Security
    /// - Original string should be zeroized after creating SecretValue
    /// - SecretValue takes ownership and manages secure cleanup
    pub fn from_string(value: String) -> Self;

    /// Create secret value from bytes
    pub fn from_bytes(value: Vec<u8>) -> Self;

    /// Get secret as string (only for immediate use)
    ///
    /// # Security Warning
    /// The returned string contains the actual secret value.
    /// Use immediately and avoid storing in variables.
    pub fn expose_secret(&self) -> &str;

    /// Get secret as bytes
    pub fn expose_bytes(&self) -> &[u8];

    /// Check if secret is empty
    pub fn is_empty(&self) -> bool;

    /// Get secret length without exposing content
    pub fn len(&self) -> usize;
}

impl Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretValue")
            .field("length", &self.len())
            .field("value", &"[REDACTED]")
            .finish()
    }
}

// Secure cleanup on drop
impl Drop for SecretValue {
    fn drop(&mut self) {
        // Zero memory before deallocation
        self.inner.zeroize();
    }
}
```

### CachedSecret

Secret with cache metadata for expiration and refresh logic.

```rust
/// Cached secret with expiration and refresh metadata
#[derive(Debug, Clone)]
pub struct CachedSecret {
    /// Secret name for identification
    pub name: SecretName,

    /// Secret value (secure container)
    pub value: SecretValue,

    /// When secret was cached
    pub cached_at: Timestamp,

    /// Normal cache expiration time
    pub expires_at: Timestamp,

    /// Extended expiration for outage scenarios
    pub extended_expires_at: Timestamp,

    /// Secret version (for change detection)
    pub version: Option<String>,
}

impl CachedSecret {
    /// Check if secret is expired (normal expiration)
    pub fn is_expired(&self) -> bool;

    /// Check if secret is expired (extended expiration)
    pub fn is_extended_expired(&self) -> bool;

    /// Check if secret should be refreshed (proactive refresh)
    pub fn should_refresh(&self) -> bool;

    /// Get age of cached secret
    pub fn get_age(&self) -> Duration;

    /// Check if version has changed
    pub fn version_changed(&self, other_version: Option<&str>) -> bool;
}
```

### KeyVaultConfiguration

Configuration for Key Vault provider behavior.

```rust
/// Configuration for Key Vault behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyVaultConfiguration {
    /// Key Vault URL (Azure) or region (AWS)
    pub vault_url: String,

    /// Default cache TTL for secrets
    pub cache_ttl_seconds: u64,

    /// Extended cache TTL for outage scenarios
    pub extended_cache_ttl_seconds: u64,

    /// Proactive refresh threshold (refresh when TTL remaining < threshold)
    pub refresh_threshold_seconds: u64,

    /// Maximum concurrent secret retrievals
    pub max_concurrent_requests: usize,

    /// Request timeout for Key Vault operations
    pub request_timeout_seconds: u64,

    /// Enable background refresh of expiring secrets
    pub enable_background_refresh: bool,
}

impl Default for KeyVaultConfiguration {
    fn default() -> Self {
        Self {
            vault_url: String::new(), // Must be provided
            cache_ttl_seconds: 300,    // 5 minutes
            extended_cache_ttl_seconds: 3600, // 1 hour for outages
            refresh_threshold_seconds: 60,    // Refresh 1 minute before expiry
            max_concurrent_requests: 10,
            request_timeout_seconds: 30,
            enable_background_refresh: true,
        }
    }
}
```

## Core Interfaces

### KeyVaultProvider

Main interface for secure secret retrieval and management.

```rust
/// Interface for secure secret management
///
/// Provides secure access to secrets with caching and rotation support.
/// Implementations handle provider-specific authentication and API calls.
#[async_trait::async_trait]
pub trait KeyVaultProvider: Send + Sync {
    /// Get secret by name with caching
    ///
    /// Returns cached value if available and not expired.
    /// Fetches from Key Vault if cache miss or expired.
    ///
    /// # Errors
    /// - `KeyVaultError::SecretNotFound` - Secret doesn in exist
    /// - `KeyVaultError::AccessDenied` - Insufficient permissions
    /// - `KeyVaultError::ServiceUnavailable` - Key Vault unreachable
    async fn get_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError>;

    /// Get secret with version for change detection
    ///
    /// Returns secret value and current version identifier.
    /// Useful for detecting when secrets have been rotated.
    async fn get_secret_with_version(&self, name: &SecretName) -> Result<(SecretValue, String), KeyVaultError>;

    /// Refresh secret in cache (force fetch from Key Vault)
    ///
    /// Bypasses cache and fetches current secret value.
    /// Updates cache with new value and expiration.
    async fn refresh_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError>;

    /// Check if secret exists without retrieving value
    ///
    /// Lightweight operation for validation and health checks.
    async fn secret_exists(&self, name: &SecretName) -> Result<bool, KeyVaultError>;

    /// List available secrets (names only, no values)
    ///
    /// Returns secret names that the service has permission to access.
    /// Useful for configuration validation and health checks.
    async fn list_secret_names(&self) -> Result<Vec<SecretName>, KeyVaultError>;

    /// Clear secret from cache
    ///
    /// Forces next get_secret to fetch fresh value from Key Vault.
    /// Useful for testing or manual secret rotation.
    async fn clear_cache(&self, name: &SecretName) -> Result<(), KeyVaultError>;

    /// Clear all cached secrets
    ///
    /// Emergency operation to clear entire secret cache.
    async fn clear_all_cache(&self) -> Result<(), KeyVaultError>;

    /// Get cache statistics for monitoring
    async fn get_cache_stats(&self) -> Result<CacheStatistics, KeyVaultError>;
}
```

### SecretCache

Interface for secret caching with security and expiration management.

```rust
/// Interface for secure secret caching
///
/// Manages in-memory cache of secrets with security and expiration policies.
/// Separate from provider to enable testing and different cache strategies.
#[async_trait::async_trait]
pub trait SecretCache: Send + Sync {
    /// Get cached secret if available and not expired
    async fn get(&self, name: &SecretName) -> Option<CachedSecret>;

    /// Store secret in cache with expiration
    async fn put(&self, name: SecretName, value: SecretValue, ttl: Duration) -> Result<(), KeyVaultError>;

    /// Store secret with version for change detection
    async fn put_with_version(
        &self,
        name: SecretName,
        value: SecretValue,
        version: String,
        ttl: Duration,
    ) -> Result<(), KeyVaultError>;

    /// Remove specific secret from cache
    async fn remove(&self, name: &SecretName) -> Result<(), KeyVaultError>;

    /// Clear all cached secrets
    async fn clear(&self) -> Result<(), KeyVaultError>;

    /// Get secrets that are expired or expiring soon
    async fn get_expiring_secrets(&self, threshold: Duration) -> Result<Vec<SecretName>, KeyVaultError>;

    /// Clean up expired secrets from cache
    async fn cleanup_expired(&self) -> Result<usize, KeyVaultError>;

    /// Get cache statistics
    async fn get_statistics(&self) -> Result<CacheStatistics, KeyVaultError>;
}
```

### SecretRotationHandler

Interface for handling secret rotation events and notifications.

```rust
/// Interface for handling secret rotation
///
/// Provides hooks for secret rotation detection and response.
/// Enables proactive cache invalidation and application notification.
#[async_trait::async_trait]
pub trait SecretRotationHandler: Send + Sync {
    /// Handle secret rotation notification
    ///
    /// Called when secret rotation is detected (version change).
    /// Implementation should update dependent services.
    async fn on_secret_rotated(&self, name: &SecretName, old_version: Option<String>, new_version: String) -> Result<(), KeyVaultError>;

    /// Handle secret expiration warning
    ///
    /// Called when secret will expire soon.
    /// Implementation can trigger renewal or alert operations team.
    async fn on_secret_expiring(&self, name: &SecretName, expires_in: Duration) -> Result<(), KeyVaultError>;

    /// Handle secret retrieval failure
    ///
    /// Called when secret cannot be retrieved from Key Vault.
    /// Implementation should handle graceful degradation.
    async fn on_secret_unavailable(&self, name: &SecretName, error: &KeyVaultError) -> Result<(), KeyVaultError>;
}
```

## Supporting Types

### CacheStatistics

Observability data for secret cache performance and health.

```rust
/// Cache performance and health statistics
#[derive(Debug, Clone, Serialize)]
pub struct CacheStatistics {
    /// Total number of secrets currently cached
    pub cached_secrets_count: usize,

    /// Cache hit ratio (0.0 to 1.0)
    pub hit_ratio: f64,

    /// Total cache hits since start
    pub total_hits: u64,

    /// Total cache misses since start
    pub total_misses: u64,

    /// Number of secrets expired and removed
    pub expired_secrets_removed: u64,

    /// Memory usage estimate (bytes)
    pub estimated_memory_usage: usize,

    /// Average secret retrieval time (milliseconds)
    pub avg_retrieval_time_ms: f64,

    /// Number of active background refresh operations
    pub active_refresh_operations: usize,

    /// Timestamp when statistics were collected
    pub collected_at: Timestamp,
}
```

### SecretMetadata

Metadata about secret without exposing the actual value.

```rust
/// Secret metadata for observability and management
#[derive(Debug, Clone, Serialize)]
pub struct SecretMetadata {
    /// Secret name
    pub name: SecretName,

    /// When secret was created in Key Vault
    pub created_at: Option<Timestamp>,

    /// When secret was last updated
    pub updated_at: Option<Timestamp>,

    /// Current version identifier
    pub version: Option<String>,

    /// Whether secret is enabled/disabled
    pub enabled: bool,

    /// Secret content type (if available)
    pub content_type: Option<String>,

    /// Tags associated with secret
    pub tags: HashMap<String, String>,
}
```

### KeyVaultHealthStatus

Health check information for Key Vault connectivity and performance.

```rust
/// Key Vault health status for monitoring
#[derive(Debug, Clone, Serialize)]
pub struct KeyVaultHealthStatus {
    /// Whether Key Vault is reachable
    pub is_available: bool,

    /// Response time for health check (milliseconds)
    pub response_time_ms: u64,

    /// Number of secrets accessible
    pub accessible_secrets_count: usize,

    /// Any authentication or permission issues
    pub auth_status: AuthStatus,

    /// Cache health information
    pub cache_health: CacheHealthStatus,

    /// When health check was performed
    pub checked_at: Timestamp,
}

/// Authentication status with Key Vault
#[derive(Debug, Clone, Serialize)]
pub enum AuthStatus {
    /// Authentication successful
    Authenticated,

    /// Authentication failed
    AuthenticationFailed { reason: String },

    /// Insufficient permissions
    InsufficientPermissions { missing_permissions: Vec<String> },

    /// Cannot determine status (service unavailable)
    Unknown,
}

/// Cache health status
#[derive(Debug, Clone, Serialize)]
pub struct CacheHealthStatus {
    /// Whether cache is functioning normally
    pub is_healthy: bool,

    /// Number of secrets with expired cache entries
    pub expired_entries: usize,

    /// Number of secrets that failed to refresh
    pub failed_refreshes: usize,

    /// Memory pressure indicators
    pub memory_pressure: bool,
}
```

## Error Types

### KeyVaultError

Comprehensive error type for Key Vault operations.

```rust
/// Errors that can occur during Key Vault operations
#[derive(Debug, thiserror::Error)]
pub enum KeyVaultError {
    #[error("Secret not found: {name}")]
    SecretNotFound { name: SecretName },

    #[error("Access denied to secret: {name} - {reason}")]
    AccessDenied { name: SecretName, reason: String },

    #[error("Key Vault service unavailable: {message}")]
    ServiceUnavailable { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Invalid secret name: {name} - {reason}")]
    InvalidSecretName { name: String, reason: String },

    #[error("Secret value too large: {size} bytes (max: {max_size})")]
    SecretTooLarge { size: usize, max_size: usize },

    #[error("Cache operation failed: {operation} - {message}")]
    CacheError { operation: String, message: String },

    #[error("Request timeout after {timeout_seconds} seconds")]
    Timeout { timeout_seconds: u64 },

    #[error("Rate limit exceeded: {retry_after_seconds} seconds")]
    RateLimitExceeded { retry_after_seconds: u64 },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl KeyVaultError {
    /// Check if error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            KeyVaultError::ServiceUnavailable { .. } |
            KeyVaultError::Timeout { .. } |
            KeyVaultError::RateLimitExceeded { .. } |
            KeyVaultError::Internal { .. }
        )
    }

    /// Get retry delay for transient errors
    pub fn get_retry_delay(&self) -> Option<Duration> {
        match self {
            KeyVaultError::RateLimitExceeded { retry_after_seconds } => {
                Some(Duration::from_secs(*retry_after_seconds))
            }
            KeyVaultError::ServiceUnavailable { .. } => Some(Duration::from_secs(30)),
            KeyVaultError::Timeout { .. } => Some(Duration::from_secs(5)),
            _ => None,
        }
    }

    /// Check if error indicates permission problems
    pub fn is_permission_error(&self) -> bool {
        matches!(
            self,
            KeyVaultError::AccessDenied { .. } |
            KeyVaultError::AuthenticationFailed { .. }
        )
    }
}
```

## Common Secret Names

### Predefined Secret Names

Queue-Keeper uses standardized secret names for consistency:

```rust
/// Standard secret names used by Queue-Keeper
pub struct StandardSecrets;

impl StandardSecrets {
    /// GitHub webhook secret for signature validation
    /// Format: "queue-keeper-{env}-github-webhook"
    pub fn github_webhook_secret(environment: &str) -> Result<SecretName, KeyVaultError> {
        SecretName::from_components("queue-keeper", environment, "github-webhook")
    }

    /// Database connection string
    /// Format: "queue-keeper-{env}-database-conn"
    pub fn database_connection(environment: &str) -> Result<SecretName, KeyVaultError> {
        SecretName::from_components("queue-keeper", environment, "database-conn")
    }

    /// Service Bus connection string
    /// Format: "queue-keeper-{env}-servicebus-conn"
    pub fn service_bus_connection(environment: &str) -> Result<SecretName, KeyVaultError> {
        SecretName::from_components("queue-keeper", environment, "servicebus-conn")
    }

    /// Blob storage connection string
    /// Format: "queue-keeper-{env}-storage-conn"
    pub fn blob_storage_connection(environment: &str) -> Result<SecretName, KeyVaultError> {
        SecretName::from_components("queue-keeper", environment, "storage-conn")
    }

    /// Application Insights instrumentation key
    /// Format: "queue-keeper-{env}-appinsights-key"
    pub fn application_insights_key(environment: &str) -> Result<SecretName, KeyVaultError> {
        SecretName::from_components("queue-keeper", environment, "appinsights-key")
    }
}
```

## Implementation Examples

### Azure Key Vault Usage

```rust
// Example usage with Azure Key Vault
use azure_security_keyvault::prelude::*;

impl KeyVaultProvider for AzureKeyVaultProvider {
    async fn get_secret(&self, name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        // Check cache first
        if let Some(cached) = self.cache.get(name).await {
            if !cached.is_expired() {
                return Ok(cached.value);
            }
        }

        // Fetch from Azure Key Vault
        let secret = self.client
            .get_secret(name.as_str())
            .await
            .map_err(|e| KeyVaultError::ServiceUnavailable {
                message: e.to_string()
            })?;

        let value = SecretValue::from_string(secret.value);

        // Cache with TTL
        self.cache.put(
            name.clone(),
            value.clone(),
            Duration::from_secs(self.config.cache_ttl_seconds),
        ).await?;

        Ok(value)
    }
}
```

### Secret Rotation Detection

```rust
// Background task for detecting secret rotation
impl SecretRotationDetector {
    async fn check_for_rotations(&self) -> Result<(), KeyVaultError> {
        let cached_secrets = self.cache.get_all_names().await?;

        for name in cached_secrets {
            // Get current version from Key Vault
            let (_, current_version) = self.provider
                .get_secret_with_version(&name)
                .await?;

            // Compare with cached version
            if let Some(cached) = self.cache.get(&name).await {
                if cached.version_changed(Some(&current_version)) {
                    // Secret has been rotated
                    self.rotation_handler
                        .on_secret_rotated(&name, cached.version, current_version)
                        .await?;

                    // Clear cache to force refresh
                    self.cache.remove(&name).await?;
                }
            }
        }

        Ok(())
    }
}
```

## Security Considerations

### Secret Handling

1. **Memory Security**: Use secure string types that zero memory on drop
2. **Logging**: Never log secret values, even in debug builds
3. **Error Messages**: Don't include secret values in error messages
4. **Temporary Storage**: Minimize time secrets spend in variables
5. **Thread Safety**: Ensure secret cache is thread-safe for concurrent access

### Cache Security

1. **Memory Protection**: Use memory protection if available (mlock)
2. **Expiration**: Enforce strict cache expiration for security
3. **Cleanup**: Automatic cleanup of expired secrets
4. **Monitoring**: Alert on unusual secret access patterns

### Access Control

1. **Managed Identity**: Use Azure Managed Identity (no stored credentials)
2. **Least Privilege**: Request minimum necessary permissions
3. **Audit Logging**: Log all secret access attempts
4. **Rotation**: Support secret rotation without downtime

## Performance Considerations

### Caching Strategy

1. **Hit Ratio**: Target >95% cache hit ratio for performance
2. **TTL Tuning**: Balance security (short TTL) vs performance (long TTL)
3. **Proactive Refresh**: Refresh secrets before expiration
4. **Concurrent Access**: Handle high-concurrency secret access

### Memory Management

1. **Cache Size**: Limit cache size to prevent memory exhaustion
2. **LRU Eviction**: Evict least recently used secrets when cache full
3. **Memory Monitoring**: Track cache memory usage
4. **Cleanup**: Regular cleanup of expired entries

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_name_validation() {
        // Valid names
        assert!(SecretName::new("queue-keeper-prod-github-webhook").is_ok());
        assert!(SecretName::new("app-dev-database").is_ok());

        // Invalid names
        assert!(SecretName::new("").is_err()); // Empty
        assert!(SecretName::new("invalid_chars!").is_err()); // Invalid characters
        assert!(SecretName::new("a".repeat(128)).is_err()); // Too long
    }

    #[test]
    fn test_secret_name_components() {
        let name = SecretName::from_components("queue-keeper", "prod", "github-webhook").unwrap();
        assert_eq!(name.as_str(), "queue-keeper-prod-github-webhook");

        let (service, env, purpose) = name.get_components().unwrap();
        assert_eq!(service, "queue-keeper");
        assert_eq!(env, "prod");
        assert_eq!(purpose, "github-webhook");
    }

    #[test]
    fn test_secret_value_security() {
        let secret = SecretValue::from_string("sensitive-data".to_string());

        // Debug should not expose value
        let debug_output = format!("{:?}", secret);
        assert!(!debug_output.contains("sensitive-data"));
        assert!(debug_output.contains("[REDACTED]"));

        // Length should be available
        assert_eq!(secret.len(), 14);
    }

    #[test]
    fn test_cached_secret_expiration() {
        let name = SecretName::new("test-secret").unwrap();
        let value = SecretValue::from_string("test-value".to_string());
        let now = Timestamp::now();

        let cached = CachedSecret {
            name,
            value,
            cached_at: now,
            expires_at: now.add_seconds(300), // 5 minutes
            extended_expires_at: now.add_seconds(3600), // 1 hour
            version: Some("v1".to_string()),
        };

        assert!(!cached.is_expired()); // Should not be expired immediately
        assert!(!cached.is_extended_expired());
    }
}
```

### Integration Tests

1. **Azure Key Vault**: Test with real Azure Key Vault instance
2. **Cache Behavior**: Test cache expiration and refresh
3. **Error Handling**: Test network failures and permission errors
4. **Rotation**: Test secret rotation detection and handling

### Contract Tests

1. **Provider Interface**: All implementations pass same test suite
2. **Cache Interface**: Different cache implementations are compatible
3. **Error Behavior**: Consistent error handling across providers

## Monitoring and Observability

### Metrics

- `keyvault_secret_requests_total`: Count of secret requests by result
- `keyvault_cache_hit_ratio`: Cache hit ratio (0.0 to 1.0)
- `keyvault_secret_age_seconds`: Age of cached secrets
- `keyvault_request_duration_seconds`: Key Vault request latency
- `keyvault_rotation_events_total`: Count of detected secret rotations

### Logging

```rust
// Secret retrieval (success)
info!(
    secret_name = %name,
    cache_hit = %cache_hit,
    retrieval_time_ms = %duration.as_millis(),
    "Secret retrieved successfully"
);

// Secret rotation detected
warn!(
    secret_name = %name,
    old_version = ?old_version,
    new_version = %new_version,
    "Secret rotation detected"
);

// Cache cleanup
debug!(
    expired_secrets = %expired_count,
    cleanup_duration_ms = %duration.as_millis(),
    "Cache cleanup completed"
);

// Error cases (no secret values in logs)
error!(
    secret_name = %name,
    error = %error,
    retry_count = %retry_count,
    "Failed to retrieve secret"
);
```

This Key Vault interface provides comprehensive support for REQ-012 while ensuring security best practices, efficient caching, and proper secret rotation handling.
