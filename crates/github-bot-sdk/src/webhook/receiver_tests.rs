//! Tests for WebhookReceiver.

use super::*;
use crate::auth::{GitHubAppId, PrivateKey};
use crate::error::SecretError;
use crate::events::ProcessorConfig;
use async_trait::async_trait;
use chrono::Duration;
use tokio::sync::Mutex;

// ============================================================================
// Mock Secret Provider
// ============================================================================

struct MockSecretProvider {
    secret: String,
}

impl MockSecretProvider {
    fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
        }
    }
}

#[async_trait]
impl SecretProvider for MockSecretProvider {
    async fn get_webhook_secret(&self) -> Result<String, SecretError> {
        Ok(self.secret.clone())
    }

    async fn get_private_key(&self) -> Result<PrivateKey, SecretError> {
        // Not used in webhook receiver tests
        Err(SecretError::NotFound {
            key: "private_key".to_string(),
        })
    }

    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError> {
        // Not used in webhook receiver tests
        Err(SecretError::NotFound {
            key: "app_id".to_string(),
        })
    }

    fn cache_duration(&self) -> Duration {
        Duration::minutes(5)
    }
}

// ============================================================================
// Mock Handler
// ============================================================================

#[derive(Clone)]
struct MockHandler {
    calls: Arc<Mutex<Vec<String>>>,
    delay: Option<std::time::Duration>,
}

impl MockHandler {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            delay: None,
        }
    }

    fn with_delay(delay: std::time::Duration) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            delay: Some(delay),
        }
    }

    async fn call_count(&self) -> usize {
        self.calls.lock().await.len()
    }

    async fn was_called_with_event_type(&self, event_type: &str) -> bool {
        self.calls.lock().await.contains(&event_type.to_string())
    }
}

#[async_trait]
impl WebhookHandler for MockHandler {
    async fn handle_event(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        self.calls.lock().await.push(envelope.event_type.clone());
        Ok(())
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn compute_signature(payload: &[u8], secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

fn create_webhook_request(event_type: &str, payload: &str, secret: &str) -> WebhookRequest {
    let payload_bytes = payload.as_bytes();
    let signature = compute_signature(payload_bytes, secret);

    let headers = HashMap::from([
        ("x-github-event".to_string(), event_type.to_string()),
        (
            "x-github-delivery".to_string(),
            "12345678-1234-1234-1234-123456789012".to_string(),
        ),
        ("x-hub-signature-256".to_string(), signature),
        ("content-type".to_string(), "application/json".to_string()),
    ]);

    WebhookRequest::new(headers, Bytes::from(payload_bytes.to_vec()))
}

fn create_minimal_payload() -> String {
    serde_json::json!({
        "action": "opened",
        "repository": {
            "id": 123,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": {
                "login": "owner",
                "id": 1,
                "avatar_url": "https://github.com/avatars/u/1",
                "type": "Organization"
            },
            "description": "Test",
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/owner/test-repo",
            "clone_url": "https://github.com/owner/test-repo.git",
            "ssh_url": "git@github.com:owner/test-repo.git",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }
    })
    .to_string()
}

// ============================================================================
// Test: WebhookRequest Construction
// ============================================================================

/// Verify WebhookRequest extracts headers correctly.
#[test]
fn test_webhook_request_header_extraction() {
    // Arrange
    let headers = HashMap::from([
        ("x-github-event".to_string(), "pull_request".to_string()),
        ("x-github-delivery".to_string(), "12345".to_string()),
        ("x-hub-signature-256".to_string(), "sha256=abc".to_string()),
    ]);
    let body = Bytes::from("test");

    // Act
    let request = WebhookRequest::new(headers, body);

    // Assert
    assert_eq!(request.event_type(), Some("pull_request"));
    assert_eq!(request.delivery_id(), Some("12345"));
    assert_eq!(request.signature(), Some("sha256=abc"));
    assert_eq!(request.payload(), b"test");
}

/// Verify WebhookRequest handles case-insensitive headers.
#[test]
fn test_webhook_request_case_insensitive_headers() {
    // Arrange
    let headers = HashMap::from([
        ("X-GitHub-Event".to_string(), "issues".to_string()),
        ("X-GitHub-Delivery".to_string(), "67890".to_string()),
        ("X-Hub-Signature-256".to_string(), "sha256=def".to_string()),
    ]);
    let body = Bytes::from("test");

    // Act
    let request = WebhookRequest::new(headers, body);

    // Assert
    assert_eq!(request.event_type(), Some("issues"));
    assert_eq!(request.delivery_id(), Some("67890"));
    assert_eq!(request.signature(), Some("sha256=def"));
}

// ============================================================================
// Test: WebhookResponse Status Codes
// ============================================================================

/// Verify WebhookResponse status codes are correct.
#[test]
fn test_webhook_response_status_codes() {
    // OK response
    let ok = WebhookResponse::Ok {
        message: "Success".to_string(),
        event_id: "evt-123".to_string(),
    };
    assert_eq!(ok.status_code(), 200);
    assert!(ok.is_success());

    // Unauthorized response
    let unauthorized = WebhookResponse::Unauthorized {
        message: "Invalid signature".to_string(),
    };
    assert_eq!(unauthorized.status_code(), 401);
    assert!(!unauthorized.is_success());

    // BadRequest response
    let bad_request = WebhookResponse::BadRequest {
        message: "Missing header".to_string(),
    };
    assert_eq!(bad_request.status_code(), 400);
    assert!(!bad_request.is_success());

    // InternalError response
    let internal_error = WebhookResponse::InternalError {
        message: "Processing failed".to_string(),
    };
    assert_eq!(internal_error.status_code(), 500);
    assert!(!internal_error.is_success());
}

// ============================================================================
// Test: Successful Webhook Reception
// ============================================================================

/// Verify receiver processes valid webhook successfully.
///
/// Tests the happy path: valid signature, valid payload, returns OK response.
#[tokio::test]
async fn test_receiver_processes_valid_webhook() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = create_minimal_payload();
    let request = create_webhook_request("pull_request", &payload, secret);

    // Act
    let response = receiver.receive_webhook(request).await;

    // Assert
    assert!(response.is_success(), "Should return success");
    assert_eq!(response.status_code(), 200);
    assert!(response.message().contains("received"));
}

/// Verify receiver returns response quickly (< 100ms target).
///
/// Tests the timing requirement for immediate HTTP response.
#[tokio::test]
async fn test_receiver_returns_response_quickly() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = create_minimal_payload();
    let request = create_webhook_request("pull_request", &payload, secret);

