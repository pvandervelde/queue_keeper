//! Tests for webhook processing module.

use super::*;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

// ============================================================================
// Mock Implementations for Testing
// ============================================================================

struct MockSignatureValidator {
    should_fail: bool,
}

#[async_trait]
impl SignatureValidator for MockSignatureValidator {
    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _secret_key: &str,
    ) -> Result<(), ValidationError> {
        if self.should_fail {
            Err(ValidationError::InvalidFormat {
                field: "signature".to_string(),
                message: "invalid signature".to_string(),
            })
        } else {
            Ok(())
        }
    }

    async fn get_webhook_secret(&self, _event_type: &str) -> Result<String, SecretError> {
        Ok("test-secret".to_string())
    }

    fn supports_constant_time_comparison(&self) -> bool {
        true
    }
}

struct MockPayloadStorer {
    should_fail: bool,
}

#[async_trait]
impl PayloadStorer for MockPayloadStorer {
    async fn store_payload(
        &self,
        request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        if self.should_fail {
            Err(StorageError::Unavailable {
                message: "storage unavailable".to_string(),
            })
        } else {
            Ok(StorageReference {
                blob_path: format!("2025/11/24/{}.json", request.delivery_id()),
                stored_at: Timestamp::now(),
                size_bytes: request.body.len() as u64,
            })
        }
    }

    async fn retrieve_payload(
        &self,
        _storage_ref: &StorageReference,
    ) -> Result<WebhookRequest, StorageError> {
        unimplemented!("Not needed for these tests")
    }

    async fn list_payloads(
        &self,
        _filters: PayloadFilters,
    ) -> Result<Vec<StorageReference>, StorageError> {
        unimplemented!("Not needed for these tests")
    }
}

// ============================================================================
// Test Helper Functions
// ============================================================================

fn create_test_headers() -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("x-github-event".to_string(), "push".to_string());
    headers.insert(
        "x-github-delivery".to_string(),
        "12345678-1234-1234-1234-123456789abc".to_string(),
    );
    headers.insert("content-type".to_string(), "application/json".to_string());
    headers.insert(
        "x-hub-signature-256".to_string(),
        "sha256=test-signature".to_string(),
    );
    headers
}

fn create_test_repository() -> Repository {
    Repository::new(
        RepositoryId::new(12345),
        "test-repo".to_string(),
        "owner/test-repo".to_string(),
        User {
            id: UserId::new(1),
            login: "owner".to_string(),
            user_type: UserType::User,
        },
        false,
    )
}

fn create_pr_payload() -> serde_json::Value {
    json!({
        "action": "opened",
        "pull_request": {
            "number": 123,
            "title": "Test PR",
            "state": "open"
        },
        "repository": {
            "id": 12345,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "private": false,
            "owner": {
                "id": 1,
                "login": "owner",
                "type": "User"
            }
        }
    })
}

// ============================================================================
// Task 12.1: WebhookRequest Parsing and Validation Tests
// ============================================================================

mod webhook_request_tests {
    use super::*;

    #[test]
    fn test_valid_webhook_with_all_headers() {
        let headers = create_test_headers();
        let webhook_headers = WebhookHeaders::from_http_headers(&headers);
        assert!(webhook_headers.is_ok());

        let headers = webhook_headers.unwrap();
        assert_eq!(headers.event_type, "push");
        assert_eq!(headers.delivery_id, "12345678-1234-1234-1234-123456789abc");
        assert_eq!(headers.signature, Some("sha256=test-signature".to_string()));
    }

