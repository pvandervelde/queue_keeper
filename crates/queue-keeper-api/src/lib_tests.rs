//! Tests for provider-specific webhook routing in the HTTP layer.

use super::*;
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use provider_registry::{ProviderId, ProviderRegistry};
use queue_keeper_core::{
    webhook::{
        NormalizationError, ProcessingOutput, StorageError, StorageReference,
        ValidationStatus, WebhookError, WebhookProcessor, WebhookRequest, WrappedEvent,
    },
    Timestamp, ValidationError,
};
use std::sync::{Arc, Mutex, OnceLock};
use tower::ServiceExt;

// ============================================================================
// Mock WebhookProcessor
// ============================================================================

/// Test double that records whether `process_webhook` was called and returns
/// a preset [`ProcessingOutput`].
struct MockWebhookProcessor {
    called: Arc<Mutex<bool>>,
}

impl MockWebhookProcessor {
    fn new() -> Self {
        Self {
            called: Arc::new(Mutex::new(false)),
        }
    }

    /// Returns `true` if `process_webhook` was called at least once.
    fn was_called(&self) -> bool {
        *self.called.lock().unwrap()
    }
}

#[async_trait]
impl WebhookProcessor for MockWebhookProcessor {
    async fn process_webhook(
        &self,
        _request: WebhookRequest,
    ) -> Result<ProcessingOutput, WebhookError> {
        *self.called.lock().unwrap() = true;
        Ok(ProcessingOutput::Wrapped(test_wrapped_event()))
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
    ) -> Result<StorageReference, StorageError> {
        Ok(StorageReference {
            blob_path: "test/path.json".to_string(),
            stored_at: Timestamp::now(),
            size_bytes: 0,
        })
    }

    async fn normalize_event(
        &self,
        _request: &WebhookRequest,
    ) -> Result<WrappedEvent, NormalizationError> {
        Ok(test_wrapped_event())
    }
}

// ============================================================================
// Test helpers
// ============================================================================

/// Build a minimal [`WrappedEvent`] suitable for mock responses.
fn test_wrapped_event() -> WrappedEvent {
    WrappedEvent::new(
        "github".to_string(),
        "ping".to_string(),
        None,
        None,
        serde_json::json!({}),
    )
}

/// Returns a shared [`ServiceMetrics`] instance.
///
/// Prometheus registers metrics with a global registry that rejects duplicate
/// registrations.  Using [`OnceLock`] ensures the instance (and therefore the
/// registrations) is created exactly once per test-binary invocation, regardless
/// of how many tests call this helper.
static TEST_METRICS: OnceLock<Arc<ServiceMetrics>> = OnceLock::new();

fn test_metrics() -> Arc<ServiceMetrics> {
    TEST_METRICS
        .get_or_init(|| ServiceMetrics::new().expect("ServiceMetrics::new must succeed in tests"))
        .clone()
}

/// Build an [`AppState`] with the given registry and default stubs for all
/// other dependencies.
fn test_app_state(registry: ProviderRegistry) -> AppState {
    AppState::new(
        ServiceConfig::default(),
        Arc::new(registry),
        Arc::new(DefaultHealthChecker),
        Arc::new(DefaultEventStore),
        test_metrics(),
        Arc::new(TelemetryConfig::new(
            "test-service".to_string(),
            "test".to_string(),
        )),
    )
}

/// Build a POST request with the minimal GitHub-style headers for a `ping` event.
///
/// A `ping` event requires no signature (`X-Hub-Signature-256`), making it
/// the simplest valid payload for testing the routing layer.
fn ping_request(path: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("x-github-event", "ping")
        .header("x-github-delivery", "12345678-1234-1234-1234-123456789abc")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap()
}

// ============================================================================
// Provider routing tests
// ============================================================================

/// Verify that POST /webhook/{provider} calls the registered processor and
/// returns 200 OK.
#[tokio::test]
async fn test_known_provider_routes_to_processor() {
    let mock = Arc::new(MockWebhookProcessor::new());
    let mut registry = ProviderRegistry::new();
    registry.register(ProviderId::new("github").unwrap(), mock.clone());

    let app = create_router(test_app_state(registry));

    let response = app.oneshot(ping_request("/webhook/github")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        mock.was_called(),
        "GitHub processor should have been called"
    );
}

/// Verify that POST /webhook/{unknown} returns 404 when the provider is not
/// registered.
#[tokio::test]
async fn test_unknown_provider_returns_404() {
    // Registry with no providers registered
    let registry = ProviderRegistry::new();

    let app = create_router(test_app_state(registry));

    let response = app
        .oneshot(ping_request("/webhook/unknown-provider"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Verify that GET /webhook/{provider} returns 405 Method Not Allowed since
/// only POST is supported.
#[tokio::test]
async fn test_get_method_not_allowed() {
    let mock = Arc::new(MockWebhookProcessor::new());
    let mut registry = ProviderRegistry::new();
    registry.register(ProviderId::new("github").unwrap(), mock.clone());

    let app = create_router(test_app_state(registry));

    let request = Request::builder()
        .method("GET")
        .uri("/webhook/github")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

/// Verify that multiple providers registered in the same registry route their
/// requests to independent processors.
#[tokio::test]
async fn test_different_providers_route_independently() {
    let github_mock = Arc::new(MockWebhookProcessor::new());
    let jira_mock = Arc::new(MockWebhookProcessor::new());

    let mut registry = ProviderRegistry::new();
    registry.register(ProviderId::new("github").unwrap(), github_mock.clone());
    registry.register(ProviderId::new("jira").unwrap(), jira_mock.clone());

    let app = create_router(test_app_state(registry));

    // Call only /webhook/github
    let response = app
        .clone()
        .oneshot(ping_request("/webhook/github"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        github_mock.was_called(),
        "GitHub processor should be called"
    );
    assert!(
        !jira_mock.was_called(),
        "Jira processor should NOT be called when routing to github"
    );
}

/// Verify that the 404 response for an unknown provider includes a descriptive
/// error body.
#[tokio::test]
async fn test_unknown_provider_404_has_error_body() {
    let registry = ProviderRegistry::new();
    let app = create_router(test_app_state(registry));

    let response = app
        .oneshot(ping_request("/webhook/nonexistent"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(
        body["error"].as_str().unwrap_or("").contains("nonexistent"),
        "Error message should mention the unknown provider name"
    );
}
