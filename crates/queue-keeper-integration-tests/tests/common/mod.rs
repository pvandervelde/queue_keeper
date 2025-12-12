//! Common test utilities for queue-keeper-api integration tests
//!
//! This module provides:
//! - Mock implementations of traits (WebhookProcessor, HealthChecker, EventStore)
//! - Helper functions for creating test fixtures
//! - Shared test data builders

use axum::http::{HeaderMap, HeaderValue};
use queue_keeper_api::{
    AppState, EventStore, HealthChecker, ServiceConfig, ServiceMetrics, TelemetryConfig,
};
use queue_keeper_core::{
    webhook::{
        EventEntity, EventEnvelope, NormalizationError, StorageError, StorageReference,
        ValidationStatus, WebhookError, WebhookProcessor, WebhookRequest,
    },
    CorrelationId, EventId, QueueKeeperError, Repository, RepositoryId, SessionId, Timestamp, User,
    UserId, UserType, ValidationError,
};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

// ============================================================================
// Mock Webhook Processor
// ============================================================================

/// Mock webhook processor for testing immediate response behavior
#[derive(Clone)]
#[allow(dead_code)]
pub struct MockWebhookProcessor {
    process_calls: Arc<Mutex<Vec<WebhookRequest>>>,
    process_result_factory:
        Arc<Mutex<Box<dyn Fn() -> Result<EventEnvelope, WebhookError> + Send + Sync>>>,
    process_delay: Arc<Mutex<Option<Duration>>>,
}

impl MockWebhookProcessor {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let default_envelope = create_default_event_envelope();

        Self {
            process_calls: Arc::new(Mutex::new(Vec::new())),
            process_result_factory: Arc::new(Mutex::new(Box::new(move || {
                Ok(default_envelope.clone())
            }))),
            process_delay: Arc::new(Mutex::new(None)),
        }
    }

    #[allow(dead_code)]
    pub fn set_result(&self, result: EventEnvelope) {
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
    ) -> Result<EventEnvelope, NormalizationError> {
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
    events: Arc<Mutex<Vec<EventEnvelope>>>,
}

impl MockEventStore {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[allow(dead_code)]
    pub fn add_event(&self, event: EventEnvelope) {
        self.events.lock().unwrap().push(event);
    }

    #[allow(dead_code)]
    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl EventStore for MockEventStore {
    async fn get_event(&self, event_id: &EventId) -> Result<EventEnvelope, QueueKeeperError> {
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
                event_id: e.event_id.clone(),
                event_type: e.event_type.clone(),
                repository: e.repository.full_name.clone(),
                session_id: e.session_id.clone(),
                occurred_at: e.occurred_at.clone(),
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
#[allow(dead_code)]
pub fn create_test_app_state_with_processor(processor: Arc<dyn WebhookProcessor>) -> AppState {
    let config = ServiceConfig::default();
    let health_checker = Arc::new(MockHealthChecker::new());
    let event_store = Arc::new(MockEventStore::new());
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

/// Create a default EventEnvelope for testing
#[allow(dead_code)]
pub fn create_default_event_envelope() -> EventEnvelope {
    EventEnvelope {
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
    }
}
