//! Common test utilities for queue-keeper-api integration tests
//!
//! This module provides:
//! - Mock implementations of traits (WebhookProcessor, HealthChecker, EventStore)
//! - Helper functions for creating test fixtures
//! - Shared test data builders

use axum::http::{HeaderMap, HeaderValue};
use queue_keeper_api::{
    AppState, EventStore, HealthChecker, ProviderId, ProviderRegistry, ServiceConfig,
    ServiceMetrics, TelemetryConfig,
};
use queue_keeper_core::{
    audit_logging::{
        AuditAction, AuditActor, AuditContext, AuditError, AuditEvent, AuditLogId, AuditLogger,
        AuditResource, AuditResult, SecurityAuditEvent, WebhookProcessingAction,
    },
    webhook::{
        NormalizationError, ProcessingOutput, StorageError, StorageReference, ValidationStatus,
        WebhookError, WebhookProcessor, WebhookRequest, WrappedEvent,
    },
    EventId, QueueKeeperError, Repository, SessionId, Timestamp, ValidationError,
};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

// ============================================================================
// Mock Webhook Processor
// ============================================================================

/// Mock webhook processor for testing immediate response behavior
#[derive(Clone)]
#[allow(dead_code)]
#[allow(clippy::type_complexity)]
pub struct MockWebhookProcessor {
    process_calls: Arc<Mutex<Vec<WebhookRequest>>>,
    process_result_factory:
        Arc<Mutex<Box<dyn Fn() -> Result<WrappedEvent, WebhookError> + Send + Sync>>>,
    process_delay: Arc<Mutex<Option<Duration>>>,
}

impl MockWebhookProcessor {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let default_event = create_default_wrapped_event();

        Self {
            process_calls: Arc::new(Mutex::new(Vec::new())),
            process_result_factory: Arc::new(Mutex::new(Box::new(move || {
                Ok(default_event.clone())
            }))),
            process_delay: Arc::new(Mutex::new(None)),
        }
    }

    #[allow(dead_code)]
    pub fn set_result(&self, result: WrappedEvent) {
        let r = result.clone();
        *self.process_result_factory.lock().unwrap() = Box::new(move || Ok(r.clone()));
    }

    #[allow(dead_code)]
    pub fn set_error(&self, error_msg: String) {
        *self.process_result_factory.lock().unwrap() =
            Box::new(move || Err(WebhookError::InvalidSignature(error_msg.clone())));
    }

    #[allow(dead_code)]
    pub fn set_delay(&self, delay: Duration) {
        *self.process_delay.lock().unwrap() = Some(delay);
    }

    #[allow(dead_code)]
    pub fn get_calls(&self) -> Vec<WebhookRequest> {
        self.process_calls.lock().unwrap().clone()
    }

    #[allow(dead_code)]
    pub fn call_count(&self) -> usize {
        self.process_calls.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl WebhookProcessor for MockWebhookProcessor {
    async fn process_webhook(
        &self,
        request: WebhookRequest,
    ) -> Result<ProcessingOutput, WebhookError> {
        // Record the call
        self.process_calls.lock().unwrap().push(request.clone());

        // Simulate processing delay if configured
        let delay = *self.process_delay.lock().unwrap();
        if let Some(delay) = delay {
            sleep(delay).await;
        }

        // Return configured result by calling factory
        let event = (self.process_result_factory.lock().unwrap())()?;
        Ok(ProcessingOutput::Wrapped(event))
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
            blob_path: "test/path".to_string(),
            stored_at: Timestamp::now(),
            size_bytes: 100,
        })
    }

    async fn normalize_event(
        &self,
        _request: &WebhookRequest,
    ) -> Result<WrappedEvent, NormalizationError> {
        (self.process_result_factory.lock().unwrap())().map_err(|e| {
            NormalizationError::MissingRequiredField {
                field: e.to_string(),
            }
        })
    }
}

// ============================================================================
// Mock Health Checker
// ============================================================================

#[derive(Clone)]
#[allow(dead_code)]
pub struct MockHealthChecker {
    healthy: Arc<Mutex<bool>>,
}

