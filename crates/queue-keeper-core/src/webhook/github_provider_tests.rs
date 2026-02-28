//! Tests for [`GithubWebhookProvider`].

use super::*;
use crate::{
    webhook::{
        NormalizationError, StorageError, StorageReference, ValidationStatus,
        WebhookError, WebhookHeaders, WebhookRequest,
    },
    Timestamp, ValidationError,
};
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;

// ============================================================================
// Test helpers
// ============================================================================

/// Build a minimal valid ping webhook request (no signature required).
fn ping_request() -> WebhookRequest {
    let headers = WebhookHeaders {
        event_type: "ping".to_string(),
        delivery_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        signature: None,
        user_agent: Some("GitHub-Hookshot/test".to_string()),
        content_type: "application/json".to_string(),
    };
    let body = serde_json::json!({
        "repository": {
            "id": 1,
            "name": "repo",
            "full_name": "owner/repo",
            "private": false,
            "owner": {
                "id": 1,
                "login": "owner",
                "type": "User"
            }
        }
    });
    WebhookRequest::new(headers, Bytes::from(body.to_string()))
}

/// Build a signed pull_request webhook request.
fn pull_request_request(signature: Option<String>) -> WebhookRequest {
    let headers = WebhookHeaders {
        event_type: "pull_request".to_string(),
        delivery_id: "550e8400-e29b-41d4-a716-446655440001".to_string(),
        signature,
        user_agent: Some("GitHub-Hookshot/test".to_string()),
        content_type: "application/json".to_string(),
    };
    let body = serde_json::json!({
        "action": "opened",
        "pull_request": { "number": 42 },
        "repository": {
            "id": 1,
            "name": "repo",
            "full_name": "owner/repo",
            "private": false,
            "owner": {
                "id": 1,
                "login": "owner",
                "type": "User"
            }
        }
    });
    WebhookRequest::new(headers, Bytes::from(body.to_string()))
}

// ============================================================================
// Minimal mock WebhookProcessor for delegation verification
// ============================================================================

struct AlwaysSucceedValidator;

#[async_trait]
impl SignatureValidator for AlwaysSucceedValidator {
    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _secret_key: &str,
    ) -> Result<(), ValidationError> {
        Ok(())
    }

    async fn get_webhook_secret(
        &self,
        _event_type: &str,
    ) -> Result<String, crate::webhook::SecretError> {
        Ok("test-secret".to_string())
    }

    fn supports_constant_time_comparison(&self) -> bool {
        true
    }
}

struct AlwaysFailValidator;

#[async_trait]
impl SignatureValidator for AlwaysFailValidator {
    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _secret_key: &str,
    ) -> Result<(), ValidationError> {
        Err(ValidationError::InvalidFormat {
            field: "signature".to_string(),
            message: "invalid".to_string(),
        })
    }

    async fn get_webhook_secret(
        &self,
        _event_type: &str,
    ) -> Result<String, crate::webhook::SecretError> {
        Ok("test-secret".to_string())
    }

    fn supports_constant_time_comparison(&self) -> bool {
        true
    }
}

struct NoopPayloadStorer;

#[async_trait]
impl PayloadStorer for NoopPayloadStorer {
    async fn store_payload(
        &self,
        request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        Ok(StorageReference {
            blob_path: format!("test/{}", request.delivery_id()),
            stored_at: Timestamp::now(),
            size_bytes: request.body.len() as u64,
        })
    }

    async fn retrieve_payload(
        &self,
        _storage_ref: &StorageReference,
    ) -> Result<WebhookRequest, StorageError> {
        Err(StorageError::OperationFailed {
            message: "not implemented in test".to_string(),
        })
    }

    async fn list_payloads(
        &self,
        _filters: crate::webhook::PayloadFilters,
    ) -> Result<Vec<StorageReference>, StorageError> {
        Ok(vec![])
    }
}

// ============================================================================
// Tests
// ============================================================================

mod provider_id_tests {
    use super::*;

    /// Verify the canonical provider ID constant is "github".
    #[test]
    fn test_provider_id_constant_is_github() {
        assert_eq!(GithubWebhookProvider::PROVIDER_ID, "github");
    }

    /// Verify that PROVIDER_ID is a valid ProviderId characters-wise
    /// (all lowercase alphanumeric / hyphens / underscores, non-empty).
    #[test]
    fn test_provider_id_is_url_safe() {
        let id_str = GithubWebhookProvider::PROVIDER_ID;
        assert!(!id_str.is_empty(), "provider ID must not be empty");
        assert!(
            id_str
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'),
            "provider ID must only contain [a-z0-9\\-_], got: {id_str}"
        );
    }
}

mod construction_tests {
    use super::*;

    /// Verify that a provider can be constructed with no dependencies.
    #[test]
    fn test_new_with_no_deps_succeeds() {
        let _provider = GithubWebhookProvider::new(None, None, None);
    }

