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
    headers.insert("X-GitHub-Event".to_string(), "push".to_string());
    headers.insert(
        "X-GitHub-Delivery".to_string(),
        "12345678-1234-1234-1234-123456789abc".to_string(),
    );
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers.insert(
        "X-Hub-Signature-256".to_string(),
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
        headers.remove("X-GitHub-Event");

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
        headers.remove("X-GitHub-Delivery");

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
        headers.insert("X-GitHub-Delivery".to_string(), "not-a-uuid".to_string());

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
        headers.remove("X-Hub-Signature-256");

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
        headers.insert("X-GitHub-Event".to_string(), "ping".to_string());
        headers.remove("X-Hub-Signature-256");

        let result = WebhookHeaders::from_http_headers(&headers);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_content_type() {
        let mut headers = create_test_headers();
        headers.insert("Content-Type".to_string(), "text/plain".to_string());

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
    fn test_case_insensitive_header_parsing() {
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
        let processor = WebhookProcessorImpl::new(Some(validator), None);

        let result = processor
            .validate_signature(b"test payload", "sha256=valid", "push")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_signature_fails() {
        let validator = Arc::new(MockSignatureValidator { should_fail: true });
        let processor = WebhookProcessorImpl::new(Some(validator), None);

        let result = processor
            .validate_signature(b"test payload", "sha256=invalid", "push")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_signature_validation_without_validator() {
        let processor = WebhookProcessorImpl::new(None, None);

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
        let processor = WebhookProcessorImpl::new(None, None);
        let mut headers = create_test_headers();
        headers.insert("X-GitHub-Event".to_string(), "pull_request".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        assert_eq!(envelope.event_type, "pull_request");
        assert_eq!(envelope.action, Some("opened".to_string()));
        assert_eq!(envelope.entity, EventEntity::PullRequest { number: 123 });
        assert_eq!(envelope.repository.name, "test-repo");
    }

    #[tokio::test]
    async fn test_issue_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None);
        let mut headers = create_test_headers();
        headers.insert("X-GitHub-Event".to_string(), "issues".to_string());
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

        let envelope = result.unwrap();
        assert_eq!(envelope.event_type, "issues");
        assert_eq!(envelope.entity, EventEntity::Issue { number: 456 });
    }

    #[tokio::test]
    async fn test_push_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None);
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

        let envelope = result.unwrap();
        assert_eq!(
            envelope.entity,
            EventEntity::Branch {
                name: "main".to_string()
            }
        );
    }

    #[tokio::test]
    async fn test_release_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None);
        let mut headers = create_test_headers();
        headers.insert("X-GitHub-Event".to_string(), "release".to_string());
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

        let envelope = result.unwrap();
        assert_eq!(
            envelope.entity,
            EventEntity::Release {
                tag: "v1.0.0".to_string()
            }
        );
    }

    #[tokio::test]
    async fn test_repository_event_normalization() {
        let processor = WebhookProcessorImpl::new(None, None);
        let mut headers = create_test_headers();
        headers.insert("X-GitHub-Event".to_string(), "repository".to_string());
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

        let envelope = result.unwrap();
        assert_eq!(envelope.entity, EventEntity::Repository);
    }

    #[tokio::test]
    async fn test_unknown_event_type_normalization() {
        let processor = WebhookProcessorImpl::new(None, None);
        let mut headers = create_test_headers();
        headers.insert("X-GitHub-Event".to_string(), "unknown_event".to_string());
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

        let envelope = result.unwrap();
        assert_eq!(envelope.entity, EventEntity::Unknown);
    }

    #[tokio::test]
    async fn test_missing_repository_field() {
        let processor = WebhookProcessorImpl::new(None, None);
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
        let processor = WebhookProcessorImpl::new(None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        assert_eq!(envelope.action, Some("opened".to_string()));
    }

    #[tokio::test]
    async fn test_event_without_action_field() {
        let processor = WebhookProcessorImpl::new(None, None);
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

        let envelope = result.unwrap();
        assert_eq!(envelope.action, None);
    }

    #[tokio::test]
    async fn test_timestamp_generation() {
        let processor = WebhookProcessorImpl::new(None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        assert!(envelope.occurred_at.as_datetime() <= &chrono::Utc::now());
        assert!(envelope.processed_at.as_datetime() <= &chrono::Utc::now());
    }

    #[tokio::test]
    async fn test_event_id_generation() {
        let processor = WebhookProcessorImpl::new(None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        // EventId is ULID format - should be parseable
        let id_str = envelope.event_id.as_str();
        assert!(!id_str.is_empty());
    }

    #[tokio::test]
    async fn test_correlation_id_generation() {
        let processor = WebhookProcessorImpl::new(None, None);
        let headers = WebhookHeaders::from_http_headers(&create_test_headers()).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(headers, body);

        let result = processor.normalize_event(&request).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        // CorrelationId is UUID format
        let id_str = envelope.correlation_id.as_str();
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
        let session_id = EventEnvelope::generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/pull_request/123");
    }

    #[test]
    fn test_issue_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Issue { number: 456 };
        let session_id = EventEnvelope::generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/issue/456");
    }

    #[test]
    fn test_branch_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Branch {
            name: "main".to_string(),
        };
        let session_id = EventEnvelope::generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/branch/main");
    }

    #[test]
    fn test_release_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Release {
            tag: "v1.0.0".to_string(),
        };
        let session_id = EventEnvelope::generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/release/v1.0.0");
    }

    #[test]
    fn test_repository_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Repository;
        let session_id = EventEnvelope::generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/repository/repository");
    }

    #[test]
    fn test_unknown_session_id() {
        let repository = create_test_repository();
        let entity = EventEntity::Unknown;
        let session_id = EventEnvelope::generate_session_id(&repository, &entity);

        assert_eq!(session_id.as_str(), "owner/test-repo/unknown/unknown");
    }

    #[test]
    fn test_session_id_max_length() {
        let repository = create_test_repository();
        let entity = EventEntity::PullRequest { number: 123 };
        let session_id = EventEnvelope::generate_session_id(&repository, &entity);

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
        let processor = WebhookProcessorImpl::new(Some(validator), Some(storer));

        let mut headers = create_test_headers();
        headers.insert("X-GitHub-Event".to_string(), "pull_request".to_string());
        let webhook_headers = WebhookHeaders::from_http_headers(&headers).unwrap();
        let payload = create_pr_payload();
        let body = Bytes::from(serde_json::to_vec(&payload).unwrap());
        let request = WebhookRequest::new(webhook_headers, body);

        let result = processor.process_webhook(request).await;
        assert!(result.is_ok());

        let envelope = result.unwrap();
        assert_eq!(envelope.event_type, "pull_request");
        assert_eq!(envelope.entity, EventEntity::PullRequest { number: 123 });
        assert_eq!(envelope.repository.name, "test-repo");
    }

    #[tokio::test]
    async fn test_pipeline_with_signature_failure() {
        let validator = Arc::new(MockSignatureValidator { should_fail: true });
        let storer = Arc::new(MockPayloadStorer { should_fail: false });
        let processor = WebhookProcessorImpl::new(Some(validator), Some(storer));

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
        let processor = WebhookProcessorImpl::new(Some(validator), Some(storer));

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
