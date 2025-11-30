//! Tests for the queue-keeper-service library module.

use super::*;
use axum::http::{HeaderMap, HeaderValue};
use axum_test::TestServer;
use bytes::Bytes;
use queue_keeper_core::{
    webhook::{
        EventEntity, EventEnvelope, ValidationStatus, WebhookError, WebhookProcessor,
        WebhookRequest,
    },
    CorrelationId, EventId, Repository, RepositoryId, SessionId, Timestamp, User, UserId, UserType,
    ValidationError,
};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_health_endpoint() {
    let config = ServiceConfig::default();
    let webhook_processor = Arc::new(queue_keeper_core::webhook::DefaultWebhookProcessor::new(
        None, None,
    ));
    let health_checker = Arc::new(DefaultHealthChecker);
    let event_store = Arc::new(DefaultEventStore);
    let metrics = ServiceMetrics::new().expect("Failed to create metrics");
    let telemetry_config = Arc::new(TelemetryConfig::default());

    let state = AppState::new(
        config,
        webhook_processor,
        health_checker,
        event_store,
        metrics,
        telemetry_config,
    );
    let app = create_router(state);

    let _server = TestServer::new(app).unwrap();

    // TODO: Fix test once health checker is implemented
    // let response = server.get("/health").await;
    // assert_eq!(response.status_code(), 200);
}

#[test]
fn test_config_defaults() {
    let config = ServiceConfig::default();
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.webhooks.endpoint_path, "/webhook");
    assert!(config.webhooks.require_signature);
}

// ============================================================================
// Mock Implementations for Testing
// ============================================================================

/// Mock webhook processor for testing immediate response behavior
#[derive(Clone)]
struct MockWebhookProcessor {
    process_calls: Arc<Mutex<Vec<WebhookRequest>>>,
    process_result_factory:
        Arc<Mutex<Box<dyn Fn() -> Result<EventEnvelope, WebhookError> + Send + Sync>>>,
    process_delay: Arc<Mutex<Option<Duration>>>,
}

impl MockWebhookProcessor {
    fn new() -> Self {
        let default_envelope = EventEnvelope {
            event_id: EventId::new(),
            event_type: "pull_request".to_string(),
            action: Some("opened".to_string()),
            repository: Repository::new(
                RepositoryId::new(1),
                "repo".to_string(),
                "owner/repo".to_string(),
                User {
                    id: UserId::new(1),
                    login: "owner".to_string(),
                    user_type: UserType::User,
                },
                false,
            ),
            entity: EventEntity::PullRequest { number: 123 },
            session_id: SessionId::from_parts("owner", "repo", "pull_request", "123"),
            correlation_id: CorrelationId::new(),
            occurred_at: Timestamp::now(),
            processed_at: Timestamp::now(),
            payload: serde_json::json!({"test": "data"}),
        };

        Self {
            process_calls: Arc::new(Mutex::new(Vec::new())),
            process_result_factory: Arc::new(Mutex::new(Box::new(move || {
                Ok(default_envelope.clone())
            }))),
            process_delay: Arc::new(Mutex::new(None)),
        }
    }

    fn set_result(&self, result: EventEnvelope) {
        let r = result.clone();
        *self.process_result_factory.lock().unwrap() = Box::new(move || Ok(r.clone()));
    }

    fn set_error(&self, error_msg: String) {
        *self.process_result_factory.lock().unwrap() =
            Box::new(move || Err(WebhookError::InvalidSignature(error_msg.clone())));
    }

    fn set_delay(&self, delay: Duration) {
        *self.process_delay.lock().unwrap() = Some(delay);
    }

    fn get_calls(&self) -> Vec<WebhookRequest> {
        self.process_calls.lock().unwrap().clone()
    }

