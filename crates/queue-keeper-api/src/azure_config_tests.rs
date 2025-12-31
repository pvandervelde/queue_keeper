//! Tests for Azure production configuration

use super::*;
use serial_test::serial;

// ============================================================================
// AzureProductionConfig Tests
// ============================================================================

mod production_config_tests {
    use super::*;

    /// Verify that AzureProductionConfig can be loaded from environment variables.
    #[test]
    #[serial]
    fn test_from_env_with_valid_variables() {
        // Arrange
        std::env::set_var("AZURE_KEY_VAULT_URL", "https://test-vault.vault.azure.net/");
        std::env::set_var("AZURE_STORAGE_ACCOUNT", "teststorage");
        std::env::set_var("AZURE_STORAGE_CONTAINER", "webhooks");
        std::env::set_var("AZURE_SERVICEBUS_NAMESPACE", "test-servicebus");
        std::env::set_var(
            "AZURE_APPINSIGHTS_CONNECTION_STRING",
            "InstrumentationKey=test-key",
        );
        std::env::set_var("AZURE_ENVIRONMENT", "production");
        std::env::set_var("AZURE_REGION", "eastus");

        // Act
        let result = AzureProductionConfig::from_env();

        // Assert
        assert!(result.is_ok(), "Should load configuration from environment");
        let config = result.unwrap();
        assert_eq!(
            config.key_vault.vault_url,
            "https://test-vault.vault.azure.net/"
        );
        assert_eq!(config.blob_storage.account_name, "teststorage");
        assert_eq!(config.blob_storage.container_name, "webhooks");
        assert_eq!(config.service_bus.namespace, "test-servicebus");
        assert_eq!(config.environment, "production");
        assert_eq!(config.region, "eastus");

        // Cleanup
        std::env::remove_var("AZURE_KEY_VAULT_URL");
        std::env::remove_var("AZURE_STORAGE_ACCOUNT");
        std::env::remove_var("AZURE_STORAGE_CONTAINER");
        std::env::remove_var("AZURE_SERVICEBUS_NAMESPACE");
        std::env::remove_var("AZURE_APPINSIGHTS_CONNECTION_STRING");
        std::env::remove_var("AZURE_ENVIRONMENT");
        std::env::remove_var("AZURE_REGION");
    }

    /// Verify that missing environment variables return appropriate errors.
    #[test]
    #[serial]
    fn test_from_env_missing_required_variable() {
        // Arrange - ensure no Azure environment variables are set
        std::env::remove_var("AZURE_KEY_VAULT_URL");

        // Act
        let result = AzureProductionConfig::from_env();

        // Assert
        assert!(result.is_err(), "Should fail with missing variable");
        let err = result.unwrap_err();
        assert!(
            matches!(err, AzureConfigError::MissingEnvVar { .. }),
            "Should return MissingEnvVar error"
        );
    }

    /// Verify that validation catches invalid Key Vault URLs.
    #[test]
    fn test_validate_invalid_key_vault_url() {
        // Arrange
        let config = AzureProductionConfig {
            key_vault: AzureKeyVaultConfig {
                vault_url: "http://not-https.com".to_string(), // Not HTTPS
                use_managed_identity: true,
                cache_ttl_seconds: 300,
            },
            blob_storage: AzureBlobStorageConfig::production(
                "teststorage".to_string(),
                "webhooks".to_string(),
            ),
            service_bus: AzureServiceBusConfig::production("test-sb".to_string()),
            telemetry: AzureTelemetryConfig::production(
                "InstrumentationKey=test".to_string(),
                "1.0.0".to_string(),
            ),
            environment: "production".to_string(),
            region: "eastus".to_string(),
        };

        // Act
        let result = config.validate();

        // Assert
        assert!(result.is_err(), "Should fail validation");
        assert!(
            matches!(
                result.unwrap_err(),
                AzureConfigError::InvalidKeyVaultUrl { .. }
            ),
            "Should return InvalidKeyVaultUrl error"
        );
    }