impl MockHealthChecker {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            healthy: Arc::new(Mutex::new(true)),
        }
    }

    #[allow(dead_code)]
    pub fn set_healthy(&self, healthy: bool) {
        *self.healthy.lock().unwrap() = healthy;
    }
}

#[async_trait::async_trait]
impl HealthChecker for MockHealthChecker {
    async fn check_basic_health(&self) -> queue_keeper_api::HealthStatus {
        let healthy = *self.healthy.lock().unwrap();
        let mut checks = std::collections::HashMap::new();
        checks.insert(
            "service".to_string(),
            queue_keeper_api::HealthCheckResult {
                healthy,
                duration_ms: 0,
                message: "Mock health check".to_string(),
            },
        );
        queue_keeper_api::HealthStatus {
            is_healthy: healthy,
            checks,
        }
    }

    async fn check_deep_health(&self) -> queue_keeper_api::HealthStatus {
        self.check_basic_health().await
    }

    async fn check_readiness(&self) -> bool {
        *self.healthy.lock().unwrap()
    }
}

// ============================================================================
// Mock Event Store
// ============================================================================

#[derive(Clone)]
#[allow(dead_code)]
pub struct MockEventStore {
    events: Arc<Mutex<Vec<WrappedEvent>>>,
}

impl MockEventStore {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[allow(dead_code)]
    pub fn add_event(&self, event: WrappedEvent) {
        self.events.lock().unwrap().push(event);
    }