    // Act
    let start = tokio::time::Instant::now();
    let response = receiver.receive_webhook(request).await;
    let duration = start.elapsed();

    // Assert
    assert!(response.is_success(), "Should return success");
    assert!(
        duration < tokio::time::Duration::from_millis(200),
        "Should respond within 200ms, took {:?}",
        duration
    );
}

// ============================================================================
// Test: Signature Validation
// ============================================================================

/// Verify receiver rejects invalid signature.
///
/// Tests that webhooks with incorrect signatures are rejected with 401.
#[tokio::test]
async fn test_receiver_rejects_invalid_signature() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = create_minimal_payload();
    // Create request with wrong secret
    let request = create_webhook_request("pull_request", &payload, "wrong_secret");

    // Act
    let response = receiver.receive_webhook(request).await;

    // Assert
    assert!(!response.is_success(), "Should reject invalid signature");
    assert_eq!(response.status_code(), 401);
    assert!(response.message().contains("Invalid signature"));
}

/// Verify receiver rejects missing signature.
///
/// Tests that webhooks without X-Hub-Signature-256 header are rejected.
#[tokio::test]
async fn test_receiver_rejects_missing_signature() {
    // Arrange
    let secret_provider = Arc::new(MockSecretProvider::new("secret"));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let headers = HashMap::from([
        ("x-github-event".to_string(), "pull_request".to_string()),
        ("x-github-delivery".to_string(), "12345".to_string()),
        // No signature header
    ]);
    let request = WebhookRequest::new(headers, Bytes::from("{}"));

    // Act
    let response = receiver.receive_webhook(request).await;

    // Assert
    assert!(!response.is_success(), "Should reject missing signature");
    assert_eq!(response.status_code(), 401);
    assert!(response.message().contains("Missing"));
}

// ============================================================================
// Test: Header Validation
// ============================================================================

/// Verify receiver rejects missing event type header.
///
/// Tests that webhooks without X-GitHub-Event header are rejected.
#[tokio::test]
async fn test_receiver_rejects_missing_event_type() {
    // Arrange
    let secret_provider = Arc::new(MockSecretProvider::new("secret"));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let headers = HashMap::from([
        // No event type header
        ("x-github-delivery".to_string(), "12345".to_string()),
        ("x-hub-signature-256".to_string(), "sha256=abc".to_string()),
    ]);
    let request = WebhookRequest::new(headers, Bytes::from("{}"));

    // Act
    let response = receiver.receive_webhook(request).await;

    // Assert
    assert!(!response.is_success(), "Should reject missing event type");
    assert_eq!(response.status_code(), 400);
    assert!(response.message().contains("Missing X-GitHub-Event"));
}