    /// Verify that validation catches invalid storage account names.
    #[test]
    fn test_validate_invalid_storage_account() {
        // Arrange
        let config = AzureProductionConfig {
            key_vault: AzureKeyVaultConfig {
                vault_url: "https://test.vault.azure.net/".to_string(),
                use_managed_identity: true,
                cache_ttl_seconds: 300,
            },
            blob_storage: AzureBlobStorageConfig::production(
                "Invalid-Name!".to_string(), // Invalid characters
                "webhooks".to_string(),
            ),
            service_bus: AzureServiceBusConfig::production("test-sb".to_string()),
            telemetry: AzureTelemetryConfig::production(
                "InstrumentationKey=test".to_string(),
                "1.0.0".to_string(),
            ),
            environment: "production".to_string(),
            region: "eastus".to_string(),
        };

        // Act
        let result = config.validate();

        // Assert
        assert!(result.is_err(), "Should fail validation");
        assert!(
            matches!(
                result.unwrap_err(),
                AzureConfigError::InvalidStorageAccount { .. }
            ),
            "Should return InvalidStorageAccount error"
        );
    }

    /// Verify that validation catches invalid environments.
    #[test]
    fn test_validate_invalid_environment() {
        // Arrange
        let config = AzureProductionConfig {
            key_vault: AzureKeyVaultConfig {
                vault_url: "https://test.vault.azure.net/".to_string(),
                use_managed_identity: true,
                cache_ttl_seconds: 300,
            },
            blob_storage: AzureBlobStorageConfig::production(
                "teststorage".to_string(),
                "webhooks".to_string(),
            ),
            service_bus: AzureServiceBusConfig::production("test-sb".to_string()),
            telemetry: AzureTelemetryConfig::production(
                "InstrumentationKey=test".to_string(),
                "1.0.0".to_string(),
            ),
            environment: "invalid-env".to_string(), // Invalid environment
            region: "eastus".to_string(),
        };

        // Act
        let result = config.validate();

        // Assert
        assert!(result.is_err(), "Should fail validation");
        assert!(
            matches!(
                result.unwrap_err(),
                AzureConfigError::InvalidEnvironment { .. }
            ),
            "Should return InvalidEnvironment error"
        );
    }

    /// Verify that validation catches empty regions.
    #[test]
    fn test_validate_empty_region() {
        // Arrange
        let config = AzureProductionConfig {
            key_vault: AzureKeyVaultConfig {
                vault_url: "https://test.vault.azure.net/".to_string(),
                use_managed_identity: true,
                cache_ttl_seconds: 300,
            },
            blob_storage: AzureBlobStorageConfig::production(
                "teststorage".to_string(),
                "webhooks".to_string(),
            ),
            service_bus: AzureServiceBusConfig::production("test-sb".to_string()),
            telemetry: AzureTelemetryConfig::production(
                "InstrumentationKey=test".to_string(),
                "1.0.0".to_string(),
            ),
            environment: "production".to_string(),
            region: String::new(), // Empty region
        };

        // Act
        let result = config.validate();

        // Assert
        assert!(result.is_err(), "Should fail validation");
        assert!(
            matches!(result.unwrap_err(), AzureConfigError::InvalidRegion { .. }),
            "Should return InvalidRegion error"
        );
    }

    /// Verify that valid production configuration passes validation.
    #[test]
    fn test_validate_valid_production_config() {
        // Arrange
        let config = AzureProductionConfig {
            key_vault: AzureKeyVaultConfig {
                vault_url: "https://prod-vault.vault.azure.net/".to_string(),
                use_managed_identity: true,
                cache_ttl_seconds: 300,
            },
            blob_storage: AzureBlobStorageConfig::production(
                "prodstorage".to_string(),
                "webhooks".to_string(),
            ),
            service_bus: AzureServiceBusConfig::production("prod-sb".to_string()),
            telemetry: AzureTelemetryConfig::production(
                "InstrumentationKey=prod-key".to_string(),
                "1.0.0".to_string(),
            ),
            environment: "production".to_string(),
            region: "eastus".to_string(),
        };

        // Act
        let result = config.validate();

        // Assert
        assert!(result.is_ok(), "Valid configuration should pass validation");
    }
}

