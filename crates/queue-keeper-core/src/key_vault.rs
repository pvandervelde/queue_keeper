//! # Key Vault Module
//!
//! Secure secret management with caching and rotation support.
//! Implements REQ-012 (Secret Management) for GitHub webhook secrets and
//! other sensitive configuration.
//!
//! See specs/interfaces/key-vault.md for complete specification.

use crate::Timestamp;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, str::FromStr, time::Duration};

// ============================================================================
// Core Types
// ============================================================================

/// Secret identifier with naming convention validation
///
/// Enforces consistent naming for secrets across environments and services.
///
/// See specs/interfaces/key-vault.md
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
    pub fn new(name: impl Into<String>) -> Result<Self, KeyVaultError> {
        let name = name.into();

        if name.is_empty() {
            return Err(KeyVaultError::InvalidSecretName {
                name: name.clone(),
                reason: "Secret name cannot be empty".to_string(),
            });
        }

        if name.len() > 127 {
            return Err(KeyVaultError::InvalidSecretName {
                name: name.clone(),
                reason: "Secret name exceeds 127 character limit".to_string(),
            });
        }

        // Check character restrictions
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(KeyVaultError::InvalidSecretName {
                name: name.clone(),
                reason: "Secret name contains invalid characters".to_string(),
            });
        }

        Ok(Self(name))
    }

    /// Create secret name from components
    ///
    /// # Arguments
    /// - service: Service name (e.g., "queue-keeper")
    /// - environment: Environment (e.g., "prod", "dev", "staging")
    /// - purpose: Secret purpose (e.g., "github-webhook", "database-conn")
    pub fn from_components(
        service: &str,
        environment: &str,
        purpose: &str,
    ) -> Result<Self, KeyVaultError> {
        let name = format!("{}-{}-{}", service, environment, purpose);
        Self::new(name)
    }

    /// Get string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get components (service, environment, purpose)
    pub fn get_components(&self) -> Option<(String, String, String)> {
        let parts: Vec<&str> = self.0.split('-').collect();
        if parts.len() >= 3 {
            let service = parts[0].to_string();
            let environment = parts[1].to_string();
            let purpose = parts[2..].join("-");
            Some((service, environment, purpose))
        } else {
            None
        }
    }
}

impl fmt::Display for SecretName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SecretName {
    type Err = KeyVaultError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// Secure container for secret values
///
/// Provides secure handling of secret data with automatic cleanup.
/// Secret values are never included in Debug output or logs.
///
/// See specs/interfaces/key-vault.md
#[derive(Clone)]
pub struct SecretValue {
    // Internal: Uses secure string or zeroized buffer
    inner: String, // TODO: Replace with zeroizing string in production
}

impl SecretValue {
    /// Create secret value from string
    ///
    /// # Security
    /// - Original string should be zeroized after creating SecretValue
    /// - SecretValue takes ownership and manages secure cleanup
    pub fn from_string(value: String) -> Self {
        Self { inner: value }
    }

    /// Create secret value from bytes
    pub fn from_bytes(value: Vec<u8>) -> Self {
        let string = String::from_utf8_lossy(&value).to_string();
        Self { inner: string }
    }

    /// Get secret as string (only for immediate use)
    ///
    /// # Security Warning
    /// The returned string contains the actual secret value.
    /// Use immediately and avoid storing in variables.
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }

    /// Get secret as bytes
    pub fn expose_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }

    /// Check if secret is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get secret length without exposing content
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl fmt::Debug for SecretValue {
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
        // TODO: Zero memory before deallocation in production
        // For now, just clear the string
        self.inner.clear();
    }
}

/// Cached secret with expiration and refresh metadata
///
/// See specs/interfaces/key-vault.md
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
    pub fn is_expired(&self) -> bool {
        Timestamp::now() > self.expires_at
    }

    /// Check if secret is expired (extended expiration)
    pub fn is_extended_expired(&self) -> bool {
        Timestamp::now() > self.extended_expires_at
    }

    /// Check if secret should be refreshed (proactive refresh)
    pub fn should_refresh(&self) -> bool {
        let now = Timestamp::now();
        let refresh_threshold = Duration::from_secs(60); // 1 minute before expiry
        now > self.expires_at.subtract_duration(refresh_threshold)
    }

    /// Get age of cached secret
    pub fn get_age(&self) -> Duration {
        Timestamp::now().duration_since(self.cached_at)
    }

    /// Check if version has changed
    pub fn version_changed(&self, other_version: Option<&str>) -> bool {
        match (&self.version, other_version) {
            (Some(cached), Some(other)) => cached != other,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        }
    }
}