    #[test]
    fn test_missing_event_type_header() {
        let mut headers = create_test_headers();
        headers.remove("x-github-event");

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_err());
        match result {
            Err(ValidationError::Required { field }) => {
                assert_eq!(field, "X-GitHub-Event");
            }
            _ => panic!("Expected Required error for X-GitHub-Event"),
        }
    }

    #[test]
    fn test_missing_delivery_id_header() {
        let mut headers = create_test_headers();
        headers.remove("x-github-delivery");

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_err());
        match result {
            Err(ValidationError::Required { field }) => {
                assert_eq!(field, "X-GitHub-Delivery");
            }
            _ => panic!("Expected Required error for X-GitHub-Delivery"),
        }
    }

    #[test]
    fn test_invalid_delivery_id_format() {
        let mut headers = create_test_headers();
        headers.insert("x-github-delivery".to_string(), "not-a-uuid".to_string());

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_err());
        match result {
            Err(ValidationError::InvalidFormat { field, .. }) => {
                assert_eq!(field, "delivery_id");
            }
            _ => panic!("Expected InvalidFormat error for delivery_id"),
        }
    }

    #[test]
    fn test_missing_signature_for_non_ping_event() {
        let mut headers = create_test_headers();
        headers.remove("x-hub-signature-256");

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_err());
        match result {
            Err(ValidationError::Required { field }) => {
                assert_eq!(field, "signature");
            }
            _ => panic!("Expected Required error for signature"),
        }
    }

    #[test]
    fn test_ping_event_without_signature() {
        let mut headers = create_test_headers();
        headers.insert("x-github-event".to_string(), "ping".to_string());
        headers.remove("x-hub-signature-256");

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_content_type() {
        let mut headers = create_test_headers();
        headers.insert("content-type".to_string(), "text/plain".to_string());

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_err());
        match result {
            Err(ValidationError::InvalidFormat { field, .. }) => {
                assert_eq!(field, "content_type");
            }
            _ => panic!("Expected InvalidFormat error for content_type"),
        }
    }

    #[test]
    fn test_lowercase_header_parsing() {
        let mut headers = HashMap::new();
        headers.insert("x-github-event".to_string(), "push".to_string());
        headers.insert(
            "x-github-delivery".to_string(),
            "12345678-1234-1234-1234-123456789abc".to_string(),
        );
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert(
            "x-hub-signature-256".to_string(),
            "sha256=test-signature".to_string(),
        );

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_webhook_request_creation() {
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let body = Bytes::from("{}");
        let request = WebhookRequest::new(headers, body.clone());

        assert_eq!(request.event_type(), "push");
        assert_eq!(
            request.delivery_id(),
            "12345678-1234-1234-1234-123456789abc"
        );
        assert_eq!(request.signature(), Some("sha256=test-signature"));
        assert_eq!(request.body, body);
    }

    #[test]
    fn test_empty_body_accepted() {
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let body = Bytes::new();
        let request = WebhookRequest::new(headers, body);

        assert_eq!(request.body.len(), 0);
    }
}

// ============================================================================
// Task 12.2: Signature Validation Integration Tests
// ============================================================================

mod signature_validation_tests {
    use super::*;

