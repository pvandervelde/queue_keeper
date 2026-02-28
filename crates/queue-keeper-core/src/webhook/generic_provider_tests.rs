//! Tests for [`GenericWebhookProvider`] and its configuration types.

use super::*;

// ============================================================================
// GenericProviderConfig validation tests
// ============================================================================

mod config_validation {
    use super::*;

    /// Verify that a valid direct-mode config passes validation.
    #[test]
    fn test_valid_direct_mode_config_passes() {
        let config = GenericProviderConfig {
            provider_id: "jira".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-jira".to_string()),
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that a valid wrap-mode config with field extraction passes validation.
    #[test]
    fn test_valid_wrap_mode_config_passes() {
        let config = GenericProviderConfig {
            provider_id: "gitlab".to_string(),
            processing_mode: ProcessingMode::Wrap,
            target_queue: None,
            event_type_source: Some(FieldSource::Header {
                name: "X-Gitlab-Event".to_string(),
            }),
            delivery_id_source: Some(FieldSource::AutoGenerate),
            signature: None,
            field_extraction: Some(FieldExtractionConfig {
                repository_path: "project.path_with_namespace".to_string(),
                entity_path: Some("object_attributes.iid".to_string()),
                action_path: Some("object_attributes.action".to_string()),
            }),
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that wrap mode without field_extraction is rejected.
    #[test]
    fn test_wrap_mode_requires_field_extraction() {
        let config = GenericProviderConfig {
            provider_id: "no-extract".to_string(),
            processing_mode: ProcessingMode::Wrap,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(
                err,
                GenericProviderConfigError::MissingFieldExtraction { .. }
            ),
            "expected MissingFieldExtraction, got: {err:?}"
        );
    }

    /// Verify that direct mode without target_queue is rejected.
    #[test]
    fn test_direct_mode_requires_target_queue() {
        let config = GenericProviderConfig {
            provider_id: "jira".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::MissingTargetQueue { .. }),
            "expected MissingTargetQueue, got: {err:?}"
        );
    }

    /// Verify that an empty target_queue is rejected even when provided.
    #[test]
    fn test_direct_mode_empty_target_queue_rejected() {
        let config = GenericProviderConfig {
            provider_id: "jira".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("".to_string()),
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::InvalidTargetQueue { .. }),
            "expected InvalidTargetQueue, got: {err:?}"
        );
    }

    /// Verify that an empty provider_id is rejected.
    #[test]
    fn test_empty_provider_id_rejected() {
        let config = GenericProviderConfig {
            provider_id: "".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::InvalidProviderId { .. }),
            "expected InvalidProviderId, got: {err:?}"
        );
    }

    /// Verify that a provider_id with uppercase letters is rejected.
    #[test]
    fn test_uppercase_provider_id_rejected() {
        let config = GenericProviderConfig {
            provider_id: "GitLab".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::InvalidProviderId { .. }),
            "expected InvalidProviderId, got: {err:?}"
        );
    }