    fn call_count(&self) -> usize {
        self.process_calls.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl WebhookProcessor for MockWebhookProcessor {
    async fn process_webhook(
        &self,
        request: WebhookRequest,
    ) -> Result<EventEnvelope, WebhookError> {
        // Record the call
        self.process_calls.lock().unwrap().push(request.clone());

        // Simulate processing delay if configured
        let delay = *self.process_delay.lock().unwrap();
        if let Some(delay) = delay {
            sleep(delay).await;
        }

        // Return configured result by calling factory
        (self.process_result_factory.lock().unwrap())()
    }

    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _event_type: &str,
    ) -> Result<(), ValidationError> {
        Ok(())
    }

    async fn store_raw_payload(
        &self,
        _request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<
        queue_keeper_core::webhook::StorageReference,
        queue_keeper_core::webhook::StorageError,
    > {
        Ok(queue_keeper_core::webhook::StorageReference {
            blob_path: "test/path".to_string(),
            stored_at: Timestamp::now(),
            size_bytes: 100,
        })
    }

    async fn normalize_event(
        &self,
        _request: &WebhookRequest,
    ) -> Result<EventEnvelope, queue_keeper_core::webhook::NormalizationError> {
        (self.process_result_factory.lock().unwrap())().map_err(|e| {
            queue_keeper_core::webhook::NormalizationError::MissingRequiredField {
                field: e.to_string(),
            }
        })
    }
}

// ============================================================================
// Tests for handle_webhook Immediate Response Pattern
// ============================================================================

/// Verify that handle_webhook returns HTTP 200 OK immediately after webhook processing
///
/// This test validates Assertion #2: Response Time SLA
/// - Webhook processing (validation + normalization + storage) completes within fast path
/// - HTTP response returned immediately (target <500ms)
/// - Queue delivery happens asynchronously in background (tested separately)
#[tokio::test]
async fn test_handle_webhook_returns_immediately_after_processing() {
    // Arrange: Create mock processor that simulates fast processing (50ms)
    let processor = MockWebhookProcessor::new();
    processor.set_delay(Duration::from_millis(50));

    let state = create_test_app_state_with_processor(Arc::new(processor.clone()));

    let headers = create_valid_webhook_headers();
    let body = Bytes::from(r#"{"action":"opened","number":123}"#);

    // Act: Measure response time
    let start = std::time::Instant::now();
    let result = handle_webhook(axum::extract::State(state), headers, body).await;
    let response_time = start.elapsed();

    // Assert: Response returned quickly (within 1 second, ideally <500ms)
    assert!(result.is_ok(), "Expected successful response");
    assert!(
        response_time < Duration::from_millis(1000),
        "Response took {}ms, expected <1000ms for fast path",
        response_time.as_millis()
    );

    // Assert: Webhook processor was called during fast path
    assert_eq!(
        processor.call_count(),
        1,
        "Expected one processor call during fast path"
    );
}

/// Verify that handle_webhook returns error if webhook processing fails
///
/// This test validates that validation/normalization errors are returned immediately
/// without spawning background task for queue delivery.
#[tokio::test]
async fn test_handle_webhook_returns_error_on_validation_failure() {
    // Arrange: Configure processor to return validation error
    let processor = MockWebhookProcessor::new();
    processor.set_error("Invalid signature".to_string());

    let state = create_test_app_state_with_processor(Arc::new(processor));

    let headers = create_valid_webhook_headers();
    let body = Bytes::from(r#"{"action":"opened"}"#);

    // Act
    let result = handle_webhook(axum::extract::State(state), headers, body).await;

    // Assert: Error response returned immediately
    assert!(result.is_err(), "Expected error response");
    match result {
        Err(WebhookHandlerError::ProcessingFailed(WebhookError::InvalidSignature(_))) => {
            // Expected error type
        }
        other => panic!("Expected InvalidSignature error, got: {:?}", other),
    }
}

/// Verify that handle_webhook response includes event_id and session_id
///
/// This test validates that the immediate response contains tracking identifiers
/// for correlation and monitoring.
#[tokio::test]
async fn test_handle_webhook_response_includes_event_metadata() {
    // Arrange
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor));

    let headers = create_valid_webhook_headers();
    let body = Bytes::from(r#"{"action":"opened","number":123}"#);

    // Act
    let result = handle_webhook(axum::extract::State(state), headers, body).await;

    // Assert
    assert!(result.is_ok(), "Expected successful response");
    let response = result.unwrap().0;

    assert!(
        !response.event_id.to_string().is_empty(),
        "Event ID should not be empty"
    );
    assert!(
        !response.session_id.to_string().is_empty(),
        "Session ID should not be empty"
    );
    assert_eq!(response.status, "processed", "Status should be 'processed'");
    assert!(
        response.message.contains("successfully"),
        "Message should indicate success"
    );
}

/// Verify that malformed headers result in immediate error response
///
/// This test validates input validation at the HTTP layer.
#[tokio::test]
async fn test_handle_webhook_rejects_malformed_headers() {
    // Arrange
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor));

    // Create headers missing required GitHub webhook headers
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    // Missing X-GitHub-Event, X-GitHub-Delivery

    let body = Bytes::from(r#"{"action":"opened"}"#);

    // Act
    let result = handle_webhook(axum::extract::State(state), headers, body).await;

    // Assert: Validation error returned immediately
    assert!(result.is_err(), "Expected error for malformed headers");
    match result {
        Err(WebhookHandlerError::InvalidHeaders(_)) => {
            // Expected error type
        }
        other => panic!("Expected InvalidHeaders error, got: {:?}", other),
    }
}

/// Verify that handle_webhook processes ping events immediately without queue delivery
///
/// This test validates that ping events return immediately without async processing.
#[tokio::test]
async fn test_handle_webhook_handles_ping_event_immediately() {
    // Arrange
    let processor = MockWebhookProcessor::new();
    let state = create_test_app_state_with_processor(Arc::new(processor.clone()));

    let mut headers = HeaderMap::new();
    headers.insert("x-github-event", HeaderValue::from_static("ping"));
    headers.insert(
        "x-github-delivery",
        HeaderValue::from_static("12345678-1234-1234-1234-123456789012"),
    );
    headers.insert("content-type", HeaderValue::from_static("application/json"));

    let body = Bytes::from(r#"{"zen":"Testing is good","hook_id":123}"#);

    // Act
    let start = std::time::Instant::now();
    let result = handle_webhook(axum::extract::State(state), headers, body).await;
    let response_time = start.elapsed();

    // Assert: Response returned very quickly for ping event
    assert!(result.is_ok(), "Expected successful ping response");
    assert!(
        response_time < Duration::from_millis(200),
        "Ping response took {}ms, expected <200ms",
        response_time.as_millis()
    );

    // Ping events may or may not call processor depending on implementation
    // Main assertion is that it returns quickly
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_app_state_with_processor(processor: Arc<dyn WebhookProcessor>) -> AppState {
    let config = ServiceConfig::default();
    let health_checker = Arc::new(DefaultHealthChecker);
    let event_store = Arc::new(DefaultEventStore);

    // Use Default which creates stub metrics - tests don't need real prometheus metrics
    let metrics = Arc::new(ServiceMetrics::default());
    let telemetry_config = Arc::new(TelemetryConfig::default());

    AppState::new(
        config,
        processor,
        health_checker,
        event_store,
        metrics,
        telemetry_config,
    )
}

fn create_valid_webhook_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-github-event", HeaderValue::from_static("pull_request"));
    headers.insert(
        "x-github-delivery",
        HeaderValue::from_static("12345678-1234-1234-1234-123456789012"),
    );
    headers.insert(
        "x-hub-signature-256",
        HeaderValue::from_static("sha256=abc123"),
    );
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    headers
}