    #[allow(dead_code)]
    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl EventStore for MockEventStore {
    async fn get_event(&self, event_id: &EventId) -> Result<WrappedEvent, QueueKeeperError> {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .find(|e| &e.event_id == event_id)
            .cloned()
            .ok_or(QueueKeeperError::Internal {
                message: format!("Event not found: {}", event_id),
            })
    }

    async fn list_events(
        &self,
        params: queue_keeper_api::EventListParams,
    ) -> Result<queue_keeper_api::EventListResponse, QueueKeeperError> {
        let events = self.events.lock().unwrap();
        let per_page = params.per_page.unwrap_or(100);
        let page = params.page.unwrap_or(1);
        let offset = (page - 1) * per_page;

        let items: Vec<queue_keeper_api::EventSummary> = events
            .iter()
            .skip(offset)
            .take(per_page)
            .map(|e| queue_keeper_api::EventSummary {
                event_id: e.event_id,
                event_type: e.event_type.clone(),
                repository: e
                    .payload
                    .get("repository")
                    .and_then(|r| r.get("full_name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                session_id: e
                    .session_id
                    .clone()
                    .unwrap_or_else(|| SessionId::from_parts("none", "none", "none", "0")),
                occurred_at: e.received_at,
                status: "processed".to_string(),
            })
            .collect();

        Ok(queue_keeper_api::EventListResponse {
            events: items,
            total: events.len(),
            page,
            per_page,
        })
    }

    async fn list_sessions(
        &self,
        _params: queue_keeper_api::SessionListParams,
    ) -> Result<queue_keeper_api::SessionListResponse, QueueKeeperError> {
        Ok(queue_keeper_api::SessionListResponse {
            sessions: vec![],
            total: 0,
        })
    }

    async fn get_session(
        &self,
        _session_id: &SessionId,
    ) -> Result<queue_keeper_api::SessionDetails, QueueKeeperError> {
        Err(QueueKeeperError::Validation(ValidationError::Required {
            field: "session_id".to_string(),
        }))
    }

    async fn get_statistics(
        &self,
    ) -> Result<queue_keeper_api::StatisticsResponse, QueueKeeperError> {
        Ok(queue_keeper_api::StatisticsResponse {
            total_events: self.events.lock().unwrap().len() as u64,
            events_per_hour: 0.0,
            active_sessions: 0,
            error_rate: 0.0,
            uptime_seconds: 0,
        })
    }
}

// ============================================================================
// Test Fixture Builders
// ============================================================================

/// Create a test AppState with mock implementations
#[allow(dead_code)]
pub fn create_test_app_state() -> AppState {
    create_test_app_state_with_processor(Arc::new(MockWebhookProcessor::new()))
}

/// Create a test AppState with a specific webhook processor
///
/// The processor is registered in a [`ProviderRegistry`] under the canonical
/// `"github"` provider ID so that the `/webhook/github` route resolves it.
#[allow(dead_code)]
pub fn create_test_app_state_with_processor(processor: Arc<dyn WebhookProcessor>) -> AppState {
    create_test_app_state_with_providers(vec![("github".to_string(), processor)])
}

/// Create a test AppState with multiple named webhook processors.
///
/// Each entry in `providers` is `(provider_id, processor)`.  The registry will have
/// exactly those providers registered; no default "github" entry is added unless
/// explicitly included.
#[allow(dead_code)]
pub fn create_test_app_state_with_providers(
    providers: Vec<(String, Arc<dyn WebhookProcessor>)>,
) -> AppState {
    let config = ServiceConfig::default();
    let health_checker = Arc::new(MockHealthChecker::new());
    let event_store = Arc::new(MockEventStore::new());
    let metrics = Arc::new(ServiceMetrics::default());
    let telemetry_config = Arc::new(TelemetryConfig::default());

    let mut registry = ProviderRegistry::new();
    for (id, processor) in providers {
        registry.register(ProviderId::new(&id).unwrap(), processor);
    }

    AppState::new(
        config,
        Arc::new(registry),
        health_checker,
        event_store,
        metrics,
        telemetry_config,
        std::collections::HashSet::new(),
    )
}

/// Create valid GitHub webhook headers for testing
#[allow(dead_code)]
pub fn create_valid_webhook_headers() -> HeaderMap {
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

/// Create a default WrappedEvent for testing
#[allow(dead_code)]
pub fn create_default_wrapped_event() -> WrappedEvent {
    WrappedEvent::new(
        "github".to_string(),
        "pull_request".to_string(),
        Some("opened".to_string()),
        Some(SessionId::from_parts(
            "owner",
            "repo",
            "pull_request",
            "123",
        )),
        serde_json::json!({
            "test": "data",
            "repository": {
                "name": "repo",
                "full_name": "owner/repo",
                "owner": {"login": "owner"}
            }
        }),
    )
}

// ============================================================================
// Mock Audit Logger
// ============================================================================

/// Mock audit logger for testing audit event recording
#[derive(Clone)]
#[allow(dead_code)]
pub struct MockAuditLogger {
    logged_events: Arc<Mutex<Vec<AuditEvent>>>,
}

#[allow(dead_code)]
impl MockAuditLogger {
    pub fn new() -> Self {
        Self {
            logged_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all logged audit events
    pub fn get_logged_events(&self) -> Vec<AuditEvent> {
        self.logged_events.lock().unwrap().clone()
    }

    /// Get count of logged events
    pub fn event_count(&self) -> usize {
        self.logged_events.lock().unwrap().len()
    }

    /// Clear all logged events
    pub fn clear(&self) {
        self.logged_events.lock().unwrap().clear();
    }
}

#[async_trait::async_trait]
impl AuditLogger for MockAuditLogger {
    async fn log_event(&self, event: AuditEvent) -> Result<AuditLogId, AuditError> {
        self.logged_events.lock().unwrap().push(event.clone());
        Ok(event.audit_id)
    }

    async fn log_webhook_processing(
        &self,
        _event_id: EventId,
        _session_id: SessionId,
        _repository: Repository,
        _action: WebhookProcessingAction,
        _result: AuditResult,
        _context: AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        // Create a dummy event for testing
        let audit_id = AuditLogId::new();
        Ok(audit_id)
    }

    async fn log_admin_action(
        &self,
        _actor: AuditActor,
        _resource: AuditResource,
        _action: AuditAction,
        _result: AuditResult,
        _context: AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        let audit_id = AuditLogId::new();
        Ok(audit_id)
    }

    async fn log_security_event(
        &self,
        _security_event: SecurityAuditEvent,
        _context: AuditContext,
    ) -> Result<AuditLogId, AuditError> {
        let audit_id = AuditLogId::new();
        Ok(audit_id)
    }

    async fn log_events_batch(
        &self,
        events: Vec<AuditEvent>,
    ) -> Result<Vec<AuditLogId>, AuditError> {
        let mut ids = Vec::new();
        for event in events {
            ids.push(self.log_event(event).await?);
        }
        Ok(ids)
    }

    async fn flush(&self) -> Result<(), AuditError> {
        Ok(())
    }
}