    #[tokio::test]
    async fn test_valid_signature_succeeds() {
        let validator = Arc::new(MockSignatureValidator { should_fail: false });
        let processor = WebhookProcessorImpl::new(Some(validator), None, None);

        let result = processor
            .validate_signature(b"test payload", "sha256=valid", "push")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_signature_fails() {
        let validator = Arc::new(MockSignatureValidator { should_fail: true });
        let processor = WebhookProcessorImpl::new(Some(validator), None, None);

        let result = processor
            .validate_signature(b"test payload", "sha256=invalid", "push")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_signature_validation_without_validator() {
        let processor = WebhookProcessorImpl::new(None, None, None);

        let result = processor
            .validate_signature(b"test payload", "sha256=any", "push")
            .await;

        // Should skip validation gracefully when no validator provided
        assert!(result.is_ok());
    }
}

// ============================================================================
// Task 12.3: Event Normalization Tests
// ============================================================================

mod event_normalization_tests {
    use super::*;

    #[tokio::test]
    async fn test_pull_request_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let mut headers = create_test_headers();
        headers.insert("x-github-event".to_string(), "pull_request".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert_eq!(event.event_type, "pull_request");
        assert_eq!(event.action, Some("opened".to_string()));
        assert!(
            event
                .session_id
                .as_ref()
                .unwrap()
                .as_str()
                .contains("pull_request/123"),
            "session_id should encode entity: {:?}",
            event.session_id
        );
    }

    #[tokio::test]
    async fn test_issue_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let mut headers = create_test_headers();
        headers.insert("x-github-event".to_string(), "issues".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();

        let payload = json!({
            "action": "opened",
            "issue": {
                "number": 456
            },
            "repository": {
                "id": 12345,
                "name": "test-repo",
                "full_name": "owner/test-repo",
                "private": false,
                "owner": {
                    "id": 1,
                    "login": "owner",
                    "type": "User"
                }
            }
        });

        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert_eq!(event.event_type, "issues");
        assert!(
            event
                .session_id
                .as_ref()
                .unwrap()
                .as_str()
                .contains("issue/456"),
            "session_id should encode entity: {:?}",
            event.session_id
        );
    }

    #[tokio::test]
    async fn test_push_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();

        let payload = json!({
            "ref": "refs/heads/main",
            "commits": [],
            "repository": {
                "id": 12345,
                "name": "test-repo",
                "full_name": "owner/test-repo",
                "private": false,
                "owner": {
                    "id": 1,
                    "login": "owner",
                    "type": "User"
                }
            }
        });

        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(
            event
                .session_id
                .as_ref()
                .unwrap()
                .as_str()
                .contains("branch/main"),
            "session_id should encode branch: {:?}",
            event.session_id
        );
    }

    #[tokio::test]
    async fn test_release_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let mut headers = create_test_headers();
        headers.insert("x-github-event".to_string(), "release".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();

        let payload = json!({
            "action": "published",
            "release": {
                "tag_name": "v1.0.0"
            },
            "repository": {
                "id": 12345,
                "name": "test-repo",
                "full_name": "owner/test-repo",
                "private": false,
                "owner": {
                    "id": 1,
                    "login": "owner",
                    "type": "User"
                }
            }
        });

        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(
            event
                .session_id
                .as_ref()
                .unwrap()
                .as_str()
                .contains("release/v1.0.0"),
            "session_id should encode release: {:?}",
            event.session_id
        );
    }

    #[tokio::test]
    async fn test_repository_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let mut headers = create_test_headers();
        headers.insert("x-github-event".to_string(), "repository".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();

        let payload = json!({
            "action": "created",
            "repository": {
                "id": 12345,
                "name": "test-repo",
                "full_name": "owner/test-repo",
                "private": false,
                "owner": {
                    "id": 1,
                    "login": "owner",
                    "type": "User"
                }
            }
        });

        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(
            event
                .session_id
                .as_ref()
                .unwrap()
                .as_str()
                .contains("repository/repository"),
            "session_id should encode repository entity: {:?}",
            event.session_id
        );
    }

    #[tokio::test]
    async fn test_unknown_event_type_normalization() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let mut headers = create_test_headers();
        headers.insert("x-github-event".to_string(), "unknown_event".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();

        let payload = json!({
            "repository": {
                "id": 12345,
                "name": "test-repo",
                "full_name": "owner/test-repo",
                "private": false,
                "owner": {
                    "id": 1,
                    "login": "owner",
                    "type": "User"
                }
            }
        });

        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(
            event
                .session_id
                .as_ref()
                .unwrap()
                .as_str()
                .contains("unknown/unknown"),
            "session_id should encode unknown entity: {:?}",
            event.session_id
        );
    }

    #[tokio::test]
    async fn test_missing_repository_field() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();

        let payload = json!({
            "action": "opened"
        });

        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_err());
        match result {
            Err(NormalizationError::MissingRequiredField { field }) => {
                assert_eq!(field, "repository");
            }
            _ => panic!("Expected MissingRequiredField error"),
        }
    }

    #[tokio::test]
    async fn test_event_with_action_field() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert_eq!(event.action, Some("opened".to_string()));
    }

    #[tokio::test]
    async fn test_event_without_action_field() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();

        let payload = json!({
            "ref": "refs/heads/main",
            "repository": {
                "id": 12345,
                "name": "test-repo",
                "full_name": "owner/test-repo",
                "private": false,
                "owner": {
                    "id": 1,
                    "login": "owner",
                    "type": "User"
                }
            }
        });

        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert_eq!(event.action, None);
    }

    #[tokio::test]
    async fn test_timestamp_generation() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        assert!(event.received_at.as_datetime() <= &chrono::Utc::now());
        assert!(event.processed_at.as_datetime() <= &chrono::Utc::now());
    }

    #[tokio::test]
    async fn test_event_id_generation() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        // EventId is ULID format - should be parseable
        let id_str = event.event_id.as_str();
        assert!(!id_str.is_empty());
    }

    #[tokio::test]
    async fn test_correlation_id_generation() {
        let processor = WebhookProcessorImpl::new(None, None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let event = result.unwrap();
        // CorrelationId is UUID format
        let id_str = event.correlation_id.as_str();
        assert!(!id_str.is_empty());
    }
}