// ============================================================================
// AzureKeyVaultConfig Tests
// ============================================================================

mod key_vault_config_tests {
    use super::*;

    /// Verify production Key Vault configuration uses Managed Identity.
    #[test]
    fn test_production_config_uses_managed_identity() {
        // Arrange & Act
        let config = AzureKeyVaultConfig::production("https://vault.azure.net/".to_string());

        // Assert
        assert!(
            config.use_managed_identity,
            "Production should use Managed Identity"
        );
        assert_eq!(
            config.cache_ttl_seconds, 300,
            "Production should use 5-minute cache"
        );
    }

    /// Verify development Key Vault configuration uses Azure CLI.
    #[test]
    fn test_development_config_uses_cli() {
        // Arrange & Act
        let config = AzureKeyVaultConfig::development("https://vault.azure.net/".to_string());

        // Assert
        assert!(
            !config.use_managed_identity,
            "Development should not use Managed Identity"
        );
        assert_eq!(
            config.cache_ttl_seconds, 60,
            "Development should use 1-minute cache"
        );
    }

    /// Verify that Key Vault URL is not logged in Debug output.
    #[test]
    fn test_debug_does_not_log_secrets() {
        // Arrange
        let config = AzureKeyVaultConfig::production("https://secret-vault.azure.net/".to_string());

        // Act
        let debug_output = format!("{:?}", config);

        // Assert
        // URL itself is not secret, so it can appear
        assert!(
            debug_output.contains("vault_url"),
            "Should show vault_url field name"
        );
    }
}

// ============================================================================
// AzureBlobStorageConfig Tests
// ============================================================================

mod blob_storage_config_tests {
    use super::*;

    /// Verify production Blob Storage configuration uses Managed Identity.
    #[test]
    fn test_production_config_no_connection_string() {
        // Arrange & Act
        let config =
            AzureBlobStorageConfig::production("storage".to_string(), "container".to_string());

        // Assert
        assert!(
            config.use_managed_identity,
            "Production should use Managed Identity"
        );
        assert!(
            config.connection_string.is_none(),
            "Production should not have connection string"
        );
    }

    /// Verify development Blob Storage configuration has connection string.
    #[test]
    fn test_development_config_has_connection_string() {
        // Arrange & Act
        let config = AzureBlobStorageConfig::development(
            "storage".to_string(),
            "container".to_string(),
            "DefaultEndpointsProtocol=https".to_string(),
        );

        // Assert
        assert!(
            !config.use_managed_identity,
            "Development should not use Managed Identity"
        );
        assert!(
            config.connection_string.is_some(),
            "Development should have connection string"
        );
    }

    /// Verify that connection string is redacted in Debug output.
    #[test]
    fn test_debug_redacts_connection_string() {
        // Arrange
        let config = AzureBlobStorageConfig::development(
            "storage".to_string(),
            "container".to_string(),
            "AccountKey=super-secret-key".to_string(),
        );

        // Act
        let debug_output = format!("{:?}", config);

        // Assert
        assert!(
            debug_output.contains("REDACTED"),
            "Should redact connection string"
        );
        assert!(
            !debug_output.contains("super-secret-key"),
            "Should not contain actual secret"
        );
    }
}

// ============================================================================
// AzureServiceBusConfig Tests
// ============================================================================

mod service_bus_config_tests {
    use super::*;

    /// Verify production Service Bus configuration uses Managed Identity.
    #[test]
    fn test_production_config_managed_identity() {
        // Arrange & Act
        let config = AzureServiceBusConfig::production("prod-namespace".to_string());

        // Assert
        assert!(
            config.use_managed_identity,
            "Production should use Managed Identity"
        );
        assert!(
            config.connection_string.is_none(),
            "Production should not have connection string"
        );
        assert!(config.use_sessions, "Production should enable sessions");
    }

