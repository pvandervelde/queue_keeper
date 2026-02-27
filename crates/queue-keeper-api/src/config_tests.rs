//! Tests for [`ServiceConfig`], [`ProviderConfig`], and [`ProviderSecretConfig`].

use super::*;

// ============================================================================
// ProviderSecretConfig tests
// ============================================================================

mod provider_secret_config_tests {
    use super::*;

    /// Verify that a valid KeyVault secret config passes validation.
    #[test]
    fn test_key_vault_with_non_empty_name_passes() {
        let secret = ProviderSecretConfig::KeyVault {
            secret_name: "github-webhook-secret".to_string(),
        };
        assert!(secret.validate("github").is_ok());
    }

    /// Verify that a KeyVault config with an empty secret_name fails.
    #[test]
    fn test_key_vault_with_empty_name_fails() {
        let secret = ProviderSecretConfig::KeyVault {
            secret_name: "".to_string(),
        };
        let result = secret.validate("github");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::ProviderValidation { .. }),
            "expected ProviderValidation, got: {:?}",
            err
        );
    }

    /// Verify that a valid Literal secret config passes validation.
    #[test]
    fn test_literal_with_non_empty_value_passes() {
        let secret = ProviderSecretConfig::Literal {
            value: "super-secret".to_string(),
        };
        assert!(secret.validate("test-provider").is_ok());
    }

    /// Verify that a Literal config with an empty value fails.
    #[test]
    fn test_literal_with_empty_value_fails() {
        let secret = ProviderSecretConfig::Literal {
            value: "".to_string(),
        };
        let result = secret.validate("test-provider");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ProviderValidation { .. }));
    }

    /// Verify that Debug output for Literal redacts the secret value.
    #[test]
    fn test_literal_debug_redacts_value() {
        let secret = ProviderSecretConfig::Literal {
            value: "super-sensitive".to_string(),
        };
        let debug_str = format!("{:?}", secret);
        assert!(
            !debug_str.contains("super-sensitive"),
            "debug output must not leak secret: {debug_str}"
        );
        assert!(
            debug_str.contains("REDACTED"),
            "debug output must contain REDACTED placeholder: {debug_str}"
        );
    }

    /// Verify that Debug output for KeyVault shows the secret_name (not sensitive).
    #[test]
    fn test_key_vault_debug_shows_name() {
        let secret = ProviderSecretConfig::KeyVault {
            secret_name: "my-secret".to_string(),
        };
        let debug_str = format!("{:?}", secret);
        assert!(
            debug_str.contains("my-secret"),
            "KeyVault debug should show secret_name"
        );
    }
}

// ============================================================================
// ProviderConfig tests
// ============================================================================

mod provider_config_tests {
    use super::*;

    fn github_provider_with_key_vault() -> ProviderConfig {
        ProviderConfig {
            id: "github".to_string(),
            require_signature: true,
            secret: Some(ProviderSecretConfig::KeyVault {
                secret_name: "github-webhook-secret".to_string(),
            }),
            allowed_event_types: vec![],
        }
    }

    /// Verify that a valid provider config (GitHub + key vault) passes validation.
    #[test]
    fn test_valid_github_config_passes() {
        let config = github_provider_with_key_vault();
        assert!(config.validate().is_ok());
    }