// ============================================================================
// Task 12.4: Session ID Generation Tests
// ============================================================================

mod session_id_generation_tests {
    use super::*;

    #[test]
    fn test_pull_request_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::PullRequest { number: 123 };
        let session_id = generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/pull_request/123");
    }

    #[test]
    fn test_issue_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Issue { number: 456 };
        let session_id = generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/issue/456");
    }

    #[test]
    fn test_branch_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Branch {
            name: "main".to_string(),
        };
        let session_id = generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/branch/main");
    }

    #[test]
    fn test_release_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Release {
            tag: "v1.0.0".to_string(),
        };
        let session_id = generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/release/v1.0.0");
    }

    #[test]
    fn test_repository_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Repository;
        let session_id = generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/repository/repository");
    }

    #[test]
    fn test_unknown_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Unknown;
        let session_id = generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/unknown/unknown");
    }

    #[test]
    fn test_session_id_max_length() {
        let repository = create_test_repository();
        let entity = EventEntity::PullRequest { number: 123 };
        let session_id = generate_session_id(&repository, &entity);

        // Session IDs must not exceed 128 characters
        assert!(session_id.as_str().len() <= 128);
    }
}

// ============================================================================
// Task 12.5: Error Handling and Classification Tests
// ============================================================================

mod error_handling_tests {
    use super::*;

    #[test]
    fn test_validation_error_classification() {
        let error = WebhookError::Validation(ValidationError::Required {
            field: "test".to_string(),
        });

        assert_eq!(error.error_category(), crate::ErrorCategory::Permanent);
        assert!(!error.should_retry());
        assert!(!error.is_transient());
    }

    #[test]
    fn test_invalid_signature_classification() {
        let error = WebhookError::InvalidSignature("test".to_string());

        assert_eq!(error.error_category(), crate::ErrorCategory::Security);
        assert!(!error.should_retry());
        assert!(!error.is_transient());
    }

    #[test]
    fn test_storage_error_transient_classification() {
        let storage_error = StorageError::Unavailable {
            message: "temporary failure".to_string(),
        };
        let error = WebhookError::Storage(storage_error);

        assert_eq!(error.error_category(), crate::ErrorCategory::Transient);
        assert!(error.should_retry());
        assert!(error.is_transient());
    }

    #[test]
    fn test_storage_error_permanent_classification() {
        let storage_error = StorageError::PayloadTooLarge { size: 2_000_000 };
        let error = WebhookError::Storage(storage_error);

        assert_eq!(error.error_category(), crate::ErrorCategory::Permanent);
        assert!(!error.should_retry());
        assert!(!error.is_transient());
    }

    #[test]
    fn test_unknown_event_type_classification() {
        let error = WebhookError::UnknownEventType {
            event_type: "unknown".to_string(),
        };

        assert_eq!(error.error_category(), crate::ErrorCategory::Permanent);
        assert!(!error.should_retry());
        assert!(!error.is_transient());
    }

    #[test]
    fn test_malformed_payload_classification() {
        let error = WebhookError::MalformedPayload {
            message: "invalid JSON".to_string(),
        };

        assert_eq!(error.error_category(), crate::ErrorCategory::Permanent);
        assert!(!error.should_retry());
        assert!(!error.is_transient());
    }

    #[test]
    fn test_normalization_error_classification() {
        let norm_error = NormalizationError::MissingRequiredField {
            field: "repository".to_string(),
        };
        let error = WebhookError::Normalization(norm_error);

        assert_eq!(error.error_category(), crate::ErrorCategory::Permanent);
        assert!(!error.should_retry());
        assert!(!error.is_transient());
    }

    #[test]
    fn test_json_parsing_error_classification() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let error = WebhookError::JsonParsing(json_error);

        assert_eq!(error.error_category(), crate::ErrorCategory::Permanent);
        assert!(!error.should_retry());
        assert!(!error.is_transient());
    }
}

// ============================================================================
// Integration Tests for Complete Pipeline
// ============================================================================

mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_webhook_processing_pipeline() {
        let validator = Arc::new(MockSignatureValidator { should_fail: false });
        let storer = Arc::new(MockPayloadStorer { should_fail: false });
        let processor = WebhookProcessorImpl::new(Some(validator), Some(storer), None);

        let mut headers = create_test_headers();
        headers.insert("x-github-event".to_string(), "pull_request".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.process_webhook(request).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        let event = output.as_wrapped().expect("should be Wrapped output");
        assert_eq!(event.event_type, "pull_request");
        assert!(
            event
                .session_id
                .as_ref()
                .unwrap()
                .as_str()
                .contains("pull_request/123"),
            "session_id should encode entity: {:?}",
            event.session_id
        );
    }

    #[tokio::test]
    async fn test_pipeline_with_signature_failure() {
        let validator = Arc::new(MockSignatureValidator { should_fail: true });
        let storer = Arc::new(MockPayloadStorer { should_fail: false });
        let processor = WebhookProcessorImpl::new(Some(validator), Some(storer), None);

        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.process_webhook(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pipeline_with_storage_failure() {
        let validator = Arc::new(MockSignatureValidator { should_fail: false });
        let storer = Arc::new(MockPayloadStorer { should_fail: true });
        let processor = WebhookProcessorImpl::new(Some(validator), Some(storer), None);

        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.process_webhook(request).await;
        assert!(result.is_err());
        match result {
            Err(WebhookError::Storage(_)) => {}
            _ => panic!("Expected Storage error"),
        }
    }
}

// ============================================================================
// Entity Extraction Edge Cases
// ============================================================================

mod entity_extraction_tests {
    use super::*;

    #[test]
    fn test_pull_request_event() {
        let payload = json!({
            "action": "opened",
            "pull_request": {
                "number": 123
            }
        });
        let entity = EventEntity::from_payload("pull_request", &payload);
        assert_eq!(entity, EventEntity::PullRequest { number: 123 });
    }

    #[test]
    fn test_issue_event() {
        let payload = json!({
            "action": "opened",
            "issue": {
                "number": 456
            }
        });
        let entity = EventEntity::from_payload("issues", &payload);
        assert_eq!(entity, EventEntity::Issue { number: 456 });
    }

    #[test]
    fn test_push_event() {
        let payload = json!({
            "ref": "refs/heads/main",
            "commits": []
        });
        let entity = EventEntity::from_payload("push", &payload);
        assert_eq!(
            entity,
            EventEntity::Branch {
                name: "main".to_string()
            }
        );
    }

    #[test]
    fn test_release_event() {
        let payload = json!({
            "release": {
                "tag_name": "v1.0.0"
            }
        });
        let entity = EventEntity::from_payload("release", &payload);
        assert_eq!(
            entity,
            EventEntity::Release {
                tag: "v1.0.0".to_string()
            }
        );
    }

    #[test]
    fn test_missing_pull_request_number() {
        let payload = json!({
            "action": "opened",
            "pull_request": {}
        });
        let entity = EventEntity::from_payload("pull_request", &payload);
        assert_eq!(entity, EventEntity::Unknown);
    }

    #[test]
    fn test_missing_issue_number() {
        let payload = json!({
            "action": "opened",
            "issue": {}
        });
        let entity = EventEntity::from_payload("issues", &payload);
        assert_eq!(entity, EventEntity::Unknown);
    }

    #[test]
    fn test_missing_ref_in_push() {
        let payload = json!({
            "commits": []
        });
        let entity = EventEntity::from_payload("push", &payload);
        assert_eq!(entity, EventEntity::Unknown);
    }
}

// ============================================================================
// New EventEntity Variants: Discussion, WorkflowRun, Team
// ============================================================================

mod event_entity_new_variants_tests {
    use super::*;

    // ------------------------------------------------------------------
    // Discussion variant
    // ------------------------------------------------------------------

    /// Verify Discussion entity_type() returns "discussion".
    #[test]
    fn test_discussion_entity_type() {
        let entity = EventEntity::Discussion { number: 42 };
        assert_eq!(entity.entity_type(), "discussion");
    }

    /// Verify Discussion entity_id() returns the discussion number as a string.
    #[test]
    fn test_discussion_entity_id() {
        let entity = EventEntity::Discussion { number: 42 };
        assert_eq!(entity.entity_id(), "42");
    }

    /// Verify Discussion produces the expected session ID format.
    #[test]
    fn test_discussion_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Discussion { number: 42 };
        let session_id = generate_session_id(&repository, &entity);
        assert_eq!(session_id.as_str(), "owner/test-repo/discussion/42");
    }

    /// Verify Discussion number zero is represented correctly.
    #[test]
    fn test_discussion_entity_id_zero() {
        let entity = EventEntity::Discussion { number: 0 };
        assert_eq!(entity.entity_id(), "0");
    }

    /// Verify Discussion serializes and deserializes via serde without loss.
    #[test]
    fn test_discussion_serde_roundtrip() {
        let entity = EventEntity::Discussion { number: 99 };
        let json = serde_json::to_string(&entity).expect("serialization failed");
        let round_tripped: EventEntity =
            serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(entity, round_tripped);
    }

    // ------------------------------------------------------------------
    // WorkflowRun variant
    // ------------------------------------------------------------------

    /// Verify WorkflowRun entity_type() returns "workflow_run".
    #[test]
    fn test_workflow_run_entity_type() {
        let entity = EventEntity::WorkflowRun { id: 9999 };
        assert_eq!(entity.entity_type(), "workflow_run");
    }

    /// Verify WorkflowRun entity_id() returns the run ID as a string.
    #[test]
    fn test_workflow_run_entity_id() {
        let entity = EventEntity::WorkflowRun { id: 9999 };
        assert_eq!(entity.entity_id(), "9999");
    }

    /// Verify WorkflowRun produces the expected session ID format.
    #[test]
    fn test_workflow_run_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::WorkflowRun { id: 9999 };
        let session_id = generate_session_id(&repository, &entity);
        assert_eq!(session_id.as_str(), "owner/test-repo/workflow_run/9999");
    }

    /// Verify WorkflowRun handles large u64 IDs (GitHub run IDs can be very large).
    #[test]
    fn test_workflow_run_large_id() {
        let large_id: u64 = 12_345_678_901;
        let entity = EventEntity::WorkflowRun { id: large_id };
        assert_eq!(entity.entity_id(), "12345678901");
    }

    /// Verify WorkflowRun serializes and deserializes via serde without loss.
    #[test]
    fn test_workflow_run_serde_roundtrip() {
        let entity = EventEntity::WorkflowRun { id: 9999 };
        let json = serde_json::to_string(&entity).expect("serialization failed");
        let round_tripped: EventEntity =
            serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(entity, round_tripped);
    }

    // ------------------------------------------------------------------
    // Team variant
    // ------------------------------------------------------------------

    /// Verify Team entity_type() returns "team".
    #[test]
    fn test_team_entity_type() {
        let entity = EventEntity::Team {
            slug: "backend".to_string(),
        };
        assert_eq!(entity.entity_type(), "team");
    }

    /// Verify Team entity_id() returns the team slug string.
    #[test]
    fn test_team_entity_id() {
        let entity = EventEntity::Team {
            slug: "backend".to_string(),
        };
        assert_eq!(entity.entity_id(), "backend");
    }

    /// Verify Team produces the expected session ID format.
    #[test]
    fn test_team_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Team {
            slug: "backend".to_string(),
        };
        let session_id = generate_session_id(&repository, &entity);
        assert_eq!(session_id.as_str(), "owner/test-repo/team/backend");
    }

    /// Verify Team slug with hyphens is preserved exactly (common GitHub slug format).
    #[test]
    fn test_team_slug_with_hyphens() {
        let entity = EventEntity::Team {
            slug: "platform-engineering".to_string(),
        };
        assert_eq!(entity.entity_id(), "platform-engineering");
    }

    /// Verify Team serializes and deserializes via serde without loss.
    #[test]
    fn test_team_serde_roundtrip() {
        let entity = EventEntity::Team {
            slug: "backend".to_string(),
        };
        let json = serde_json::to_string(&entity).expect("serialization failed");
        let round_tripped: EventEntity =
            serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(entity, round_tripped);
    }

    // ------------------------------------------------------------------
    // PartialEq / Clone coverage for new variants
    // ------------------------------------------------------------------

    /// Verify Discussion equality works correctly.
    #[test]
    fn test_discussion_equality() {
        assert_eq!(
            EventEntity::Discussion { number: 1 },
            EventEntity::Discussion { number: 1 }
        );
        assert_ne!(
            EventEntity::Discussion { number: 1 },
            EventEntity::Discussion { number: 2 }
        );
    }

    /// Verify WorkflowRun equality works correctly.
    #[test]
    fn test_workflow_run_equality() {
        assert_eq!(
            EventEntity::WorkflowRun { id: 100 },
            EventEntity::WorkflowRun { id: 100 }
        );
        assert_ne!(
            EventEntity::WorkflowRun { id: 100 },
            EventEntity::WorkflowRun { id: 200 }
        );
    }

    /// Verify Team equality works correctly.
    #[test]
    fn test_team_equality() {
        assert_eq!(
            EventEntity::Team {
                slug: "a".to_string()
            },
            EventEntity::Team {
                slug: "a".to_string()
            }
        );
        assert_ne!(
            EventEntity::Team {
                slug: "a".to_string()
            },
            EventEntity::Team {
                slug: "b".to_string()
            }
        );
    }

    /// Verify new variants can be cloned.
    #[test]
    fn test_new_variants_clone() {
        let d = EventEntity::Discussion { number: 1 };
        let w = EventEntity::WorkflowRun { id: 2 };
        let t = EventEntity::Team {
            slug: "eng".to_string(),
        };
        assert_eq!(d.clone(), d);
        assert_eq!(w.clone(), w);
        assert_eq!(t.clone(), t);
    }
}

