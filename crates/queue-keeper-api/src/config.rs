//! Configuration types for the HTTP service

use crate::azure_config::AzureKeyVaultConfig;
use crate::errors::ConfigError;
use queue_keeper_core::webhook::generic_provider::{
    GenericProviderConfig, GenericProviderConfigError, WebhookSecretConfig,
};
use serde::{Deserialize, Serialize};

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceConfig {
    /// HTTP server settings
    #[serde(default)]
    pub server: ServerConfig,

    /// Webhook processing settings
    #[serde(default)]
    pub webhooks: WebhookConfig,

    /// Security settings
    #[serde(default)]
    pub security: SecurityConfig,

    /// Logging configuration
    #[serde(default)]
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

    /// Azure Key Vault configuration.
    ///
    /// Required when any provider in [`providers`](Self::providers) or
    /// [`generic_providers`](Self::generic_providers) is configured with
    /// [`ProviderSecretConfig::KeyVault`] or [`WebhookSecretConfig::KeyVault`].
    /// The `vault_url` field must be a non-empty Azure Key Vault URL
    /// (e.g. `https://my-vault.vault.azure.net`).
    ///
    /// When absent, Key Vault–backed providers cannot be used and service
    /// startup will fail if any provider requests Key Vault secrets.
    #[serde(default)]
    pub key_vault: Option<AzureKeyVaultConfig>,

    /// Queue backend provider configuration.
    ///
    /// Selects and configures the message queue used for routing processed
    /// webhook events to bot queues. Defaults to `in_memory` when absent,
    /// which is suitable for development only — events are not persisted
    /// across restarts.
    ///
    /// See [`QueueBackendConfig`] for the full set of providers and their
    /// YAML shapes.
    #[serde(default)]
    pub queue: QueueBackendConfig,
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

        // Verify Key Vault configuration is present when any provider requires it.
        let needs_key_vault = self
            .providers
            .iter()
            .any(|p| matches!(&p.secret, Some(ProviderSecretConfig::KeyVault { .. })))
            || self.generic_providers.iter().any(|p| {
                matches!(
                    &p.webhook_secret,
                    Some(WebhookSecretConfig::KeyVault { .. })
                )
            });

        if needs_key_vault {
            match &self.key_vault {
                None => {
                    return Err(ConfigError::ProviderValidation {
                        message: "one or more providers use Key Vault secrets but no \
                                  `key_vault` configuration section is present"
                            .to_string(),
                    });
                }
                Some(kv) if kv.vault_url.is_empty() => {
                    return Err(ConfigError::ProviderValidation {
                        message: "`key_vault.vault_url` must not be empty when \
                                  Key Vault secrets are in use"
                            .to_string(),
                    });
                }
                Some(kv) if !kv.vault_url.starts_with("https://") => {
                    return Err(ConfigError::ProviderValidation {
                        message: format!(
                            "`key_vault.vault_url` must use HTTPS (got '{}')",
                            kv.vault_url
                        ),
                    });
                }
                Some(_) => {} // valid
            }
        }

        // Validate queue backend configuration
        self.queue
            .validate()
            .map_err(|msg| ConfigError::ProviderValidation { message: msg })?;

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
/// [`ProviderConfig`] holds per-provider settings such as `allowed_event_types`.
/// The routing handler enforces `allowed_event_types` from the matching
/// [`ProviderConfig`] entry when present. [`WebhookConfig`] is retained for
/// settings that do not yet have a per-provider equivalent
/// (e.g. `store_payloads`, `rate_limit_per_repo`).
///
/// > **Note**: `require_signature` in `WebhookConfig` and `ProviderConfig` is
/// > **not** enforced by the routing layer. Signature validation is delegated
/// > entirely to the processor's [`SignatureValidator`]. The field is present for
/// > documentation and future use only.
///
/// [`SignatureValidator`]: queue_keeper_core::webhook::SignatureValidator
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
#[derive(Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable request rate limiting
    #[serde(default = "SecurityConfig::default_enable_rate_limiting")]
    pub enable_rate_limiting: bool,

    /// Global rate limit (requests per minute)
    #[serde(default = "SecurityConfig::default_global_rate_limit")]
    pub global_rate_limit: u32,

    /// Enable IP-based rate limiting
    #[serde(default = "SecurityConfig::default_enable_ip_rate_limiting")]
    pub enable_ip_rate_limiting: bool,

    /// IP rate limit (requests per minute per IP)
    #[serde(default = "SecurityConfig::default_ip_rate_limit")]
    pub ip_rate_limit: u32,

    /// Maximum number of authentication failures from a single IP before it
    /// is rate-limited. Defaults to 10 (spec assertion #19).
    ///
    /// Configure via `QK__SECURITY__AUTH_FAILURE_THRESHOLD`.
    #[serde(default = "SecurityConfig::default_auth_failure_threshold")]
    pub auth_failure_threshold: usize,

    /// Duration of the sliding window for authentication failure counting,
    /// in seconds. Defaults to 300 (5 minutes, spec assertion #19).
    ///
    /// Configure via `QK__SECURITY__AUTH_FAILURE_WINDOW_SECS`.
    #[serde(default = "SecurityConfig::default_auth_failure_window_secs")]
    pub auth_failure_window_secs: u64,

    /// Enable request logging
    #[serde(default = "SecurityConfig::default_log_requests")]
    pub log_requests: bool,

    /// Log request bodies (security risk)
    #[serde(default)]
    pub log_request_bodies: bool,

    /// API key required for admin endpoints (`/admin/**`).
    ///
    /// When `Some`, every request to an admin endpoint must supply a matching
    /// `Authorization: Bearer <key>` header. When `None`, admin endpoints are
    /// accessible without authentication (suitable for development only).
    ///
    /// In production deployments set this via the `QK__SECURITY__ADMIN_API_KEY`
    /// environment variable; do not store the key in committed YAML files.
    ///
    /// This field is intentionally excluded from serialization so it is never
    /// returned by the `/admin/config` endpoint or written to log output.
    #[serde(default, skip_serializing)]
    pub admin_api_key: Option<String>,
}