    /// Verify that a provider_id with slashes is rejected.
    #[test]
    fn test_provider_id_with_slashes_rejected() {
        let config = GenericProviderConfig {
            provider_id: "../escape".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::InvalidProviderId { .. }),
            "expected InvalidProviderId, got: {err:?}"
        );
    }

    /// Verify that a provider_id with spaces is rejected.
    #[test]
    fn test_provider_id_with_spaces_rejected() {
        let config = GenericProviderConfig {
            provider_id: "my provider".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::InvalidProviderId { .. }),
            "expected InvalidProviderId, got: {err:?}"
        );
    }

    /// Verify that hyphens and underscores are allowed in provider_id.
    #[test]
    fn test_provider_id_allows_hyphens_and_underscores() {
        let config = GenericProviderConfig {
            provider_id: "my-cool_app".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-my-cool-app".to_string()),
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that digits are allowed in provider_id.
    #[test]
    fn test_provider_id_allows_digits() {
        let config = GenericProviderConfig {
            provider_id: "app42".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-app42".to_string()),
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        assert!(config.validate().is_ok());
    }

    /// Verify that an invalid event_type_source is caught.
    #[test]
    fn test_invalid_event_type_source_rejected() {
        let config = GenericProviderConfig {
            provider_id: "test".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-test".to_string()),
            event_type_source: Some(FieldSource::Header {
                name: "".to_string(),
            }),
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::InvalidFieldSource { .. }),
            "expected InvalidFieldSource, got: {err:?}"
        );
    }

    /// Verify that an invalid delivery_id_source is caught.
    #[test]
    fn test_invalid_delivery_id_source_rejected() {
        let config = GenericProviderConfig {
            provider_id: "test".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-test".to_string()),
            event_type_source: None,
            delivery_id_source: Some(FieldSource::JsonPath {
                path: "".to_string(),
            }),
            signature: None,
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(err, GenericProviderConfigError::InvalidFieldSource { .. }),
            "expected InvalidFieldSource, got: {err:?}"
        );
    }

    /// Verify that an invalid signature config (empty header) is caught.
    #[test]
    fn test_invalid_signature_config_rejected() {
        let config = GenericProviderConfig {
            provider_id: "test".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-test".to_string()),
            event_type_source: None,
            delivery_id_source: None,
            signature: Some(SignatureConfig {
                header_name: "".to_string(),
                algorithm: SignatureAlgorithm::HmacSha256,
            }),
            field_extraction: None,
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(
                err,
                GenericProviderConfigError::InvalidSignatureConfig { .. }
            ),
            "expected InvalidSignatureConfig, got: {err:?}"
        );
    }

    /// Verify that an empty repository_path in extraction config is caught.
    #[test]
    fn test_empty_repository_path_rejected() {
        let config = GenericProviderConfig {
            provider_id: "test".to_string(),
            processing_mode: ProcessingMode::Wrap,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: Some(FieldExtractionConfig {
                repository_path: "".to_string(),
                entity_path: None,
                action_path: None,
            }),
        };
        let err = config.validate().unwrap_err();
        assert!(
            matches!(
                err,
                GenericProviderConfigError::InvalidFieldExtraction { .. }
            ),
            "expected InvalidFieldExtraction, got: {err:?}"
        );
    }
}

// ============================================================================
// FieldSource tests
// ============================================================================

mod field_source_tests {
    use super::*;

    /// Verify Header variant stores name correctly.
    #[test]
    fn test_header_stores_name() {
        let source = FieldSource::Header {
            name: "X-Custom-Header".to_string(),
        };
        if let FieldSource::Header { name } = &source {
            assert_eq!(name, "X-Custom-Header");
        } else {
            panic!("expected Header variant");
        }
    }

    /// Verify JsonPath variant stores path correctly.
    #[test]
    fn test_json_path_stores_path() {
        let source = FieldSource::JsonPath {
            path: "data.attributes.type".to_string(),
        };
        if let FieldSource::JsonPath { path } = &source {
            assert_eq!(path, "data.attributes.type");
        } else {
            panic!("expected JsonPath variant");
        }
    }

    /// Verify Static variant stores value correctly.
    #[test]
    fn test_static_stores_value() {
        let source = FieldSource::Static {
            value: "push".to_string(),
        };
        if let FieldSource::Static { value } = &source {
            assert_eq!(value, "push");
        } else {
            panic!("expected Static variant");
        }
    }

    /// Verify AutoGenerate variant exists and validates.
    #[test]
    fn test_auto_generate_validates() {
        let source = FieldSource::AutoGenerate;
        assert!(source.validate("test").is_ok());
    }

    /// Verify empty Header name is rejected.
    #[test]
    fn test_empty_header_name_rejected() {
        let source = FieldSource::Header {
            name: "".to_string(),
        };
        assert!(source.validate("test").is_err());
    }

    /// Verify empty JsonPath path is rejected.
    #[test]
    fn test_empty_json_path_rejected() {
        let source = FieldSource::JsonPath {
            path: "".to_string(),
        };
        assert!(source.validate("test").is_err());
    }

    /// Verify empty Static value is rejected.
    #[test]
    fn test_empty_static_value_rejected() {
        let source = FieldSource::Static {
            value: "".to_string(),
        };
        assert!(source.validate("test").is_err());
    }
}

// ============================================================================
// Serialization roundtrip tests
// ============================================================================

mod serde_roundtrip {
    use super::*;

    /// Verify ProcessingMode serialisation roundtrip.
    #[test]
    fn test_processing_mode_roundtrip() {
        for mode in [ProcessingMode::Wrap, ProcessingMode::Direct] {
            let json = serde_json::to_string(&mode).expect("serialise");
            let deser: ProcessingMode = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(deser, mode);
        }
    }

    /// Verify FieldSource::Header serialisation roundtrip.
    #[test]
    fn test_field_source_header_roundtrip() {
        let source = FieldSource::Header {
            name: "X-Event".to_string(),
        };
        let json = serde_json::to_string(&source).expect("serialise");
        let deser: FieldSource = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(deser, source);
    }

    /// Verify FieldSource::JsonPath serialisation roundtrip.
    #[test]
    fn test_field_source_json_path_roundtrip() {
        let source = FieldSource::JsonPath {
            path: "a.b.c".to_string(),
        };
        let json = serde_json::to_string(&source).expect("serialise");
        let deser: FieldSource = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(deser, source);
    }

    /// Verify FieldSource::Static serialisation roundtrip.
    #[test]
    fn test_field_source_static_roundtrip() {
        let source = FieldSource::Static {
            value: "static-val".to_string(),
        };
        let json = serde_json::to_string(&source).expect("serialise");
        let deser: FieldSource = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(deser, source);
    }

    /// Verify FieldSource::AutoGenerate serialisation roundtrip.
    #[test]
    fn test_field_source_auto_generate_roundtrip() {
        let source = FieldSource::AutoGenerate;
        let json = serde_json::to_string(&source).expect("serialise");
        let deser: FieldSource = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(deser, source);
    }

    /// Verify SignatureAlgorithm serialisation roundtrip.
    #[test]
    fn test_signature_algorithm_roundtrip() {
        for alg in [
            SignatureAlgorithm::HmacSha256,
            SignatureAlgorithm::HmacSha1,
            SignatureAlgorithm::BearerToken,
        ] {
            let json = serde_json::to_string(&alg).expect("serialise");
            let deser: SignatureAlgorithm = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(deser, alg);
        }
    }

    /// Verify full GenericProviderConfig serialisation roundtrip via JSON.
    #[test]
    fn test_full_config_json_roundtrip() {
        let config = GenericProviderConfig {
            provider_id: "roundtrip".to_string(),
            processing_mode: ProcessingMode::Wrap,
            target_queue: None,
            event_type_source: Some(FieldSource::Header {
                name: "X-Event-Type".to_string(),
            }),
            delivery_id_source: Some(FieldSource::AutoGenerate),
            signature: Some(SignatureConfig {
                header_name: "X-Signature".to_string(),
                algorithm: SignatureAlgorithm::HmacSha256,
            }),
            field_extraction: Some(FieldExtractionConfig {
                repository_path: "repo.full_name".to_string(),
                entity_path: Some("pr.number".to_string()),
                action_path: Some("action".to_string()),
            }),
        };
        let json = serde_json::to_string_pretty(&config).expect("serialise");
        let deser: GenericProviderConfig = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(deser.provider_id, config.provider_id);
        assert_eq!(deser.processing_mode, config.processing_mode);
        assert_eq!(deser.event_type_source, config.event_type_source);
        assert_eq!(deser.delivery_id_source, config.delivery_id_source);
        assert_eq!(deser.signature, config.signature);
        assert_eq!(deser.field_extraction, config.field_extraction);
    }

    /// Verify GenericProviderConfig can be deserialized from YAML.
    #[test]
    fn test_config_yaml_deserialization() {
        let yaml = r#"
provider_id: "gitlab"
processing_mode: "wrap"
event_type_source:
  type: "header"
  name: "X-Gitlab-Event"
field_extraction:
  repository_path: "project.path_with_namespace"
  entity_path: "object_attributes.iid"
"#;
        let config: GenericProviderConfig =
            serde_yaml::from_str(yaml).expect("YAML deserialisation");
        assert_eq!(config.provider_id, "gitlab");
        assert_eq!(config.processing_mode, ProcessingMode::Wrap);
        assert!(config.validate().is_ok());
    }

    /// Verify GenericProviderConfig can be deserialized from minimal YAML
    /// (all optional fields omitted via serde defaults).
    #[test]
    fn test_config_minimal_yaml_deserialization() {
        let yaml = r#"
provider_id: "slack"
processing_mode: "direct"
target_queue: "queue-keeper-slack"
"#;
        let config: GenericProviderConfig =
            serde_yaml::from_str(yaml).expect("YAML deserialisation");
        assert_eq!(config.provider_id, "slack");
        assert_eq!(config.processing_mode, ProcessingMode::Direct);
        assert_eq!(config.target_queue.as_deref(), Some("queue-keeper-slack"));
        assert!(config.event_type_source.is_none());
        assert!(config.delivery_id_source.is_none());
        assert!(config.signature.is_none());
        assert!(config.field_extraction.is_none());
        assert!(config.validate().is_ok());
    }
}

// ============================================================================
// GenericWebhookProvider construction tests
// ============================================================================

mod provider_construction {
    use super::*;

    /// Verify that a valid config produces a provider successfully.
    #[test]
    fn test_new_with_valid_config_succeeds() {
        let config = GenericProviderConfig {
            provider_id: "jira".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-jira".to_string()),
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let provider = GenericWebhookProvider::new(config, None);
        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.provider_id(), "jira");
        assert_eq!(provider.processing_mode(), ProcessingMode::Direct);
    }

    /// Verify that an invalid config causes construction to fail.
    #[test]
    fn test_new_with_invalid_config_fails() {
        let config = GenericProviderConfig {
            provider_id: "".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        let result = GenericWebhookProvider::new(config, None);
        assert!(result.is_err());
    }

    /// Verify the provider stores the processing mode correctly.
    #[test]
    fn test_processing_mode_stored() {
        let config = GenericProviderConfig {
            provider_id: "wrap-test".to_string(),
            processing_mode: ProcessingMode::Wrap,
            target_queue: None,
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: Some(FieldExtractionConfig {
                repository_path: "repo.name".to_string(),
                entity_path: None,
                action_path: None,
            }),
        };
        let provider = GenericWebhookProvider::new(config, None).unwrap();
        assert_eq!(provider.processing_mode(), ProcessingMode::Wrap);
    }
}

// ============================================================================
// WebhookProcessor stub behaviour tests
// ============================================================================

mod processor_stubs {
    use super::*;
    use crate::webhook::{ValidationStatus, WebhookHeaders, WebhookRequest};
    use bytes::Bytes;

    fn direct_provider() -> GenericWebhookProvider {
        let config = GenericProviderConfig {
            provider_id: "stub-provider".to_string(),
            processing_mode: ProcessingMode::Direct,
            target_queue: Some("queue-keeper-stub-provider".to_string()),
            event_type_source: None,
            delivery_id_source: None,
            signature: None,
            field_extraction: None,
        };
        GenericWebhookProvider::new(config, None).unwrap()
    }

    fn test_request() -> WebhookRequest {
        let headers = WebhookHeaders {
            event_type: "ping".to_string(),
            delivery_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            signature: None,
            user_agent: None,
            content_type: "application/json".to_string(),
        };
        WebhookRequest::new(headers, Bytes::from(r#"{"test":true}"#))
    }

    /// Verify that process_webhook returns an error (not yet implemented).
    #[tokio::test]
    async fn test_process_webhook_returns_not_implemented_error() {
        let provider = direct_provider();
        let result = provider.process_webhook(test_request()).await;
        assert!(result.is_err());
    }

    /// Verify that validate_signature succeeds (stub passthrough).
    #[tokio::test]
    async fn test_validate_signature_stub_succeeds() {
        let provider = direct_provider();
        let result = provider
            .validate_signature(b"payload", "sig", "event")
            .await;
        assert!(result.is_ok());
    }

    /// Verify that store_raw_payload returns a placeholder reference.
    #[tokio::test]
    async fn test_store_raw_payload_returns_placeholder() {
        let provider = direct_provider();
        let request = test_request();
        let result = provider
            .store_raw_payload(&request, ValidationStatus::Valid)
            .await;
        assert!(result.is_ok());
        let storage_ref = result.unwrap();
        assert!(storage_ref.blob_path.starts_with("not-stored/"));
    }

    /// Verify that normalize_event returns not-implemented error.
    #[tokio::test]
    async fn test_normalize_event_returns_not_implemented() {
        let provider = direct_provider();
        let result = provider.normalize_event(&test_request()).await;
        assert!(result.is_err());
    }
}