// ============================================================================
// from_payload() for ~25 Additional GitHub Event Types (issue #82)
// ============================================================================

mod event_entity_new_events_tests {
    use super::*;

    // ------------------------------------------------------------------
    // discussion / discussion_comment  →  Discussion { number }
    // ------------------------------------------------------------------

    /// Happy path: `discussion` event with discussion.number present.
    #[test]
    fn test_discussion_event_maps_to_discussion_entity() {
        let payload = json!({ "discussion": { "number": 7 } });
        assert_eq!(
            EventEntity::from_payload("discussion", &payload),
            EventEntity::Discussion { number: 7 }
        );
    }

    /// Happy path: `discussion_comment` event uses the same discussion.number path.
    #[test]
    fn test_discussion_comment_event_maps_to_discussion_entity() {
        let payload = json!({ "discussion": { "number": 42 } });
        assert_eq!(
            EventEntity::from_payload("discussion_comment", &payload),
            EventEntity::Discussion { number: 42 }
        );
    }

    /// Missing `discussion.number` must fall back to Unknown, not Repository.
    #[test]
    fn test_discussion_event_missing_number_returns_unknown() {
        let payload = json!({ "discussion": {} });
        assert_eq!(
            EventEntity::from_payload("discussion", &payload),
            EventEntity::Unknown
        );
    }