// ============================================================================
// Test: Handler Registration and Execution
// ============================================================================

/// Verify handlers are invoked after response is returned.
///
/// Tests the fire-and-forget pattern: response returns immediately,
/// handlers execute asynchronously.
#[tokio::test]
async fn test_receiver_invokes_handlers_async() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let mut receiver = WebhookReceiver::new(secret_provider, processor);

    let handler = Arc::new(MockHandler::with_delay(std::time::Duration::from_millis(
        50,
    )));
    receiver.add_handler(handler.clone()).await;

    let payload = create_minimal_payload();
    let request = create_webhook_request("pull_request", &payload, secret);

    // Act
    let start = tokio::time::Instant::now();
    let response = receiver.receive_webhook(request).await;
    let response_time = start.elapsed();

    // Assert: Response returned quickly (not waiting for handler)
    assert!(response.is_success(), "Should return success");
    assert!(
        response_time < tokio::time::Duration::from_millis(100),
        "Response should not wait for handler, took {:?}",
        response_time
    );

    // Wait for handler to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Assert: Handler was eventually called
    assert_eq!(handler.call_count().await, 1, "Handler should be called");
}

/// Verify multiple handlers are invoked concurrently.
///
/// Tests that all registered handlers receive the event.
#[tokio::test]
async fn test_receiver_invokes_multiple_handlers() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let mut receiver = WebhookReceiver::new(secret_provider, processor);

    let handler1 = Arc::new(MockHandler::new());
    let handler2 = Arc::new(MockHandler::new());
    let handler3 = Arc::new(MockHandler::new());

    receiver.add_handler(handler1.clone()).await;
    receiver.add_handler(handler2.clone()).await;
    receiver.add_handler(handler3.clone()).await;

    let payload = create_minimal_payload();
    let request = create_webhook_request("issues", &payload, secret);

    // Act
    let response = receiver.receive_webhook(request).await;

    // Wait for handlers to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Assert
    assert!(response.is_success());
    assert_eq!(handler1.call_count().await, 1, "Handler 1 should be called");
    assert_eq!(handler2.call_count().await, 1, "Handler 2 should be called");
    assert_eq!(handler3.call_count().await, 1, "Handler 3 should be called");
}

/// Verify handler errors don't affect HTTP response.
///
/// Tests that if handlers fail, the HTTP response is still successful.
#[tokio::test]
async fn test_receiver_handler_errors_dont_affect_response() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let mut receiver = WebhookReceiver::new(secret_provider, processor);

    // Handler that always fails
    #[derive(Clone)]
    struct FailingHandler;

    #[async_trait]
    impl WebhookHandler for FailingHandler {
        async fn handle_event(
            &self,
            _envelope: &EventEnvelope,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Err("Handler failed".into())
        }
    }

    receiver.add_handler(Arc::new(FailingHandler)).await;

    let payload = create_minimal_payload();
    let request = create_webhook_request("pull_request", &payload, secret);

    // Act
    let response = receiver.receive_webhook(request).await;

    // Assert: Response is still successful even though handler will fail
    assert!(response.is_success(), "Response should be successful");
}

// ============================================================================
// Test: Malformed Payload Handling
// ============================================================================

/// Verify receiver rejects malformed JSON payload.
///
/// Tests that invalid JSON results in BadRequest response.
#[tokio::test]
async fn test_receiver_rejects_malformed_json() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = "{invalid json";
    let request = create_webhook_request("pull_request", payload, secret);

    // Act
    let response = receiver.receive_webhook(request).await;

    // Assert
    assert!(!response.is_success(), "Should reject malformed JSON");
    assert_eq!(response.status_code(), 400);
    assert!(response.message().contains("Invalid"));
}

// ============================================================================
// Test: Different Event Types
// ============================================================================

/// Verify receiver processes different event types.
///
/// Tests that the receiver can handle various GitHub event types.
#[tokio::test]
async fn test_receiver_processes_different_event_types() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret));
    let processor = EventProcessor::new(ProcessorConfig::default());
    let receiver = WebhookReceiver::new(secret_provider, processor);

    let payload = create_minimal_payload();

    // Act & Assert
    for event_type in &["pull_request", "issues", "push", "release", "ping"] {
        let request = create_webhook_request(event_type, &payload, secret);
        let response = receiver.receive_webhook(request).await;

        assert!(response.is_success(), "Should process {} event", event_type);
    }
}
