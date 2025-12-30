//! Azure production configuration
//!
//! Secure configuration management for Azure deployment with:
//! - Environment variable loading
//! - Secret redaction in Debug output
//! - Managed Identity support
//! - Configuration validation at startup
//!
//! See specs/design/configuration.md for complete specification.

use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(test)]
#[path = "azure_config_tests.rs"]
mod tests;

/// Azure production configuration
///
/// Loaded from environment variables for secure deployment.
/// Supports Managed Identity authentication (no connection strings in code).
#[derive(Clone, Serialize, Deserialize)]
pub struct AzureProductionConfig {
    /// Azure Key Vault configuration
    pub key_vault: AzureKeyVaultConfig,

    /// Azure Blob Storage configuration
    pub blob_storage: AzureBlobStorageConfig,

    /// Azure Service Bus configuration
    pub service_bus: AzureServiceBusConfig,

    /// Azure Monitor telemetry configuration
    pub telemetry: AzureTelemetryConfig,

    /// Application environment (dev, staging, production)
    pub environment: String,

    /// Azure region for resources
    pub region: String,
}

impl AzureProductionConfig {
    /// Load configuration from environment variables
    ///
    /// Expected environment variables:
    /// - `AZURE_KEY_VAULT_URL`: Key Vault URL
    /// - `AZURE_STORAGE_ACCOUNT`: Storage account name
    /// - `AZURE_STORAGE_CONTAINER`: Blob container name
    /// - `AZURE_SERVICEBUS_NAMESPACE`: Service Bus namespace
    /// - `AZURE_APPINSIGHTS_CONNECTION_STRING`: Application Insights connection string
    /// - `AZURE_ENVIRONMENT`: Environment name (dev, staging, production)
    /// - `AZURE_REGION`: Azure region
    ///
    /// # Errors
    /// Returns error if required variables are missing or invalid
    pub fn from_env() -> Result<Self, AzureConfigError> {
        // TODO: implement
        todo!()
    }

    /// Validate configuration
    ///
    /// Checks that all required fields are present and valid:
    /// - Key Vault URL is HTTPS and ends with .vault.azure.net
    /// - Storage account name is valid (3-24 chars, lowercase/numbers)
    /// - Service Bus namespace is valid
    /// - Environment is one of: dev, staging, production
    /// - Region is a valid Azure region
    pub fn validate(&self) -> Result<(), AzureConfigError> {
        // TODO: implement
        todo!()
    }
}

impl fmt::Debug for AzureProductionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureProductionConfig")
            .field("key_vault", &self.key_vault)
            .field("blob_storage", &self.blob_storage)
            .field("service_bus", &self.service_bus)
            .field("telemetry", &self.telemetry)
            .field("environment", &self.environment)
            .field("region", &self.region)
            .finish()
    }
}

/// Azure Key Vault configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct AzureKeyVaultConfig {
    /// Key Vault URL (e.g., https://my-vault.vault.azure.net/)
    pub vault_url: String,

    /// Use Managed Identity for authentication
    pub use_managed_identity: bool,

    /// Cache TTL for secrets in seconds
    pub cache_ttl_seconds: u64,
}

impl AzureKeyVaultConfig {
    /// Create production configuration with Managed Identity
    pub fn production(vault_url: String) -> Self {
        Self {
            vault_url,
            use_managed_identity: true,
            cache_ttl_seconds: 300, // 5 minutes per REQ-012
        }
    }

    /// Create development configuration
    pub fn development(vault_url: String) -> Self {
        Self {
            vault_url,
            use_managed_identity: false, // Use Azure CLI credentials
            cache_ttl_seconds: 60,       // 1 minute for faster testing
        }
    }
}

impl fmt::Debug for AzureKeyVaultConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureKeyVaultConfig")
            .field("vault_url", &self.vault_url)
            .field("use_managed_identity", &self.use_managed_identity)
            .field("cache_ttl_seconds", &self.cache_ttl_seconds)
            .finish()
    }
}

/// Azure Blob Storage configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct AzureBlobStorageConfig {
    /// Storage account name
    pub account_name: String,

    /// Blob container name
    pub container_name: String,

    /// Connection string (optional, for local development)
    pub connection_string: Option<String>,

    /// Use Managed Identity for authentication
    pub use_managed_identity: bool,
}

impl AzureBlobStorageConfig {
    /// Create production configuration with Managed Identity
    pub fn production(account_name: String, container_name: String) -> Self {
        Self {
            account_name,
            container_name,
            connection_string: None,
            use_managed_identity: true,
        }
    }

    /// Create development configuration with connection string
    pub fn development(
        account_name: String,
        container_name: String,
        connection_string: String,
    ) -> Self {
        Self {
            account_name,
            container_name,
            connection_string: Some(connection_string),
            use_managed_identity: false,
        }
    }
}