    /// Entirely absent `discussion` key must fall back to Unknown.
    #[test]
    fn test_discussion_event_missing_discussion_key_returns_unknown() {
        let payload = json!({});
        assert_eq!(
            EventEntity::from_payload("discussion", &payload),
            EventEntity::Unknown
        );
    }

    // ------------------------------------------------------------------
    // workflow_run  →  WorkflowRun { id }
    // ------------------------------------------------------------------

    /// Happy path: `workflow_run` event with workflow_run.id present.
    #[test]
    fn test_workflow_run_event_maps_to_workflow_run_entity() {
        let payload = json!({ "workflow_run": { "id": 9999_u64 } });
        assert_eq!(
            EventEntity::from_payload("workflow_run", &payload),
            EventEntity::WorkflowRun { id: 9999 }
        );
    }

    /// Missing `workflow_run.id` must fall back to Unknown.
    #[test]
    fn test_workflow_run_event_missing_id_returns_unknown() {
        let payload = json!({ "workflow_run": {} });
        assert_eq!(
            EventEntity::from_payload("workflow_run", &payload),
            EventEntity::Unknown
        );
    }

    /// Entirely absent `workflow_run` key must fall back to Unknown.
    #[test]
    fn test_workflow_run_event_missing_workflow_run_key_returns_unknown() {
        let payload = json!({});
        assert_eq!(
            EventEntity::from_payload("workflow_run", &payload),
            EventEntity::Unknown
        );
    }

    // ------------------------------------------------------------------
    // workflow_job  →  WorkflowRun { id }  (dual-path fallback)
    // ------------------------------------------------------------------

    /// Primary path: `workflow_job` with workflow_run.id present.
    #[test]
    fn test_workflow_job_uses_workflow_run_id_primary() {
        let payload = json!({
            "workflow_run": { "id": 1111_u64 },
            "workflow_job": { "run_id": 2222_u64 }
        });
        assert_eq!(
            EventEntity::from_payload("workflow_job", &payload),
            EventEntity::WorkflowRun { id: 1111 }
        );
    }