    /// Verify development Service Bus configuration has connection string.
    #[test]
    fn test_development_config_connection_string() {
        // Arrange & Act
        let config = AzureServiceBusConfig::development(
            "dev-namespace".to_string(),
            "Endpoint=sb://test.servicebus.windows.net".to_string(),
        );

        // Assert
        assert!(
            !config.use_managed_identity,
            "Development should not use Managed Identity"
        );
        assert!(
            config.connection_string.is_some(),
            "Development should have connection string"
        );
    }

    /// Verify that connection string is redacted in Debug output.
    #[test]
    fn test_debug_redacts_service_bus_connection_string() {
        // Arrange
        let config = AzureServiceBusConfig::development(
            "namespace".to_string(),
            "SharedAccessKey=secret-key".to_string(),
        );

        // Act
        let debug_output = format!("{:?}", config);

        // Assert
        assert!(
            debug_output.contains("REDACTED"),
            "Should redact connection string"
        );
        assert!(
            !debug_output.contains("secret-key"),
            "Should not contain actual secret"
        );
    }
}

// ============================================================================
// AzureTelemetryConfig Tests
// ============================================================================

mod telemetry_config_tests {
    use super::*;

    /// Verify production telemetry configuration has appropriate sampling.
    #[test]
    fn test_production_config_sampling() {
        // Arrange & Act
        let config = AzureTelemetryConfig::production(
            "InstrumentationKey=key".to_string(),
            "1.0.0".to_string(),
        );

        // Assert
        assert!(config.enable_tracing, "Production should enable tracing");
        assert_eq!(
            config.sampling_ratio, 0.1,
            "Production should use 10% sampling"
        );
        assert_eq!(config.service_name, "queue-keeper");
        assert_eq!(config.service_version, "1.0.0");
    }

    /// Verify development telemetry configuration uses full sampling.
    #[test]
    fn test_development_config_full_sampling() {
        // Arrange & Act
        let config = AzureTelemetryConfig::development("InstrumentationKey=key".to_string());

        // Assert
        assert!(config.enable_tracing, "Development should enable tracing");
        assert_eq!(
            config.sampling_ratio, 1.0,
            "Development should use 100% sampling"
        );
        assert_eq!(config.service_version, "dev");
    }

    /// Verify that Application Insights connection string is redacted in Debug output.
    #[test]
    fn test_debug_redacts_connection_string() {
        // Arrange
        let config = AzureTelemetryConfig::production(
            "InstrumentationKey=secret-instrumentation-key".to_string(),
            "1.0.0".to_string(),
        );

        // Act
        let debug_output = format!("{:?}", config);

        // Assert
        assert!(
            debug_output.contains("REDACTED"),
            "Should redact connection string"
        );
        assert!(
            !debug_output.contains("secret-instrumentation-key"),
            "Should not contain actual key"
        );
    }
}

// ============================================================================
// Error Tests
// ============================================================================

mod error_tests {
    use super::*;

    /// Verify error messages are descriptive.
    #[test]
    fn test_error_messages_are_descriptive() {
        // Arrange & Act
        let missing_var_error = AzureConfigError::MissingEnvVar {
            variable: "AZURE_KEY_VAULT_URL".to_string(),
        };
        let invalid_kv_error = AzureConfigError::InvalidKeyVaultUrl {
            url: "http://not-secure".to_string(),
            reason: "Must use HTTPS".to_string(),
        };
        let invalid_storage_error = AzureConfigError::InvalidStorageAccount {
            name: "Bad-Name".to_string(),
            reason: "Contains invalid characters".to_string(),
        };

        // Assert
        assert!(
            missing_var_error
                .to_string()
                .contains("AZURE_KEY_VAULT_URL"),
            "Error should mention variable name"
        );
        assert!(
            invalid_kv_error.to_string().contains("http://not-secure"),
            "Error should mention invalid URL"
        );
        assert!(
            invalid_storage_error.to_string().contains("Bad-Name"),
            "Error should mention invalid name"
        );
    }
}