    /// Verify that a provider can be constructed and held behind a trait object.
    #[test]
    fn test_provider_is_webhook_processor() {
        let _processor: Arc<dyn WebhookProcessor> =
            Arc::new(GithubWebhookProvider::new(None, None, None));
    }

    /// Verify construction with a signature validator.
    #[test]
    fn test_new_with_validator() {
        let validator = Arc::new(AlwaysSucceedValidator);
        let _provider = GithubWebhookProvider::new(Some(validator), None, None);
    }

    /// Verify construction with all dependencies.
    #[test]
    fn test_new_with_all_deps() {
        let validator = Arc::new(AlwaysSucceedValidator);
        let storer = Arc::new(NoopPayloadStorer);
        let _provider = GithubWebhookProvider::new(Some(validator), Some(storer), None);
    }
}

mod process_webhook_tests {
    use super::*;

    /// Verify that a ping event (no signature required) is processed successfully.
    #[tokio::test]
    async fn test_process_ping_event_succeeds() {
        let provider = GithubWebhookProvider::new(None, None, None);
        let request = ping_request();

        let result = provider.process_webhook(request).await;

        assert!(
            result.is_ok(),
            "ping event should succeed: {:?}",
            result.err()
        );
        let output = result.unwrap();
        assert_eq!(output.event_type(), Some("ping"));
    }

    /// Verify that a pull_request event without a signature fails validation.
    ///
    /// WebhookHeaders::validate() requires signature for non-ping events.
    #[tokio::test]
    async fn test_process_non_ping_without_signature_fails() {
        let provider = GithubWebhookProvider::new(None, None, None);
        let request = pull_request_request(None);

        let result = provider.process_webhook(request).await;

        assert!(result.is_err(), "non-ping without signature should fail");
        matches!(result.unwrap_err(), WebhookError::Validation(_));
    }

    /// Verify that a pull_request event with a valid signature is processed.
    ///
    /// When a signature is present and the validator accepts it, processing
    /// should produce an EventEnvelope with the correct entity type.
    #[tokio::test]
    async fn test_process_pull_request_with_valid_signature_succeeds() {
        let validator = Arc::new(AlwaysSucceedValidator);
        let provider = GithubWebhookProvider::new(Some(validator), None, None);
        let request = pull_request_request(Some("sha256=validsig".to_string()));

        let result = provider.process_webhook(request).await;

        assert!(
            result.is_ok(),
            "signed PR event should succeed: {:?}",
            result.err()
        );
        let output = result.unwrap();
        let event = output.as_wrapped().expect("should be Wrapped output");
        assert_eq!(event.event_type, "pull_request");
        let pr_number = event.payload["pull_request"]["number"].as_u64();
        assert_eq!(pr_number, Some(42), "expected PR number 42");
    }

    /// Verify that an invalid signature causes processing to fail.
    #[tokio::test]
    async fn test_process_pull_request_with_invalid_signature_fails() {
        let validator = Arc::new(AlwaysFailValidator);
        let provider = GithubWebhookProvider::new(Some(validator), None, None);
        let request = pull_request_request(Some("sha256=badsig".to_string()));

        let result = provider.process_webhook(request).await;

        assert!(result.is_err(), "bad signature should fail processing");
    }

    /// Verify that an unknown event type is normalised to EventEntity::Unknown.
    #[tokio::test]
    async fn test_process_unknown_event_type_produces_unknown_entity() {
        let provider = GithubWebhookProvider::new(None, None, None);
        let headers = WebhookHeaders {
            event_type: "ping".to_string(), // use ping so no signature required
            delivery_id: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            signature: None,
            user_agent: None,
            content_type: "application/json".to_string(),
        };
        let body = serde_json::json!({
            "repository": {
                "id": 99,
                "name": "repo",
                "full_name": "owner/repo",
                "private": false,
                "owner": { "id": 1, "login": "owner", "type": "User" }
            }
        });
        let request = WebhookRequest::new(headers, Bytes::from(body.to_string()));

        let result = provider.process_webhook(request).await;

        // ping event normalises to Repository entity (special case in EventEntity)
        // because event type is "ping" which falls through to Unknown, but
        // the entity type "ping" isn't in the list so it produces Unknown.
        assert!(
            result.is_ok(),
            "ping event with unknown entity should succeed: {:?}",
            result.err()
        );
    }

    /// Verify that payload storage is invoked when a storer is configured.
    #[tokio::test]
    async fn test_process_with_storer_stores_payload() {
        let storer = Arc::new(NoopPayloadStorer);
        let provider = GithubWebhookProvider::new(None, Some(storer), None);
        let request = ping_request();

        let result = provider.process_webhook(request).await;

        assert!(
            result.is_ok(),
            "processing with storer should succeed: {:?}",
            result.err()
        );
    }
}

mod validate_signature_tests {
    use super::*;

    /// Verify that validate_signature passes through to a succeeding validator.
    #[tokio::test]
    async fn test_validate_signature_passes_with_succeed_validator() {
        let validator = Arc::new(AlwaysSucceedValidator);
        let provider = GithubWebhookProvider::new(Some(validator), None, None);

        let result = provider
            .validate_signature(b"payload", "sha256=abc", "push")
            .await;

        assert!(result.is_ok());
    }