/// Configuration for Key Vault behavior
///
/// See specs/interfaces/key-vault.md
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
            vault_url: String::new(),         // Must be provided
            cache_ttl_seconds: 300,           // 5 minutes
            extended_cache_ttl_seconds: 3600, // 1 hour for outages
            refresh_threshold_seconds: 60,    // Refresh 1 minute before expiry
            max_concurrent_requests: 10,
            request_timeout_seconds: 30,
            enable_background_refresh: true,
        }
    }
}

// ============================================================================
// Interface Traits
// ============================================================================

/// Interface for secure secret management
///
/// Provides secure access to secrets with caching and rotation support.
/// Implementations handle provider-specific authentication and API calls.
///
/// See specs/interfaces/key-vault.md
#[async_trait]
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
    async fn get_secret_with_version(
        &self,
        name: &SecretName,
    ) -> Result<(SecretValue, String), KeyVaultError>;

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

/// Interface for secure secret caching
///
/// Manages in-memory cache of secrets with security and expiration policies.
/// Separate from provider to enable testing and different cache strategies.
///
/// See specs/interfaces/key-vault.md
#[async_trait]
pub trait SecretCache: Send + Sync {
    /// Get cached secret if available and not expired
    async fn get(&self, name: &SecretName) -> Option<CachedSecret>;

    /// Store secret in cache with expiration
    async fn put(
        &self,
        name: SecretName,
        value: SecretValue,
        ttl: Duration,
    ) -> Result<(), KeyVaultError>;

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
    async fn get_expiring_secrets(
        &self,
        threshold: Duration,
    ) -> Result<Vec<SecretName>, KeyVaultError>;

    /// Clean up expired secrets from cache
    async fn cleanup_expired(&self) -> Result<usize, KeyVaultError>;

    /// Get cache statistics
    async fn get_statistics(&self) -> Result<CacheStatistics, KeyVaultError>;
}

/// Interface for handling secret rotation
///
/// Provides hooks for secret rotation detection and response.
/// Enables proactive cache invalidation and application notification.
///
/// See specs/interfaces/key-vault.md
#[async_trait]
pub trait SecretRotationHandler: Send + Sync {
    /// Handle secret rotation notification
    ///
    /// Called when secret rotation is detected (version change).
    /// Implementation should update dependent services.
    async fn on_secret_rotated(
        &self,
        name: &SecretName,
        old_version: Option<String>,
        new_version: String,
    ) -> Result<(), KeyVaultError>;

    /// Handle secret expiration warning
    ///
    /// Called when secret will expire soon.
    /// Implementation can trigger renewal or alert operations team.
    async fn on_secret_expiring(
        &self,
        name: &SecretName,
        expires_in: Duration,
    ) -> Result<(), KeyVaultError>;

    /// Handle secret retrieval failure
    ///
    /// Called when secret cannot be retrieved from Key Vault.
    /// Implementation should handle graceful degradation.
    async fn on_secret_unavailable(
        &self,
        name: &SecretName,
        error: &KeyVaultError,
    ) -> Result<(), KeyVaultError>;
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Cache performance and health statistics
///
/// See specs/interfaces/key-vault.md
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

/// Secret metadata for observability and management
///
/// See specs/interfaces/key-vault.md
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

/// Key Vault health status for monitoring
///
/// See specs/interfaces/key-vault.md
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
///
/// See specs/interfaces/key-vault.md
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
///
/// See specs/interfaces/key-vault.md
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

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Key Vault operations
///
/// See specs/interfaces/key-vault.md
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
            KeyVaultError::ServiceUnavailable { .. }
                | KeyVaultError::Timeout { .. }
                | KeyVaultError::RateLimitExceeded { .. }
                | KeyVaultError::Internal { .. }
        )
    }

    /// Get retry delay for transient errors
    pub fn get_retry_delay(&self) -> Option<Duration> {
        match self {
            KeyVaultError::RateLimitExceeded {
                retry_after_seconds,
            } => Some(Duration::from_secs(*retry_after_seconds)),
            KeyVaultError::ServiceUnavailable { .. } => Some(Duration::from_secs(30)),
            KeyVaultError::Timeout { .. } => Some(Duration::from_secs(5)),
            _ => None,
        }
    }

    /// Check if error indicates permission problems
    pub fn is_permission_error(&self) -> bool {
        matches!(
            self,
            KeyVaultError::AccessDenied { .. } | KeyVaultError::AuthenticationFailed { .. }
        )
    }
}

// ============================================================================
// Standard Secret Names
// ============================================================================

/// Standard secret names used by Queue-Keeper
///
/// See specs/interfaces/key-vault.md
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

// ============================================================================
// Default Implementations (Stubs)
// ============================================================================