    /// Fallback: `workflow_job` without workflow_run.id falls back to workflow_job.run_id.
    #[test]
    fn test_workflow_job_falls_back_to_run_id_when_workflow_run_absent() {
        let payload = json!({ "workflow_job": { "run_id": 2222_u64 } });
        assert_eq!(
            EventEntity::from_payload("workflow_job", &payload),
            EventEntity::WorkflowRun { id: 2222 }
        );
    }

    /// Both absent: `workflow_job` with neither field present falls back to Unknown.
    #[test]
    fn test_workflow_job_returns_unknown_when_both_ids_absent() {
        let payload = json!({ "workflow_job": {} });
        assert_eq!(
            EventEntity::from_payload("workflow_job", &payload),
            EventEntity::Unknown
        );
    }

    /// Both keys entirely missing falls back to Unknown.
    #[test]
    fn test_workflow_job_empty_payload_returns_unknown() {
        let payload = json!({});
        assert_eq!(
            EventEntity::from_payload("workflow_job", &payload),
            EventEntity::Unknown
        );
    }

    // ------------------------------------------------------------------
    // team  →  Team { slug }
    // ------------------------------------------------------------------

    /// Happy path: `team` event with team.slug present.
    #[test]
    fn test_team_event_maps_to_team_entity() {
        let payload = json!({ "team": { "slug": "backend" } });
        assert_eq!(
            EventEntity::from_payload("team", &payload),
            EventEntity::Team {
                slug: "backend".to_string()
            }
        );
    }

    /// Missing `team.slug` must fall back to Unknown.
    #[test]
    fn test_team_event_missing_slug_returns_unknown() {
        let payload = json!({ "team": {} });
        assert_eq!(
            EventEntity::from_payload("team", &payload),
            EventEntity::Unknown
        );
    }

    /// Entirely absent `team` key must fall back to Unknown.
    #[test]
    fn test_team_event_missing_team_key_returns_unknown() {
        let payload = json!({});
        assert_eq!(
            EventEntity::from_payload("team", &payload),
            EventEntity::Unknown
        );
    }

    // ------------------------------------------------------------------
    // issue_dependencies  →  Issue { number }
    // ------------------------------------------------------------------

    /// Happy path: `issue_dependencies` with issue.number present.
    #[test]
    fn test_issue_dependencies_maps_to_issue_entity() {
        let payload = json!({ "issue": { "number": 88 } });
        assert_eq!(
            EventEntity::from_payload("issue_dependencies", &payload),
            EventEntity::Issue { number: 88 }
        );
    }

    /// Missing `issue.number` must fall back to Unknown.
    #[test]
    fn test_issue_dependencies_missing_number_returns_unknown() {
        let payload = json!({ "issue": {} });
        assert_eq!(
            EventEntity::from_payload("issue_dependencies", &payload),
            EventEntity::Unknown
        );
    }

    // ------------------------------------------------------------------
    // All Repository-mapped event types
    // ------------------------------------------------------------------

    /// Every Repository-mapped event type must return Self::Repository regardless
    /// of payload content (even empty payloads).
    #[test]
    fn test_repository_mapped_events_return_repository_entity() {
        let repository_events = [
            "commit_comment",
            "status",
            "custom_property",
            "custom_property_values",
            "label",
            "milestone",
            "projects_v2",
            "projects_v2_item",
            "projects_v2_status_update",
            "workflow_dispatch",
            "deploy_key",
            "deployment",
            "repository_ruleset",
            "github_app_authorization",
            "installation",
            "installation_repositories",
            "installation_target",
            "ping",
            "team_add",
        ];

        let empty_payload = json!({});
        for event_type in &repository_events {
            assert_eq!(
                EventEntity::from_payload(event_type, &empty_payload),
                EventEntity::Repository,
                "expected Repository for event type '{event_type}'"
            );
        }
    }

    /// Unknown event types must fall through to Unknown, never Repository.
    #[test]
    fn test_completely_unknown_event_type_returns_unknown() {
        let payload = json!({});
        assert_eq!(
            EventEntity::from_payload("some_future_github_event", &payload),
            EventEntity::Unknown
        );
    }
}