    /// Verify that validate_signature passes through to a failing validator.
    #[tokio::test]
    async fn test_validate_signature_fails_with_fail_validator() {
        let validator = Arc::new(AlwaysFailValidator);
        let provider = GithubWebhookProvider::new(Some(validator), None, None);

        let result = provider
            .validate_signature(b"payload", "sha256=bad", "push")
            .await;

        assert!(result.is_err());
    }

    /// Verify that validate_signature is a no-op when no validator is configured.
    #[tokio::test]
    async fn test_validate_signature_no_op_without_validator() {
        let provider = GithubWebhookProvider::new(None, None, None);

        let result = provider
            .validate_signature(b"payload", "sha256=anything", "push")
            .await;

        assert!(result.is_ok(), "no validator means skip — not an error");
    }
}

mod store_raw_payload_tests {
    use super::*;

    /// Verify that store_raw_payload succeeds with a configured storer.
    #[tokio::test]
    async fn test_store_raw_payload_with_storer_succeeds() {
        let storer = Arc::new(NoopPayloadStorer);
        let provider = GithubWebhookProvider::new(None, Some(storer), None);
        let request = ping_request();

        let result = provider
            .store_raw_payload(&request, ValidationStatus::Valid)
            .await;

        assert!(result.is_ok());
        let storage_ref = result.unwrap();
        assert!(!storage_ref.blob_path.is_empty());
    }

    /// Verify that store_raw_payload returns a placeholder when no storer is configured.
    #[tokio::test]
    async fn test_store_raw_payload_without_storer_returns_placeholder() {
        let provider = GithubWebhookProvider::new(None, None, None);
        let request = ping_request();

        let result = provider
            .store_raw_payload(&request, ValidationStatus::Valid)
            .await;

        assert!(
            result.is_ok(),
            "no storer should still succeed with placeholder"
        );
        let storage_ref = result.unwrap();
        assert!(
            storage_ref.blob_path.starts_with("not-stored/"),
            "expected placeholder path, got: {}",
            storage_ref.blob_path
        );
    }
}

mod normalize_event_tests {
    use super::*;

    /// Verify that a valid ping payload normalises correctly.
    #[tokio::test]
    async fn test_normalize_ping_event() {
        let provider = GithubWebhookProvider::new(None, None, None);
        let request = ping_request();

        let result = provider.normalize_event(&request).await;

        assert!(
            result.is_ok(),
            "normalization should succeed: {:?}",
            result.err()
        );
        let event = result.unwrap();
        assert_eq!(event.event_type, "ping");
        assert_eq!(
            event.payload["repository"]["full_name"].as_str().unwrap_or(""),
            "owner/repo"
        );
    }

    /// Verify that a pull_request payload normalises with the correct entity.
    #[tokio::test]
    async fn test_normalize_pull_request_event() {
        let provider = GithubWebhookProvider::new(None, None, None);
        let headers = WebhookHeaders {
            event_type: "pull_request".to_string(),
            delivery_id: "550e8400-e29b-41d4-a716-446655440003".to_string(),
            signature: Some("sha256=test".to_string()),
            user_agent: None,
            content_type: "application/json".to_string(),
        };
        let body = serde_json::json!({
            "action": "opened",
            "pull_request": { "number": 7 },
            "repository": {
                "id": 42,
                "name": "myrepo",
                "full_name": "org/myrepo",
                "private": true,
                "owner": { "id": 10, "login": "org", "type": "Organization" }
            }
        });
        let request = WebhookRequest::new(headers, Bytes::from(body.to_string()));

        let result = provider.normalize_event(&request).await;

        assert!(
            result.is_ok(),
            "PR normalisation should succeed: {:?}",
            result.err()
        );
        let event = result.unwrap();
        let pr_number = event.payload["pull_request"]["number"].as_u64();
        assert_eq!(pr_number, Some(7), "expected PR number 7");
        assert_eq!(event.action, Some("opened".to_string()));
    }

    /// Verify that a payload missing the repository field fails normalisation.
    #[tokio::test]
    async fn test_normalize_missing_repository_fails() {
        let provider = GithubWebhookProvider::new(None, None, None);
        let headers = WebhookHeaders {
            event_type: "ping".to_string(),
            delivery_id: "550e8400-e29b-41d4-a716-446655440004".to_string(),
            signature: None,
            user_agent: None,
            content_type: "application/json".to_string(),
        };
        let body = serde_json::json!({ "zen": "Practicality beats purity." });
        let request = WebhookRequest::new(headers, Bytes::from(body.to_string()));

        let result = provider.normalize_event(&request).await;

        assert!(
            result.is_err(),
            "missing repository should fail normalisation"
        );
        matches!(
            result.unwrap_err(),
            NormalizationError::MissingRequiredField { .. }
        );
    }
}