/// Default Key Vault provider implementation
///
/// See specs/interfaces/key-vault.md
pub struct DefaultKeyVaultProvider {
    #[allow(dead_code)]
    config: KeyVaultConfiguration,
    cache: Box<dyn SecretCache>,
}

impl DefaultKeyVaultProvider {
    /// Create new Key Vault provider
    pub fn new(config: KeyVaultConfiguration, cache: Box<dyn SecretCache>) -> Self {
        Self { config, cache }
    }
}

#[async_trait]
impl KeyVaultProvider for DefaultKeyVaultProvider {
    async fn get_secret(&self, _name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn get_secret_with_version(
        &self,
        _name: &SecretName,
    ) -> Result<(SecretValue, String), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn refresh_secret(&self, _name: &SecretName) -> Result<SecretValue, KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn secret_exists(&self, _name: &SecretName) -> Result<bool, KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn list_secret_names(&self) -> Result<Vec<SecretName>, KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn clear_cache(&self, _name: &SecretName) -> Result<(), KeyVaultError> {
        self.cache.remove(_name).await
    }

    async fn clear_all_cache(&self) -> Result<(), KeyVaultError> {
        self.cache.clear().await
    }

    async fn get_cache_stats(&self) -> Result<CacheStatistics, KeyVaultError> {
        self.cache.get_statistics().await
    }
}

/// Default secret cache implementation
///
/// See specs/interfaces/key-vault.md
pub struct DefaultSecretCache {
    // TODO: Implement with secure concurrent HashMap
}

impl DefaultSecretCache {
    /// Create new secret cache
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for DefaultSecretCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretCache for DefaultSecretCache {
    async fn get(&self, _name: &SecretName) -> Option<CachedSecret> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn put(
        &self,
        _name: SecretName,
        _value: SecretValue,
        _ttl: Duration,
    ) -> Result<(), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn put_with_version(
        &self,
        _name: SecretName,
        _value: SecretValue,
        _version: String,
        _ttl: Duration,
    ) -> Result<(), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn remove(&self, _name: &SecretName) -> Result<(), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn clear(&self) -> Result<(), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn get_expiring_secrets(
        &self,
        _threshold: Duration,
    ) -> Result<Vec<SecretName>, KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn cleanup_expired(&self) -> Result<usize, KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn get_statistics(&self) -> Result<CacheStatistics, KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }
}

/// Default secret rotation handler
///
/// See specs/interfaces/key-vault.md
pub struct DefaultSecretRotationHandler;

#[async_trait]
impl SecretRotationHandler for DefaultSecretRotationHandler {
    async fn on_secret_rotated(
        &self,
        _name: &SecretName,
        _old_version: Option<String>,
        _new_version: String,
    ) -> Result<(), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn on_secret_expiring(
        &self,
        _name: &SecretName,
        _expires_in: Duration,
    ) -> Result<(), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }

    async fn on_secret_unavailable(
        &self,
        _name: &SecretName,
        _error: &KeyVaultError,
    ) -> Result<(), KeyVaultError> {
        unimplemented!("See specs/interfaces/key-vault.md")
    }
}

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
            expires_at: now.add_seconds(300),           // 5 minutes
            extended_expires_at: now.add_seconds(3600), // 1 hour
            version: Some("v1".to_string()),
        };

        assert!(!cached.is_expired()); // Should not be expired immediately
        assert!(!cached.is_extended_expired());
    }

    #[test]
    fn test_standard_secrets() {
        let webhook_secret = StandardSecrets::github_webhook_secret("prod").unwrap();
        assert_eq!(webhook_secret.as_str(), "queue-keeper-prod-github-webhook");

        let db_conn = StandardSecrets::database_connection("dev").unwrap();
        assert_eq!(db_conn.as_str(), "queue-keeper-dev-database-conn");
    }

    #[test]
    fn test_keyvault_error_transient() {
        assert!(KeyVaultError::ServiceUnavailable {
            message: "test".to_string()
        }
        .is_transient());

        assert!(!KeyVaultError::SecretNotFound {
            name: SecretName::new("test").unwrap()
        }
        .is_transient());
    }

    #[test]
    fn test_keyvault_error_retry_delay() {
        let rate_limit_error = KeyVaultError::RateLimitExceeded {
            retry_after_seconds: 60,
        };
        assert_eq!(
            rate_limit_error.get_retry_delay(),
            Some(Duration::from_secs(60))
        );

        let not_found_error = KeyVaultError::SecretNotFound {
            name: SecretName::new("test").unwrap(),
        };
        assert_eq!(not_found_error.get_retry_delay(), None);
    }
}