    /// Verify that an empty provider ID fails validation.
    #[test]
    fn test_empty_id_fails() {
        let config = ProviderConfig {
            id: "".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec![],
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ProviderValidation { .. }));
    }

    /// Verify that an ID with uppercase letters fails validation.
    #[test]
    fn test_uppercase_id_fails() {
        let config = ProviderConfig {
            id: "GitHub".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec![],
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ProviderValidation { .. }));
    }

    /// Verify that an ID with slashes fails validation.
    #[test]
    fn test_id_with_slash_fails() {
        let config = ProviderConfig {
            id: "../escape".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_err());
    }

    /// Verify that an ID with spaces fails validation.
    #[test]
    fn test_id_with_spaces_fails() {
        let config = ProviderConfig {
            id: "my app".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_err());
    }

    /// Verify that IDs with hyphens and underscores are accepted.
    #[test]
    fn test_id_with_hyphens_and_underscores_passes() {
        let config = ProviderConfig {
            id: "my-cool_app".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that require_signature=true without a secret fails.
    #[test]
    fn test_signature_required_without_secret_fails() {
        let config = ProviderConfig {
            id: "myprovider".to_string(),
            require_signature: true,
            secret: None,
            allowed_event_types: vec![],
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::ProviderValidation { message }
                if message.contains("require_signature")),
            "expected message about require_signature, got: {:?}",
            err
        );
    }

    /// Verify that require_signature=false without a secret is valid.
    #[test]
    fn test_no_signature_no_secret_passes() {
        let config = ProviderConfig {
            id: "myprovider".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that require_signature=true with a valid key vault secret passes.
    #[test]
    fn test_signature_required_with_key_vault_passes() {
        let config = ProviderConfig {
            id: "myprovider".to_string(),
            require_signature: true,
            secret: Some(ProviderSecretConfig::KeyVault {
                secret_name: "my-secret".to_string(),
            }),
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that a provider with require_signature=true and an empty KeyVault name fails.
    #[test]
    fn test_empty_key_vault_name_fails() {
        let config = ProviderConfig {
            id: "myprovider".to_string(),
            require_signature: true,
            secret: Some(ProviderSecretConfig::KeyVault {
                secret_name: "".to_string(),
            }),
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_err());
    }

    /// Verify that a Literal secret is accepted (for dev/test usage).
    #[test]
    fn test_literal_secret_is_accepted() {
        let config = ProviderConfig {
            id: "testprovider".to_string(),
            require_signature: true,
            secret: Some(ProviderSecretConfig::Literal {
                value: "dev-secret-value".to_string(),
            }),
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that allowed_event_types can be empty (all events allowed).
    #[test]
    fn test_empty_allowed_event_types_passes() {
        let config = ProviderConfig {
            id: "github".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec![],
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that allowed_event_types with values passes validation.
    #[test]
    fn test_non_empty_allowed_event_types_passes() {
        let config = ProviderConfig {
            id: "github".to_string(),
            require_signature: false,
            secret: None,
            allowed_event_types: vec!["push".to_string(), "pull_request".to_string()],
        };
        assert!(config.validate().is_ok());
    }
}

// ============================================================================
// ServiceConfig::validate tests
// ============================================================================

mod service_config_validate_tests {
    use super::*;

    /// Verify that the default ServiceConfig (no providers) passes validation.
    #[test]
    fn test_default_config_is_valid() {
        let config = ServiceConfig::default();
        assert!(config.validate().is_ok());
    }

    /// Verify that an empty providers list passes validation.
    #[test]
    fn test_no_providers_is_valid() {
        let config = ServiceConfig {
            providers: vec![],
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that one valid provider passes validation.
    #[test]
    fn test_single_valid_provider_passes() {
        let config = ServiceConfig {
            providers: vec![ProviderConfig {
                id: "github".to_string(),
                require_signature: true,
                secret: Some(ProviderSecretConfig::KeyVault {
                    secret_name: "github-webhook-secret".to_string(),
                }),
                allowed_event_types: vec![],
            }],
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that multiple providers with distinct IDs pass validation.
    #[test]
    fn test_multiple_distinct_providers_pass() {
        let config = ServiceConfig {
            providers: vec![
                ProviderConfig {
                    id: "github".to_string(),
                    require_signature: false,
                    secret: None,
                    allowed_event_types: vec![],
                },
                ProviderConfig {
                    id: "jira".to_string(),
                    require_signature: false,
                    secret: None,
                    allowed_event_types: vec![],
                },
            ],
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that duplicate provider IDs fail validation.
    #[test]
    fn test_duplicate_provider_ids_fail() {
        let config = ServiceConfig {
            providers: vec![
                ProviderConfig {
                    id: "github".to_string(),
                    require_signature: false,
                    secret: None,
                    allowed_event_types: vec![],
                },
                ProviderConfig {
                    id: "github".to_string(),
                    require_signature: false,
                    secret: None,
                    allowed_event_types: vec![],
                },
            ],
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::ProviderValidation { message }
                if message.contains("duplicate")),
            "expected duplicate provider error, got: {:?}",
            err
        );
    }

    /// Verify that an invalid provider ID propagates from provider validation.
    #[test]
    fn test_invalid_provider_id_propagates_error() {
        let config = ServiceConfig {
            providers: vec![ProviderConfig {
                id: "INVALID".to_string(),
                require_signature: false,
                secret: None,
                allowed_event_types: vec![],
            }],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    /// Verify that missing secret for a signed provider propagates error.
    #[test]
    fn test_missing_secret_propagates_error() {
        let config = ServiceConfig {
            providers: vec![ProviderConfig {
                id: "myprovider".to_string(),
                require_signature: true,
                secret: None,
                allowed_event_types: vec![],
            }],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}

// ============================================================================
// Serialization tests
// ============================================================================

mod serialization_tests {
    use super::*;

    /// Verify that a ProviderConfig round-trips through JSON serialization.
    #[test]
    fn test_provider_config_json_round_trip() {
        let original = ProviderConfig {
            id: "github".to_string(),
            require_signature: true,
            secret: Some(ProviderSecretConfig::KeyVault {
                secret_name: "github-webhook-secret".to_string(),
            }),
            allowed_event_types: vec!["push".to_string()],
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: ProviderConfig =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.require_signature, original.require_signature);
        assert_eq!(
            deserialized.allowed_event_types,
            original.allowed_event_types
        );
    }

    /// Verify that ServiceConfig with providers can be round-tripped through JSON.
    #[test]
    fn test_service_config_with_providers_round_trip() {
        let original = ServiceConfig {
            providers: vec![ProviderConfig {
                id: "github".to_string(),
                require_signature: false,
                secret: None,
                allowed_event_types: vec![],
            }],
            ..Default::default()
        };

        let json = serde_json::to_string(&original).expect("serialization failed");
        let deserialized: ServiceConfig =
            serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(deserialized.providers.len(), 1);
        assert_eq!(deserialized.providers[0].id, "github");
    }

    /// Verify that a ServiceConfig without the providers field deserializes
    /// with an empty providers list (backward compatibility).
    #[test]
    fn test_missing_providers_field_deserializes_as_empty() {
        // Round-trip a default config (which has an empty providers list)
        // then manually remove the "providers" key and re-deserialize.
        // The #[serde(default)] annotation must make this succeed.
        let default_config = ServiceConfig::default();
        let mut json_value: serde_json::Value =
            serde_json::to_value(&default_config).expect("serialization failed");

        // Remove the providers field to simulate older config files
        if let serde_json::Value::Object(ref mut map) = json_value {
            map.remove("providers");
        }

        let json_str = serde_json::to_string(&json_value).expect("re-serialization failed");
        let result: Result<ServiceConfig, _> = serde_json::from_str(&json_str);

        assert!(
            result.is_ok(),
            "config without providers field must parse successfully: {:?}",
            result.err()
        );
        let config = result.unwrap();
        assert!(
            config.providers.is_empty(),
            "providers should default to empty list"
        );
    }
}
