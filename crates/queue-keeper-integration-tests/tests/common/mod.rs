//! Common test utilities for queue-keeper-api integration tests
//!
//! This module provides:
//! - Mock implementations of traits (WebhookProcessor, HealthChecker, EventStore)
//! - Mock QueueClient and BlobStorage for queue delivery tests
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
    blob_storage::{
        BlobMetadata, BlobStorage, BlobStorageError, PayloadFilter, StorageHealthStatus,
        StorageMetrics, StoredWebhook, WebhookPayload,
    },
    bot_config::{BotConfiguration, BotConfigurationSettings, BotSpecificConfig, BotSubscription},
    webhook::{
        NormalizationError, ProcessingOutput, StorageError, StorageReference, ValidationStatus,
        WebhookError, WebhookProcessor, WebhookRequest, WrappedEvent,
    },
    BotName, EventId, QueueKeeperError, QueueName, Repository, SessionId, Timestamp,
    ValidationError,
};
use queue_runtime::{
    Message, MessageId, ProviderType, QueueClient, QueueError, QueueName as RuntimeQueueName,
    ReceiptHandle, ReceivedMessage, SessionId as RuntimeSessionId,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

// ============================================================================
// Mock Queue Client
// ============================================================================

/// Controllable mock `QueueClient` for delivery integration tests.
///
/// Supports pre-queuing explicit send results (success or failure); once
/// the queue is exhausted every subsequent `send_message` returns `Ok`.
/// Dead-letter and abandon calls are recorded for assertion.
#[derive(Clone)]
#[allow(dead_code)]
pub struct MockQueueClient {
    send_results: Arc<Mutex<VecDeque<Result<MessageId, QueueError>>>>,
    sent_messages: Arc<Mutex<Vec<(RuntimeQueueName, Message)>>>,
    dead_lettered: Arc<Mutex<Vec<(ReceiptHandle, String)>>>,
    abandoned: Arc<Mutex<Vec<ReceiptHandle>>>,
}

#[allow(dead_code)]
impl MockQueueClient {
    /// Create a new mock that succeeds by default.
    pub fn new() -> Self {
        Self {
            send_results: Arc::new(Mutex::new(VecDeque::new())),
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            dead_lettered: Arc::new(Mutex::new(Vec::new())),
            abandoned: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Queue an explicit success result for the next `send_message` call.
    pub fn expect_success(&self) -> MessageId {
        let id = MessageId::new();
        self.send_results
            .lock()
            .unwrap()
            .push_back(Ok(id.clone()));
        id
    }

    /// Queue an explicit transient failure for the next `send_message` call.
    pub fn expect_transient_failure(&self) {
        self.send_results.lock().unwrap().push_back(Err(
            QueueError::ProviderError {
                provider: "MockQueue".to_string(),
                code: "TransientError".to_string(),
                message: "simulated transient failure".to_string(),
            },
        ));
    }

    /// Queue an explicit permanent failure for the next `send_message` call.
    pub fn expect_permanent_failure(&self) {
        self.send_results
            .lock()
            .unwrap()
            .push_back(Err(QueueError::MessageTooLarge {
                size: usize::MAX,
                max_size: 1,
            }));
    }

    /// Pre-queue enough failures so every send will fail (for DLQ tests).
    pub fn always_fail_transient(&self, count: usize) {
        let mut q = self.send_results.lock().unwrap();
        for _ in 0..count {
            q.push_back(Err(QueueError::ProviderError {
                provider: "MockQueue".to_string(),
                code: "TransientError".to_string(),
                message: "simulated transient failure".to_string(),
            }));
        }
    }

    /// Number of `send_message` calls recorded so far.
    pub fn send_count(&self) -> usize {
        self.sent_messages.lock().unwrap().len()
    }

    /// Get all sent (queue_name, message) pairs recorded so far.
    pub fn sent_messages(&self) -> Vec<(RuntimeQueueName, Message)> {
        self.sent_messages.lock().unwrap().clone()
    }

    /// Number of `dead_letter_message` calls recorded so far.
    pub fn dead_letter_count(&self) -> usize {
        self.dead_lettered.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl QueueClient for MockQueueClient {
    async fn send_message(
        &self,
        queue: &RuntimeQueueName,
        message: Message,
    ) -> Result<MessageId, QueueError> {
        self.sent_messages
            .lock()
            .unwrap()
            .push((queue.clone(), message));

        let result = self.send_results.lock().unwrap().pop_front();
        match result {
            Some(r) => r,
            None => Ok(MessageId::new()),
        }
    }

    async fn send_messages(
        &self,
        queue: &RuntimeQueueName,
        messages: Vec<Message>,
    ) -> Result<Vec<MessageId>, QueueError> {
        let mut ids = Vec::new();
        for msg in messages {
            ids.push(self.send_message(queue, msg).await?);
        }
        Ok(ids)
    }

    async fn receive_message(
        &self,
        _queue: &RuntimeQueueName,
        _timeout: chrono::Duration,
    ) -> Result<Option<ReceivedMessage>, QueueError> {
        Ok(None)
    }

    async fn receive_messages(
        &self,
        _queue: &RuntimeQueueName,
        _max_messages: u32,
        _timeout: chrono::Duration,
    ) -> Result<Vec<ReceivedMessage>, QueueError> {
        Ok(vec![])
    }

    async fn complete_message(&self, _receipt: ReceiptHandle) -> Result<(), QueueError> {
        Ok(())
    }

    async fn abandon_message(&self, receipt: ReceiptHandle) -> Result<(), QueueError> {
        self.abandoned.lock().unwrap().push(receipt);
        Ok(())
    }

    async fn dead_letter_message(
        &self,
        receipt: ReceiptHandle,
        reason: String,
    ) -> Result<(), QueueError> {
        self.dead_lettered.lock().unwrap().push((receipt, reason));
        Ok(())
    }

    async fn accept_session(
        &self,
        _queue: &RuntimeQueueName,
        _session_id: Option<RuntimeSessionId>,
    ) -> Result<Box<dyn queue_runtime::SessionClient>, QueueError> {
        Err(QueueError::ProviderError {
            provider: "MockQueue".to_string(),
            code: "NotSupported".to_string(),
            message: "sessions not supported in MockQueueClient".to_string(),
        })
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::InMemory
    }

    fn supports_sessions(&self) -> bool {
        false
    }

    fn supports_batching(&self) -> bool {
        false
    }
}

// ============================================================================
// Mock Blob Storage
// ============================================================================

/// In-memory `BlobStorage` implementation for DLQ persistence tests.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct MockBlobStorage {
    stored: Arc<Mutex<Vec<(EventId, WebhookPayload)>>>,
}

#[allow(dead_code)]
impl MockBlobStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stored_count(&self) -> usize {
        self.stored.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl BlobStorage for MockBlobStorage {
    async fn store_payload(
        &self,
        event_id: &EventId,
        payload: &WebhookPayload,
    ) -> Result<BlobMetadata, BlobStorageError> {
        self.stored.lock().unwrap().push((*event_id, payload.clone()));
        Ok(BlobMetadata {
            blob_path: format!("mock/{}.json", event_id),
            event_id: *event_id,
            size_bytes: payload.body.len() as u64,
            created_at: Timestamp::now(),
            content_type: "application/json".to_string(),
            checksum_sha256: "mock-checksum-sha256".to_string(),
            metadata: queue_keeper_core::blob_storage::PayloadMetadata {
                event_id: *event_id,
                event_type: payload.metadata.event_type.clone(),
                repository: None,
                signature_valid: true,
                received_at: Timestamp::now(),
                delivery_id: None,
            },
        })
    }

    async fn get_payload(
        &self,
        event_id: &EventId,
    ) -> Result<Option<StoredWebhook>, BlobStorageError> {
        let stored = self.stored.lock().unwrap();
        Ok(stored.iter().find(|(id, _)| id == event_id).map(|(id, payload)| {
            StoredWebhook {
                metadata: BlobMetadata {
                    blob_path: format!("mock/{}.json", id),
                    event_id: *id,
                    size_bytes: payload.body.len() as u64,
                    created_at: Timestamp::now(),
                    content_type: "application/json".to_string(),
                    checksum_sha256: "mock-checksum-sha256".to_string(),
                    metadata: queue_keeper_core::blob_storage::PayloadMetadata {
                        event_id: *id,
                        event_type: payload.metadata.event_type.clone(),
                        repository: None,
                        signature_valid: true,
                        received_at: Timestamp::now(),
                        delivery_id: None,
                    },
                },
                payload: payload.clone(),
            }
        }))
    }

    async fn list_payloads(
        &self,
        _filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError> {
        Ok(vec![])
    }

    async fn delete_payload(&self, event_id: &EventId) -> Result<(), BlobStorageError> {
        self.stored.lock().unwrap().retain(|(id, _)| id != event_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError> {
        Ok(StorageHealthStatus {
            healthy: true,
            connected: true,
            last_success: Some(Timestamp::now()),
            error_message: None,
            metrics: StorageMetrics {
                avg_write_latency_ms: 0.0,
                avg_read_latency_ms: 0.0,
                success_rate: 1.0,
            },
        })
    }
}

// ============================================================================
// Bot Configuration Helpers
// ============================================================================

/// Create a `BotConfiguration` with `count` bots, each subscribing to all events.
///
/// Queue names follow the pattern `queue-keeper-test-bot-N` (1-indexed).
#[allow(dead_code)]
pub fn create_test_bot_config(count: usize) -> BotConfiguration {
    let bots = (1..=count)
        .map(|i| BotSubscription {
            name: BotName::new(format!("test-bot-{}", i)).unwrap(),
            queue: QueueName::new(format!("queue-keeper-test-bot-{}", i)).unwrap(),
            events: vec![queue_keeper_core::bot_config::EventTypePattern::Wildcard(
                "*".to_string(),
            )],
            ordered: false,
            repository_filter: None,
            config: BotSpecificConfig::new(),
        })
        .collect();

    BotConfiguration {
        bots,
        settings: BotConfigurationSettings::default(),
    }
}

/// Create a `BotConfiguration` with no subscriptions (produces NoTargetQueues).
#[allow(dead_code)]
pub fn create_empty_bot_config() -> BotConfiguration {
    BotConfiguration {
        bots: vec![],
        settings: BotConfigurationSettings::default(),
    }
}

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
    // ServiceMetrics::default() is intentionally used here to create a
    // no-op test stub with unique metric names to avoid Prometheus
    // duplicate-registration conflicts across concurrent tests.
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
        None, // queue_client: disabled in unit/integration tests
        Arc::new(queue_keeper_core::queue_integration::DefaultEventRouter::new()),
        Arc::new(queue_keeper_core::bot_config::BotConfiguration {
            bots: vec![],
            settings: queue_keeper_core::bot_config::BotConfigurationSettings::default(),
        }),
        queue_keeper_api::queue_delivery::QueueDeliveryConfig::default(),
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