impl fmt::Debug for AzureBlobStorageConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureBlobStorageConfig")
            .field("account_name", &self.account_name)
            .field("container_name", &self.container_name)
            .field(
                "connection_string",
                if self.connection_string.is_some() {
                    &"<REDACTED>"
                } else {
                    &"None"
                },
            )
            .field("use_managed_identity", &self.use_managed_identity)
            .finish()
    }
}

/// Azure Service Bus configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct AzureServiceBusConfig {
    /// Service Bus namespace
    pub namespace: String,

    /// Connection string (optional, for local development)
    pub connection_string: Option<String>,

    /// Use Managed Identity for authentication
    pub use_managed_identity: bool,

    /// Enable sessions for ordered processing
    pub use_sessions: bool,

    /// Session timeout in seconds
    pub session_timeout_seconds: u64,
}

impl AzureServiceBusConfig {
    /// Create production configuration with Managed Identity
    pub fn production(namespace: String) -> Self {
        Self {
            namespace,
            connection_string: None,
            use_managed_identity: true,
            use_sessions: true,
            session_timeout_seconds: 300, // 5 minutes
        }
    }

    /// Create development configuration with connection string
    pub fn development(namespace: String, connection_string: String) -> Self {
        Self {
            namespace,
            connection_string: Some(connection_string),
            use_managed_identity: false,
            use_sessions: true,
            session_timeout_seconds: 300,
        }
    }
}

impl fmt::Debug for AzureServiceBusConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureServiceBusConfig")
            .field("namespace", &self.namespace)
            .field(
                "connection_string",
                if self.connection_string.is_some() {
                    &"<REDACTED>"
                } else {
                    &"None"
                },
            )
            .field("use_managed_identity", &self.use_managed_identity)
            .field("use_sessions", &self.use_sessions)
            .field("session_timeout_seconds", &self.session_timeout_seconds)
            .finish()
    }
}

/// Azure telemetry configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct AzureTelemetryConfig {
    /// Application Insights connection string
    pub connection_string: String,

    /// Enable distributed tracing
    pub enable_tracing: bool,

    /// Sampling ratio (0.0 - 1.0)
    pub sampling_ratio: f64,

    /// Service name for telemetry
    pub service_name: String,

    /// Service version
    pub service_version: String,
}

impl AzureTelemetryConfig {
    /// Create production configuration
    pub fn production(connection_string: String, service_version: String) -> Self {
        Self {
            connection_string,
            enable_tracing: true,
            sampling_ratio: 0.1, // 10% sampling for production
            service_name: "queue-keeper".to_string(),
            service_version,
        }
    }

    /// Create development configuration
    pub fn development(connection_string: String) -> Self {
        Self {
            connection_string,
            enable_tracing: true,
            sampling_ratio: 1.0, // 100% sampling for development
            service_name: "queue-keeper".to_string(),
            service_version: "dev".to_string(),
        }
    }
}

impl fmt::Debug for AzureTelemetryConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureTelemetryConfig")
            .field("connection_string", &"<REDACTED>")
            .field("enable_tracing", &self.enable_tracing)
            .field("sampling_ratio", &self.sampling_ratio)
            .field("service_name", &self.service_name)
            .field("service_version", &self.service_version)
            .finish()
    }
}

/// Azure configuration errors
#[derive(Debug, thiserror::Error)]
pub enum AzureConfigError {
    /// Environment variable missing
    #[error("Missing required environment variable: {variable}")]
    MissingEnvVar { variable: String },

    /// Environment variable has invalid value
    #[error("Invalid value for environment variable {variable}: {message}")]
    InvalidEnvVar { variable: String, message: String },

    /// Configuration validation failed
    #[error("Configuration validation failed: {message}")]
    ValidationFailed { message: String },

    /// Invalid Key Vault URL
    #[error("Invalid Key Vault URL: {url} - {reason}")]
    InvalidKeyVaultUrl { url: String, reason: String },

    /// Invalid storage account name
    #[error("Invalid storage account name: {name} - {reason}")]
    InvalidStorageAccount { name: String, reason: String },

    /// Invalid Service Bus namespace
    #[error("Invalid Service Bus namespace: {namespace} - {reason}")]
    InvalidServiceBusNamespace { namespace: String, reason: String },

    /// Invalid environment name
    #[error("Invalid environment: {environment} - must be one of: dev, staging, production")]
    InvalidEnvironment { environment: String },

    /// Invalid Azure region
    #[error("Invalid Azure region: {region}")]
    InvalidRegion { region: String },

    /// Invalid connection string
    #[error("Invalid connection string: {message}")]
    InvalidConnectionString { message: String },
}
