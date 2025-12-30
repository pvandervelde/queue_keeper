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
        // Load required environment variables
        let vault_url =
            std::env::var("AZURE_KEY_VAULT_URL").map_err(|_| AzureConfigError::MissingEnvVar {
                variable: "AZURE_KEY_VAULT_URL".to_string(),
            })?;

        let storage_account = std::env::var("AZURE_STORAGE_ACCOUNT").map_err(|_| {
            AzureConfigError::MissingEnvVar {
                variable: "AZURE_STORAGE_ACCOUNT".to_string(),
            }
        })?;

        let storage_container = std::env::var("AZURE_STORAGE_CONTAINER").map_err(|_| {
            AzureConfigError::MissingEnvVar {
                variable: "AZURE_STORAGE_CONTAINER".to_string(),
            }
        })?;

        let servicebus_namespace = std::env::var("AZURE_SERVICEBUS_NAMESPACE").map_err(|_| {
            AzureConfigError::MissingEnvVar {
                variable: "AZURE_SERVICEBUS_NAMESPACE".to_string(),
            }
        })?;

        let appinsights_connection_string = std::env::var("AZURE_APPINSIGHTS_CONNECTION_STRING")
            .map_err(|_| AzureConfigError::MissingEnvVar {
                variable: "AZURE_APPINSIGHTS_CONNECTION_STRING".to_string(),
            })?;

        let environment =
            std::env::var("AZURE_ENVIRONMENT").map_err(|_| AzureConfigError::MissingEnvVar {
                variable: "AZURE_ENVIRONMENT".to_string(),
            })?;

        let region =
            std::env::var("AZURE_REGION").map_err(|_| AzureConfigError::MissingEnvVar {
                variable: "AZURE_REGION".to_string(),
            })?;

        // Determine if we're in production based on environment
        let is_production = environment == "production";

        // Create configuration
        let config = Self {
            key_vault: if is_production {
                AzureKeyVaultConfig::production(vault_url)
            } else {
                AzureKeyVaultConfig::development(vault_url)
            },
            blob_storage: if is_production {
                AzureBlobStorageConfig::production(storage_account, storage_container)
            } else {
                // Development might have connection string
                let conn_str = std::env::var("AZURE_STORAGE_CONNECTION_STRING").ok();
                if let Some(conn_str) = conn_str {
                    AzureBlobStorageConfig::development(
                        storage_account,
                        storage_container,
                        conn_str,
                    )
                } else {
                    AzureBlobStorageConfig::production(storage_account, storage_container)
                }
            },
            service_bus: if is_production {
                AzureServiceBusConfig::production(servicebus_namespace)
            } else {
                // Development might have connection string
                let conn_str = std::env::var("AZURE_SERVICEBUS_CONNECTION_STRING").ok();
                if let Some(conn_str) = conn_str {
                    AzureServiceBusConfig::development(servicebus_namespace, conn_str)
                } else {
                    AzureServiceBusConfig::production(servicebus_namespace)
                }
            },
            telemetry: if is_production {
                let version =
                    std::env::var("SERVICE_VERSION").unwrap_or_else(|_| "1.0.0".to_string());
                AzureTelemetryConfig::production(appinsights_connection_string, version)
            } else {
                AzureTelemetryConfig::development(appinsights_connection_string)
            },
            environment,
            region,
        };

        // Validate before returning
        config.validate()?;

        Ok(config)
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
        // Validate Key Vault URL
        if !self.key_vault.vault_url.starts_with("https://") {
            return Err(AzureConfigError::InvalidKeyVaultUrl {
                url: self.key_vault.vault_url.clone(),
                reason: "Key Vault URL must use HTTPS".to_string(),
            });
        }

        if !self.key_vault.vault_url.contains(".vault.azure.net") {
            return Err(AzureConfigError::InvalidKeyVaultUrl {
                url: self.key_vault.vault_url.clone(),
                reason: "Key Vault URL must end with .vault.azure.net".to_string(),
            });
        }

        // Validate storage account name
        let account_name = &self.blob_storage.account_name;
        if account_name.len() < 3 || account_name.len() > 24 {
            return Err(AzureConfigError::InvalidStorageAccount {
                name: account_name.clone(),
                reason: "Storage account name must be 3-24 characters".to_string(),
            });
        }

        if !account_name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(AzureConfigError::InvalidStorageAccount {
                name: account_name.clone(),
                reason: "Storage account name must contain only lowercase letters and numbers"
                    .to_string(),
            });
        }

        // Validate Service Bus namespace
        let namespace = &self.service_bus.namespace;
        if namespace.is_empty() || namespace.len() > 50 {
            return Err(AzureConfigError::InvalidServiceBusNamespace {
                namespace: namespace.clone(),
                reason: "Service Bus namespace must be 1-50 characters".to_string(),
            });
        }

        // Validate environment
        if !["dev", "staging", "production"].contains(&self.environment.as_str()) {
            return Err(AzureConfigError::InvalidEnvironment {
                environment: self.environment.clone(),
            });
        }

        // Validate region (basic check - Azure has many regions)
        if self.region.is_empty() {
            return Err(AzureConfigError::InvalidRegion {
                region: self.region.clone(),
            });
        }

        // Valid Azure regions (common ones for validation)
        let valid_regions = [
            "eastus",
            "eastus2",
            "westus",
            "westus2",
            "westus3",
            "centralus",
            "northcentralus",
            "southcentralus",
            "westcentralus",
            "canadacentral",
            "canadaeast",
            "brazilsouth",
            "northeurope",
            "westeurope",
            "uksouth",
            "ukwest",
            "francecentral",
            "francesouth",
            "germanywestcentral",
            "norwayeast",
            "switzerlandnorth",
            "swedencentral",
            "eastasia",
            "southeastasia",
            "japaneast",
            "japanwest",
            "australiaeast",
            "australiasoutheast",
            "australiacentral",
            "centralindia",
            "southindia",
            "westindia",
            "koreacentral",
            "koreasouth",
        ];

        if !valid_regions.contains(&self.region.as_str()) {
            return Err(AzureConfigError::InvalidRegion {
                region: self.region.clone(),
            });
        }

        Ok(())
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
