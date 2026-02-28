//! Configuration types for the HTTP service

use crate::errors::ConfigError;
use queue_keeper_core::webhook::generic_provider::{
    GenericProviderConfig, GenericProviderConfigError,
};
use serde::{Deserialize, Serialize};

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceConfig {
    /// HTTP server settings
    pub server: ServerConfig,

    /// Webhook processing settings
    pub webhooks: WebhookConfig,

    /// Security settings
    pub security: SecurityConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Per-provider webhook configuration.
    ///
    /// Each entry registers one webhook provider (e.g. `github`) with its
    /// own secret source and validation rules. An empty list is valid;
    /// requests to unknown providers will receive `404 Not Found`.
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,

    /// Configuration-driven generic webhook providers.
    ///
    /// Each entry registers a non-GitHub provider (e.g. `jira`, `slack`)
    /// using [`GenericProviderConfig`]. These providers are fully
    /// configuration-driven — no Rust code changes are needed to add
    /// a new source. An empty list is valid.
    ///
    /// Provider IDs in this list must be unique and must not conflict
    /// with IDs in the [`providers`](Self::providers) list.
    #[serde(default)]
    pub generic_providers: Vec<GenericProviderConfig>,
}

impl ServiceConfig {
    /// Validate the service configuration for consistency and correctness.
    ///
    /// This should be called once at startup before the service is marked
    /// ready. It checks:
    ///
    /// - Provider IDs are non-empty and URL-safe (`[a-z0-9\-_]`)
    /// - Provider IDs are unique across all entries
    /// - Providers with `require_signature: true` supply a secret source
    /// - Secret sources are internally valid (e.g. non-empty Key Vault names)
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::ProviderValidation`] describing the first
    /// validation failure encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_api::config::ServiceConfig;
    ///
    /// let config = ServiceConfig::default();
    /// assert!(config.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate each provider individually
        for provider in &self.providers {
            provider.validate()?;
        }

        // Validate each generic provider individually
        for generic in &self.generic_providers {
            generic.validate().map_err(|e| match e {
                GenericProviderConfigError::InvalidProviderId { message } => {
                    ConfigError::ProviderValidation { message }
                }
                other => ConfigError::ProviderValidation {
                    message: other.to_string(),
                },
            })?;
        }

        // Detect duplicate provider IDs (across both lists)
        let mut seen = std::collections::HashSet::new();
        for provider in &self.providers {
            if !seen.insert(provider.id.as_str()) {
                return Err(ConfigError::ProviderValidation {
                    message: format!(
                        "duplicate provider ID '{}': each provider ID must be unique",
                        provider.id
                    ),
                });
            }
        }
        for generic in &self.generic_providers {
            if !seen.insert(generic.provider_id.as_str()) {
                return Err(ConfigError::ProviderValidation {
                    message: format!(
                        "duplicate provider ID '{}': each provider ID must be unique across providers and generic_providers",
                        generic.provider_id
                    ),
                });
            }
        }

        Ok(())
    }
}

// ============================================================================
// Provider Configuration
// ============================================================================

/// Configuration for a single webhook provider.
///
/// Each provider corresponds to one URL path segment, e.g. a provider
/// with `id = "github"` handles webhooks at `POST /webhook/github`.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_api::config::{ProviderConfig, ProviderSecretConfig};
///
/// let config = ProviderConfig {
///     id: "github".to_string(),
///     require_signature: true,
///     secret: Some(ProviderSecretConfig::KeyVault {
///         secret_name: "github-webhook-secret".to_string(),
///     }),
///     allowed_event_types: vec![],
/// };
/// assert!(config.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// URL-safe provider identifier.
    ///
    /// Must match `[a-z0-9\-_]+` (non-empty, no uppercase, no slashes).
    /// This value appears verbatim in the URL path: `/webhook/{id}`.
    pub id: String,

    /// Whether incoming requests must carry a valid HMAC-SHA256 signature.
    ///
    /// When `true`, a `secret` source must also be provided.
    /// Defaults to `true`.
    #[serde(default = "default_require_signature")]
    pub require_signature: bool,

    /// Source for the HMAC-SHA256 webhook secret.
    ///
    /// Required when `require_signature` is `true`.
    #[serde(default)]
    pub secret: Option<ProviderSecretConfig>,

    /// Allowlist of event types this provider accepts.
    ///
    /// An empty list means all event types are accepted. Non-empty lists
    /// cause requests with unlisted event types to be rejected.
    #[serde(default)]
    pub allowed_event_types: Vec<String>,
}

fn default_require_signature() -> bool {
    true
}

impl ProviderConfig {
    /// Validate this provider configuration entry.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::ProviderValidation`] when:
    /// - `id` is empty
    /// - `id` contains characters outside `[a-z0-9\-_]`
    /// - `require_signature` is `true` but `secret` is `None`
    /// - The `secret` source is internally invalid (e.g. empty Key Vault name)
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate provider ID format by delegating to ProviderId::new().
        // This ensures a single source of truth for the allowed character set
        // across both configuration parsing and runtime registry lookups.
        crate::provider_registry::ProviderId::new(&self.id).map_err(|e| {
            ConfigError::ProviderValidation {
                message: format!("invalid provider ID '{}': {}", self.id, e),
            }
        })?;

        // Signature consistency
        if self.require_signature && self.secret.is_none() {
            return Err(ConfigError::ProviderValidation {
                message: format!(
                    "provider '{}': require_signature is true but no secret source is configured",
                    self.id
                ),
            });
        }

        // Validate secret source if present
        if let Some(secret) = &self.secret {
            secret.validate(&self.id)?;
        }

        Ok(())
    }
}