impl std::fmt::Debug for SecurityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityConfig")
            .field("enable_rate_limiting", &self.enable_rate_limiting)
            .field("global_rate_limit", &self.global_rate_limit)
            .field("enable_ip_rate_limiting", &self.enable_ip_rate_limiting)
            .field("ip_rate_limit", &self.ip_rate_limit)
            .field("auth_failure_threshold", &self.auth_failure_threshold)
            .field("auth_failure_window_secs", &self.auth_failure_window_secs)
            .field("log_requests", &self.log_requests)
            .field("log_request_bodies", &self.log_request_bodies)
            .field(
                "admin_api_key",
                &self.admin_api_key.as_ref().map(|_| "<REDACTED>"),
            )
            .finish()
    }
}

impl SecurityConfig {
    fn default_enable_rate_limiting() -> bool {
        true
    }

    fn default_global_rate_limit() -> u32 {
        1000
    }

    fn default_enable_ip_rate_limiting() -> bool {
        true
    }

    fn default_ip_rate_limit() -> u32 {
        100
    }

    fn default_log_requests() -> bool {
        true
    }

    fn default_auth_failure_threshold() -> usize {
        10
    }

    fn default_auth_failure_window_secs() -> u64 {
        300
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_rate_limiting: true,
            global_rate_limit: 1000,
            enable_ip_rate_limiting: true,
            ip_rate_limit: 100,
            auth_failure_threshold: SecurityConfig::default_auth_failure_threshold(),
            auth_failure_window_secs: SecurityConfig::default_auth_failure_window_secs(),
            log_requests: true,
            log_request_bodies: false,
            admin_api_key: None,
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

// ============================================================================
// Queue Backend Configuration
// ============================================================================

/// Queue backend provider selection and configuration.
///
/// Controls which message queue provider the service uses for routing
/// processed webhook events to bot queues. When absent the in-memory
/// provider is used (development / testing only — data is lost on restart).
///
/// # YAML examples
///
/// ```yaml
/// # Azure Service Bus — managed identity (production)
/// queue:
///   provider: azure_service_bus
///   namespace: mybus.servicebus.windows.net
///   use_sessions: true
///
/// # Azure Service Bus — connection string (dev/test only)
/// queue:
///   provider: azure_service_bus
///   connection_string: "Endpoint=sb://mybus.servicebus.windows.net/;SharedAccessKeyName=..."
///
/// # AWS SQS (production — uses IAM role credential chain)
/// queue:
///   provider: aws_sqs
///   region: us-east-1
///   use_fifo_queues: true
///
/// # In-memory (development only)
/// queue:
///   provider: in_memory
/// ```
///
/// # Production guidance
///
/// Always configure a durable provider in production:
/// - **Azure Service Bus**: omit `connection_string`, set `namespace`, and assign
///   the Service Bus Data Sender/Receiver role to the pod's managed identity.
/// - **AWS SQS**: omit explicit credentials, attach an IAM role with
///   `sqs:SendMessage` and `sqs:ReceiveMessage` permissions to the workload.
///
/// The in-memory backend must not be used in production because events are not
/// persisted across restarts and no dead-letter queue semantics are guaranteed.
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum QueueBackendConfig {
    /// In-memory provider. **Development and testing only.**
    ///
    /// A `WARN` is emitted at startup when this variant is active.
    /// Events are not persisted; all data is lost on restart.
    #[serde(rename = "in_memory")]
    InMemory {
        /// Maximum number of messages held per queue. Defaults to 10 000.
        #[serde(default)]
        max_queue_size: Option<usize>,
    },

    /// Azure Service Bus.
    ///
    /// Provide either `namespace` (managed identity / DefaultCredential) or
    /// `connection_string` (dev/test SharedAccessKey auth, emits `WARN`).
    /// `namespace` is required when `connection_string` is absent.
    #[serde(rename = "azure_service_bus")]
    AzureServiceBus {
        /// Fully-qualified Service Bus namespace hostname.
        ///
        /// e.g. `mybus.servicebus.windows.net`
        ///
        /// Required when `connection_string` is absent. The credential used
        /// is the DefaultAzureCredential chain: managed identity → env vars
        /// (`AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET`) →
        /// Azure CLI.
        #[serde(default)]
        namespace: Option<String>,

        /// Connection string (development / testing only).
        ///
        /// When present overrides `namespace` and uses SharedAccessKey auth.
        /// **Never use in production.** A `WARN` is emitted at startup.
        ///
        /// Excluded from serialization (e.g. `/admin/config` response) so
        /// the secret key embedded in the string is never returned by the API.
        #[serde(default, skip_serializing)]
        connection_string: Option<String>,

        /// Enable session-ordered delivery (default: `true`).
        ///
        /// Must match the session enablement setting on the target queues.
        /// When `true`, messages for the same session are delivered in order.
        #[serde(default = "default_queue_true")]
        use_sessions: bool,

        /// Session lock duration in seconds (default: 300 = 5 minutes).
        #[serde(default)]
        session_timeout_seconds: Option<u64>,
    },

    /// AWS SQS.
    ///
    /// Uses the standard AWS credential chain: IAM role, `AWS_ACCESS_KEY_ID` /
    /// `AWS_SECRET_ACCESS_KEY` environment variables, or `~/.aws/credentials`.
    /// In production, attach an IAM role with `sqs:SendMessage` and
    /// `sqs:ReceiveMessage` permissions to the compute resource.
    #[serde(rename = "aws_sqs")]
    AwsSqs {
        /// AWS region (e.g. `us-east-1`).
        region: String,

        /// Use FIFO queues for ordered delivery (default: `true`).
        ///
        /// FIFO queues enforce strict ordering at the cost of lower
        /// throughput. Required for session-ordered bot delivery.
        #[serde(default = "default_queue_true")]
        use_fifo_queues: bool,
    },
}

fn default_queue_true() -> bool {
    true
}

impl QueueBackendConfig {
    /// Validate the queue backend configuration for completeness.
    ///
    /// Returns an error string describing the first problem found.
    ///
    /// # Errors
    ///
    /// - `AzureServiceBus` with neither `namespace` nor `connection_string` set.
    /// - `AwsSqs` with an empty `region`.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::InMemory { .. } => Ok(()),
            Self::AzureServiceBus {
                namespace,
                connection_string,
                ..
            } => {
                if namespace.is_none() && connection_string.is_none() {
                    return Err(
                        "queue.azure_service_bus: either `namespace` or `connection_string` \
                         must be provided"
                            .to_string(),
                    );
                }
                Ok(())
            }
            Self::AwsSqs { region, .. } => {
                if region.is_empty() {
                    return Err("queue.aws_sqs: `region` must not be empty".to_string());
                }
                Ok(())
            }
        }
    }
}

impl Default for QueueBackendConfig {
    fn default() -> Self {
        Self::InMemory {
            max_queue_size: None,
        }
    }
}

/// Secret values in `QueueBackendConfig` are redacted in `Debug` output.
impl std::fmt::Debug for QueueBackendConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InMemory { max_queue_size } => f
                .debug_struct("InMemory")
                .field("max_queue_size", max_queue_size)
                .finish(),
            Self::AzureServiceBus {
                namespace,
                connection_string,
                use_sessions,
                session_timeout_seconds,
            } => f
                .debug_struct("AzureServiceBus")
                .field("namespace", namespace)
                .field(
                    "connection_string",
                    &connection_string.as_ref().map(|_| "<REDACTED>"),
                )
                .field("use_sessions", use_sessions)
                .field("session_timeout_seconds", session_timeout_seconds)
                .finish(),
            Self::AwsSqs {
                region,
                use_fifo_queues,
            } => f
                .debug_struct("AwsSqs")
                .field("region", region)
                .field("use_fifo_queues", use_fifo_queues)
                .finish(),
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