/// Source for a provider's HMAC-SHA256 webhook secret.
///
/// In production deployments, always use [`ProviderSecretConfig::KeyVault`]
/// to avoid embedding secrets in configuration files or environment variables.
///
/// # Security
///
/// [`ProviderSecretConfig::Literal`] is provided for development and testing
/// only. It emits a `warn!` at startup. Never commit literal secrets to source control.
///
/// When serialized (e.g. via `/admin/config`), the `Literal` value is always
/// replaced with `"<REDACTED>"` to prevent secret leakage in API responses.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ProviderSecretConfig {
    /// Secret stored in Azure Key Vault.
    ///
    /// `secret_name` is the name of the secret inside the vault (not the
    /// vault URL, which is configured separately in `AzureKeyVaultConfig`).
    KeyVault {
        /// Name of the secret in Azure Key Vault.
        secret_name: String,
    },

    /// Literal secret value embedded in the configuration.
    ///
    /// **For development and testing only.** Never use in production.
    Literal {
        /// The raw secret value.
        ///
        /// This field is excluded from `Debug` output to prevent accidental
        /// leakage in logs.
        #[serde(rename = "value")]
        value: String,
    },
}

impl serde::Serialize for ProviderSecretConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Self::KeyVault { secret_name } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "key_vault")?;
                map.serialize_entry("secret_name", secret_name)?;
                map.end()
            }
            Self::Literal { .. } => {
                // Never serialize the raw secret value — replace with a
                // redaction placeholder so the /admin/config endpoint does
                // not leak secrets embedded in the configuration file.
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "literal")?;
                map.serialize_entry("value", "<REDACTED>")?;
                map.end()
            }
        }
    }
}

impl std::fmt::Debug for ProviderSecretConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyVault { secret_name } => f
                .debug_struct("KeyVault")
                .field("secret_name", secret_name)
                .finish(),
            Self::Literal { .. } => f
                .debug_struct("Literal")
                .field("value", &"<REDACTED>")
                .finish(),
        }
    }
}

impl ProviderSecretConfig {
    /// Validate this secret source for the given provider ID.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::ProviderValidation`] when:
    /// - `KeyVault.secret_name` is empty
    /// - `Literal.value` is empty
    pub fn validate(&self, provider_id: &str) -> Result<(), ConfigError> {
        match self {
            Self::KeyVault { secret_name } => {
                if secret_name.is_empty() {
                    return Err(ConfigError::ProviderValidation {
                        message: format!(
                            "provider '{}': key_vault.secret_name must not be empty",
                            provider_id
                        ),
                    });
                }
            }
            Self::Literal { value } => {
                if value.is_empty() {
                    return Err(ConfigError::ProviderValidation {
                        message: format!(
                            "provider '{}': literal.value must not be empty",
                            provider_id
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Graceful shutdown timeout in seconds
    pub shutdown_timeout_seconds: u64,

    /// Maximum request size in bytes
    pub max_body_size: usize,

    /// Enable CORS
    pub enable_cors: bool,

    /// Enable compression
    pub enable_compression: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            timeout_seconds: 30,
            shutdown_timeout_seconds: 30,
            max_body_size: 10 * 1024 * 1024, // 10MB
            enable_cors: true,
            enable_compression: true,
        }
    }
}

/// Global webhook processing configuration.
///
/// These settings apply across all providers as service-wide defaults.
///
/// # Relationship to [`ProviderConfig`]
///
/// [`ProviderConfig`] introduces per-provider overrides for
/// `require_signature` and `allowed_event_types`. When a matching
/// [`ProviderConfig`] entry exists, its values take precedence over
/// the global defaults defined here. [`WebhookConfig`] is retained
/// for backward compatibility and for settings that do not yet have
/// a per-provider equivalent (e.g. `store_payloads`, `rate_limit_per_repo`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook endpoint path
    pub endpoint_path: String,

    /// Require signature validation (global default; overridden per-provider by [`ProviderConfig::require_signature`])
    pub require_signature: bool,

    /// Enable payload storage for audit
    pub store_payloads: bool,

    /// Supported event types — global default (empty = all).
    /// Per-provider filtering is configured via [`ProviderConfig::allowed_event_types`].
    pub allowed_event_types: Vec<String>,

    /// Maximum events per repository per minute
    pub rate_limit_per_repo: Option<u32>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            endpoint_path: "/webhook".to_string(),
            require_signature: true,
            store_payloads: true,
            allowed_event_types: vec![], // All events allowed by default
            rate_limit_per_repo: Some(100), // 100 events per minute per repo
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable request rate limiting
    pub enable_rate_limiting: bool,

    /// Global rate limit (requests per minute)
    pub global_rate_limit: u32,

    /// Enable IP-based rate limiting
    pub enable_ip_rate_limiting: bool,

    /// IP rate limit (requests per minute per IP)
    pub ip_rate_limit: u32,

    /// Enable request logging
    pub log_requests: bool,

    /// Log request bodies (security risk)
    pub log_request_bodies: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_rate_limiting: true,
            global_rate_limit: 1000,
            enable_ip_rate_limiting: true,
            ip_rate_limit: 100,
            log_requests: true,
            log_request_bodies: false,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Logging level
    pub level: String,

    /// Enable JSON structured logging
    pub json_format: bool,

    /// Log file path (optional)
    pub file_path: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            file_path: None,
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
