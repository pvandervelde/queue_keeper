//! Response types, query parameters, and supporting types for the API.

use crate::ProviderRegistry;
use queue_keeper_core::blob_storage::{
    BlobStorage, BlobStorageError, PayloadFilter, PayloadMetadata, WebhookPayload,
};
use queue_keeper_core::webhook::WrappedEvent;
use queue_keeper_core::{EventId, QueueKeeperError, Repository, SessionId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, warn};

// ============================================================================
// Response Types
// ============================================================================

/// Webhook processing response
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub event_id: EventId,
    pub session_id: Option<SessionId>,
    pub status: String,
    pub message: String,
}

/// Webhook test response
#[derive(Debug, Serialize)]
pub struct WebhookTestResponse {
    pub status: String,
    pub message: String,
    pub event_type: String,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: Timestamp,
    pub checks: HashMap<String, HealthCheckResult>,
    pub version: String,
}

/// Readiness check response
#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub timestamp: Timestamp,
}

/// Event list response
#[derive(Debug, Serialize)]
pub struct EventListResponse {
    pub events: Vec<EventSummary>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

/// Event detail response
#[derive(Debug, Serialize)]
pub struct EventDetailResponse {
    pub event: WrappedEvent,
}

/// Session list response
#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionSummary>,
    pub total: usize,
}

/// Session detail response
#[derive(Debug, Serialize)]
pub struct SessionDetailResponse {
    pub session: SessionDetails,
}

/// Statistics response
#[derive(Debug, Serialize)]
pub struct StatisticsResponse {
    pub total_events: u64,
    pub events_per_hour: f64,
    pub active_sessions: u64,
    pub error_rate: f64,
    pub uptime_seconds: u64,
}

/// Event replay response
#[derive(Debug, Serialize)]
pub struct ReplayResponse {
    pub event_id: EventId,
    pub status: String,
    pub message: String,
}

/// Session reset response
#[derive(Debug, Serialize)]
pub struct ResetResponse {
    pub session_id: SessionId,
    pub status: String,
    pub message: String,
}

/// Debug profile response
#[derive(Debug, Serialize)]
pub struct DebugProfileResponse {
    pub profile_type: String,
    pub duration_seconds: u64,
    pub samples: u64,
    pub message: String,
}

/// Debug variables response
#[derive(Debug, Serialize)]
pub struct DebugVarsResponse {
    pub vars: HashMap<String, String>,
}

/// Log level response
#[derive(Debug, Serialize)]
pub struct LogLevelResponse {
    pub level: String,
}

/// Set log level request
#[derive(Debug, Deserialize)]
pub struct SetLogLevelRequest {
    pub level: String,
}

/// Trace sampling response
#[derive(Debug, Serialize)]
pub struct TraceSamplingResponse {
    pub sampling_ratio: f64,
    pub service_name: String,
}

/// Set trace sampling request
#[derive(Debug, Deserialize)]
pub struct SetTraceSamplingRequest {
    pub sampling_ratio: f64,
}

/// Metrics reset response
#[derive(Debug, Serialize)]
pub struct MetricsResetResponse {
    pub status: String,
    pub message: String,
    pub timestamp: Timestamp,
}

// ============================================================================
// Query Parameter Types
// ============================================================================

/// Parameters for event listing
#[derive(Debug, Deserialize)]
pub struct EventListParams {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub event_type: Option<String>,
    pub repository: Option<String>,
    pub session_id: Option<String>,
    pub since: Option<String>,
}

/// Parameters for session listing
#[derive(Debug, Deserialize)]
pub struct SessionListParams {
    pub repository: Option<String>,
    pub entity_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Event summary for listing
#[derive(Debug, Serialize)]
pub struct EventSummary {
    pub event_id: EventId,
    pub event_type: String,
    pub repository: String,
    pub session_id: SessionId,
    pub occurred_at: Timestamp,
    pub status: String,
}

/// Session summary for listing
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub repository: String,
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub event_count: u32,
    pub last_activity: Timestamp,
}

/// Detailed session information
#[derive(Debug, Serialize)]
pub struct SessionDetails {
    pub session_id: SessionId,
    pub repository: Repository,
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
    pub event_count: u32,
    pub events: Vec<EventSummary>,
}

/// Health check result for individual components
#[derive(Debug, Serialize, Clone)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub message: String,
    pub duration_ms: u64,
}

/// Overall health status
#[derive(Debug)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub checks: HashMap<String, HealthCheckResult>,
}

// ============================================================================
// Trait Definitions
// ============================================================================

/// Interface for system health monitoring
#[async_trait::async_trait]
pub trait HealthChecker: Send + Sync {
    /// Basic health check (fast)
    async fn check_basic_health(&self) -> HealthStatus;

    /// Deep health check with dependencies
    async fn check_deep_health(&self) -> HealthStatus;

    /// Readiness check for load balancers
    async fn check_readiness(&self) -> bool;
}

/// Interface for event storage and querying
#[async_trait::async_trait]
pub trait EventStore: Send + Sync {
    /// List events with filters and pagination
    async fn list_events(
        &self,
        params: EventListParams,
    ) -> Result<EventListResponse, QueueKeeperError>;

    /// Get event by ID
    async fn get_event(&self, event_id: &EventId) -> Result<WrappedEvent, QueueKeeperError>;

    /// List sessions with filters
    async fn list_sessions(
        &self,
        params: SessionListParams,
    ) -> Result<SessionListResponse, QueueKeeperError>;

    /// Get session details
    async fn get_session(&self, session_id: &SessionId)
        -> Result<SessionDetails, QueueKeeperError>;

    /// Get system statistics
    async fn get_statistics(&self) -> Result<StatisticsResponse, QueueKeeperError>;
}

// ============================================================================
// Default Implementations
// ============================================================================

/// Default health checker implementation
pub struct DefaultHealthChecker;

#[async_trait::async_trait]
impl HealthChecker for DefaultHealthChecker {
    async fn check_basic_health(&self) -> HealthStatus {
        let start = std::time::Instant::now();
        let mut checks = HashMap::new();

        // Basic service check - if we can respond, we're alive
        checks.insert(
            "service".to_string(),
            HealthCheckResult {
                healthy: true,
                message: "Service is running".to_string(),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        );

        HealthStatus {
            is_healthy: true,
            checks,
        }
    }

    async fn check_deep_health(&self) -> HealthStatus {
        let start = std::time::Instant::now();
        let mut checks = HashMap::new();
        let overall_healthy = true;

        // Service check
        checks.insert(
            "service".to_string(),
            HealthCheckResult {
                healthy: true,
                message: "Service is running".to_string(),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        );

        // TODO: Add dependency checks when integrated:
        // - Queue provider connectivity
        // - Blob storage accessibility
        // - Key vault connectivity
        // For now, deep health is same as basic health

        HealthStatus {
            is_healthy: overall_healthy,
            checks,
        }
    }

    async fn check_readiness(&self) -> bool {
        // Readiness check - service is ready to accept traffic
        // For now, if the service is running, it's ready
        // TODO: Add checks for:
        // - Configuration loaded successfully
        // - Required dependencies initialized
        // - No circuit breakers open
        true
    }
}

/// Production health checker that verifies service-level readiness.
///
/// Unlike [`DefaultHealthChecker`], this implementation checks that at least
/// one webhook provider is registered before reporting ready. An empty provider
/// registry means the service cannot process any incoming webhooks, so traffic
/// should not be routed to it.
///
/// # Readiness contract
///
/// - **Ready** (`true`): ≥ 1 provider registered.
/// - **Not ready** (`false`): provider registry is empty (Kubernetes will not route
///   traffic until a subsequent `/ready` poll returns 200).
pub struct ServiceHealthChecker {
    provider_registry: Arc<ProviderRegistry>,
}

impl ServiceHealthChecker {
    /// Create a new checker bound to the given provider registry.
    pub fn new(provider_registry: Arc<ProviderRegistry>) -> Self {
        Self { provider_registry }
    }
}

#[async_trait::async_trait]
impl HealthChecker for ServiceHealthChecker {
    async fn check_basic_health(&self) -> HealthStatus {
        let start = std::time::Instant::now();
        let mut checks = HashMap::new();

        let provider_count = self.provider_registry.len();
        let providers_healthy = provider_count > 0;
        checks.insert(
            "service".to_string(),
            HealthCheckResult {
                healthy: true,
                message: "Service is running".to_string(),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        );
        checks.insert(
            "providers".to_string(),
            HealthCheckResult {
                healthy: providers_healthy,
                message: format!("{} webhook provider(s) registered", provider_count),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        );

        HealthStatus {
            is_healthy: providers_healthy,
            checks,
        }
    }

    async fn check_deep_health(&self) -> HealthStatus {
        let start = std::time::Instant::now();
        let mut checks = HashMap::new();

        let provider_count = self.provider_registry.len();
        let providers_healthy = provider_count > 0;

        checks.insert(
            "service".to_string(),
            HealthCheckResult {
                healthy: true,
                message: "Service is running".to_string(),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        );
        checks.insert(
            "providers".to_string(),
            HealthCheckResult {
                healthy: providers_healthy,
                message: format!("{} webhook provider(s) registered", provider_count),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        );

        HealthStatus {
            is_healthy: providers_healthy,
            checks,
        }
    }

    async fn check_readiness(&self) -> bool {
        // Ready when at least one webhook provider is registered.
        // An empty registry means the service cannot handle any incoming
        // webhooks, so Kubernetes should not route traffic to this pod.
        !self.provider_registry.is_empty()
    }
}

// ============================================================================
// Blob-Backed Event Store
// ============================================================================

/// Persist a [`WrappedEvent`] to blob storage so it can later be queried via
/// [`BlobBackedEventStore`].
///
/// The event is serialised as JSON and wrapped in a synthetic [`WebhookPayload`]
/// stored under the wrapped-event's own `event_id`. The blob storage
/// implementation computes and persists a SHA-256 checksum at write time and
/// verifies it on every read, providing tamper-evidence for free.
///
/// # Errors
///
/// Returns a [`BlobStorageError`] if serialisation or the underlying storage
/// operation fails. The caller is expected to log the error and treat blob
/// persistence failures as non-fatal (events are still delivered to queues).
pub async fn store_wrapped_event_to_blob(
    storage: &dyn BlobStorage,
    event: &WrappedEvent,
) -> Result<(), BlobStorageError> {
    let body_json =
        serde_json::to_vec(event).map_err(|e| BlobStorageError::SerializationFailed {
            message: format!("Failed to serialise WrappedEvent {}: {}", event.event_id, e),
        })?;

    let body = bytes::Bytes::from(body_json);

    // Extract repository from payload when available so that `list_payloads`
    // repository filtering works correctly.
    let repository = extract_repository_from_wrapped_event(event);

    let payload = WebhookPayload {
        body,
        headers: HashMap::new(),
        metadata: PayloadMetadata {
            event_id: event.event_id,
            event_type: event.event_type.clone(),
            repository,
            signature_valid: true,
            received_at: event.received_at,
            delivery_id: None,
        },
    };

    storage
        .store_payload(&event.event_id, &payload)
        .await
        .map(|_| ())
}

/// Extract repository information from a [`WrappedEvent`]'s JSON payload.
///
/// Returns `None` when the payload does not contain recognisable repository
/// fields (e.g., non-GitHub providers).
fn extract_repository_from_wrapped_event(event: &WrappedEvent) -> Option<Repository> {
    use queue_keeper_core::{RepositoryId, User, UserId, UserType};

    let repo_data = event.payload.get("repository")?;

    let id = repo_data.get("id")?.as_u64()?;
    let name = repo_data.get("name")?.as_str()?.to_string();
    let full_name = repo_data.get("full_name")?.as_str()?.to_string();
    let private = repo_data.get("private").and_then(|p| p.as_bool()).unwrap_or(false);

    let owner = repo_data.get("owner").and_then(|o| {
        let owner_id = o.get("id")?.as_u64()?;
        let login = o.get("login")?.as_str()?.to_string();
        let user_type = match o.get("type").and_then(|t| t.as_str()) {
            Some("Organization") => UserType::Organization,
            Some("Bot") => UserType::Bot,
            _ => UserType::User,
        };
        Some(User {
            id: UserId::new(owner_id),
            login,
            user_type,
        })
    })?;

    Some(Repository::new(
        RepositoryId::new(id),
        name,
        full_name,
        owner,
        private,
    ))
}

/// Event store backed by blob storage.
///
/// Persisted [`WrappedEvent`] objects are read from the blob storage instance
/// supplied at construction. Events must have been written there via
/// [`store_wrapped_event_to_blob`] (called by the webhook handler after each
/// successful processing pass).
///
/// # Session Queries
///
/// Sessions are derived by grouping events that share the same `session_id`.
/// Listing sessions requires loading every stored event body, which is O(n)
/// in the number of stored events. This is acceptable for the expected data
/// volumes; a dedicated session index would be required at larger scale.
///
/// # Uptime Tracking
///
/// The store records the instant it was created so that `get_statistics` can
/// report a meaningful `uptime_seconds` value.
pub struct BlobBackedEventStore {
    storage: Arc<dyn BlobStorage>,
    started_at: Instant,
}

impl BlobBackedEventStore {
    /// Create a new store wrapping the provided blob storage.
    pub fn new(storage: Arc<dyn BlobStorage>) -> Self {
        Self {
            storage,
            started_at: Instant::now(),
        }
    }

    /// Deserialise a [`WrappedEvent`] from a [`StoredWebhook`] body.
    fn deserialise_event(
        stored: &queue_keeper_core::blob_storage::StoredWebhook,
    ) -> Option<WrappedEvent> {
        match serde_json::from_slice(&stored.payload.body) {
            Ok(event) => Some(event),
            Err(e) => {
                warn!(
                    event_id = %stored.metadata.event_id,
                    error = %e,
                    "Failed to deserialise WrappedEvent from blob; skipping"
                );
                None
            }
        }
    }

    /// Map a `BlobStorageError` onto the appropriate `QueueKeeperError`.
    fn map_storage_error(err: BlobStorageError) -> QueueKeeperError {
        match err {
            BlobStorageError::BlobNotFound { event_id } => QueueKeeperError::NotFound {
                resource: "event".to_string(),
                id: event_id.to_string(),
            },
            BlobStorageError::ChecksumMismatch { path, expected, actual } => {
                error!(
                    path = %path,
                    expected_checksum = %expected,
                    actual_checksum = %actual,
                    "Checksum mismatch detected — event data may be corrupted or tampered with"
                );
                QueueKeeperError::Internal {
                    message: format!("Checksum mismatch for stored event at {path}"),
                }
            }
            e => QueueKeeperError::ExternalService {
                service: "blob_storage".to_string(),
                message: e.to_string(),
            },
        }
    }

    /// Extract the repository full name from a [`WrappedEvent`]'s payload, if present.
    fn repo_full_name(event: &WrappedEvent) -> Option<String> {
        event
            .payload
            .get("repository")
            .and_then(|r| r.get("full_name"))
            .and_then(|f| f.as_str())
            .map(str::to_string)
    }

    /// Convert a [`WrappedEvent`] into an [`EventSummary`].
    fn to_event_summary(event: &WrappedEvent) -> EventSummary {
        EventSummary {
            event_id: event.event_id,
            event_type: event.event_type.clone(),
            repository: Self::repo_full_name(event).unwrap_or_else(|| "unknown".to_string()),
            session_id: event
                .session_id
                .clone()
                .unwrap_or_else(|| SessionId::from_parts("unknown", "unknown", "unknown", "0")),
            occurred_at: event.received_at,
            status: "processed".to_string(),
        }
    }

    /// Load all stored events, deserialising each blob body.
    ///
    /// Blobs that fail to deserialise (e.g. raw webhook payloads accidentally
    /// in the same storage) are silently skipped with a warning.
    async fn load_all_events(
        &self,
        filter: &PayloadFilter,
    ) -> Result<Vec<WrappedEvent>, QueueKeeperError> {
        let blob_list = self
            .storage
            .list_payloads(filter)
            .await
            .map_err(Self::map_storage_error)?;

        let mut events = Vec::with_capacity(blob_list.len());
        for meta in blob_list {
            match self.storage.get_payload(&meta.event_id).await {
                Ok(Some(stored)) => {
                    if let Some(event) = Self::deserialise_event(&stored) {
                        events.push(event);
                    }
                }
                Ok(None) => {
                    debug!(
                        event_id = %meta.event_id,
                        "Blob listed but not found during load; may have been deleted"
                    );
                }
                Err(e) => {
                    warn!(event_id = %meta.event_id, error = %e, "Error loading event blob; skipping");
                }
            }
        }

        Ok(events)
    }
}

#[async_trait::async_trait]
impl EventStore for BlobBackedEventStore {
    async fn list_events(
        &self,
        params: EventListParams,
    ) -> Result<EventListResponse, QueueKeeperError> {
        let page = params.page.unwrap_or(1).max(1);
        let per_page = params.per_page.unwrap_or(50).clamp(1, 500);
        let offset = (page - 1) * per_page;

        let filter = PayloadFilter {
            repository: params.repository.clone(),
            event_type: params.event_type.clone(),
            ..Default::default()
        };

        let all_events = self.load_all_events(&filter).await?;

        // Apply session filter if provided (not a blob-level filter)
        let filtered: Vec<&WrappedEvent> = if let Some(ref session_filter) = params.session_id {
            all_events
                .iter()
                .filter(|e| {
                    e.session_id
                        .as_ref()
                        .map(|s| s.as_str() == session_filter)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            all_events.iter().collect()
        };

        let total = filtered.len();
        let events = filtered
            .into_iter()
            .skip(offset)
            .take(per_page)
            .map(Self::to_event_summary)
            .collect();

        Ok(EventListResponse {
            events,
            total,
            page,
            per_page,
        })
    }

    async fn get_event(&self, event_id: &EventId) -> Result<WrappedEvent, QueueKeeperError> {
        match self.storage.get_payload(event_id).await {
            Ok(Some(stored)) => serde_json::from_slice::<WrappedEvent>(&stored.payload.body)
                .map_err(|e| QueueKeeperError::Internal {
                    message: format!(
                        "Failed to deserialise event {}: {}",
                        event_id, e
                    ),
                }),
            Ok(None) => Err(QueueKeeperError::NotFound {
                resource: "event".to_string(),
                id: event_id.to_string(),
            }),
            Err(e) => Err(Self::map_storage_error(e)),
        }
    }

    async fn list_sessions(
        &self,
        params: SessionListParams,
    ) -> Result<SessionListResponse, QueueKeeperError> {
        let filter = PayloadFilter {
            repository: params.repository.clone(),
            ..Default::default()
        };

        let all_events = self.load_all_events(&filter).await?;

        // Group events by session_id
        let mut session_map: HashMap<String, Vec<&WrappedEvent>> = HashMap::new();
        for event in &all_events {
            if let Some(ref sid) = event.session_id {
                session_map
                    .entry(sid.as_str().to_string())
                    .or_default()
                    .push(event);
            }
        }

        // Apply entity_type filter if provided
        let limit = params.limit.unwrap_or(usize::MAX);

        let mut sessions: Vec<SessionSummary> = session_map
            .iter()
            .filter(|(sid, events)| {
                // entity_type filter: session_id format is owner/repo/entity_type/entity_id
                if let Some(ref entity_type_filter) = params.entity_type {
                    let parts: Vec<&str> = sid.splitn(4, '/').collect();
                    if parts.len() >= 3 && parts[2] != entity_type_filter {
                        return false;
                    }
                }
                // status filter: we treat all sessions as "active" for now
                if let Some(ref status_filter) = params.status {
                    if status_filter != "active" {
                        // Only "active" sessions are currently tracked
                        let _ = events; // suppress unused warning
                        return false;
                    }
                }
                true
            })
            .map(|(sid, events)| {
                let last_activity = events
                    .iter()
                    .map(|e| e.received_at)
                    .max()
                    .unwrap_or_else(Timestamp::now);
                let first_event = events[0];

                // Parse repo from session_id: owner/repo/entity_type/entity_id
                let parts: Vec<&str> = sid.splitn(4, '/').collect();
                let repository = if parts.len() >= 2 {
                    format!("{}/{}", parts[0], parts[1])
                } else {
                    "unknown/unknown".to_string()
                };
                let entity_type = parts.get(2).copied().unwrap_or("unknown").to_string();
                let entity_id = parts.get(3).copied().unwrap_or("0").to_string();

                SessionSummary {
                    session_id: first_event
                        .session_id
                        .clone()
                        .unwrap_or_else(|| SessionId::from_parts("unknown", "unknown", "unknown", "0")),
                    repository,
                    entity_type,
                    entity_id,
                    status: "active".to_string(),
                    event_count: events.len() as u32,
                    last_activity,
                }
            })
            .take(limit)
            .collect();

        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        let total = sessions.len();
        Ok(SessionListResponse { sessions, total })
    }

    async fn get_session(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionDetails, QueueKeeperError> {
        let all_events = self.load_all_events(&PayloadFilter::default()).await?;

        let session_events: Vec<&WrappedEvent> = all_events
            .iter()
            .filter(|e| {
                e.session_id
                    .as_ref()
                    .map(|s| s == session_id)
                    .unwrap_or(false)
            })
            .collect();

        if session_events.is_empty() {
            return Err(QueueKeeperError::NotFound {
                resource: "session".to_string(),
                id: session_id.to_string(),
            });
        }

        // Parse repository from session_id: owner/repo/entity_type/entity_id
        let sid_str = session_id.as_str();
        let parts: Vec<&str> = sid_str.splitn(4, '/').collect();
        let repo_full_name = if parts.len() >= 2 {
            format!("{}/{}", parts[0], parts[1])
        } else {
            "unknown/unknown".to_string()
        };
        let entity_type = parts.get(2).copied().unwrap_or("unknown").to_string();
        let entity_id = parts.get(3).copied().unwrap_or("0").to_string();

        let created_at = session_events
            .iter()
            .map(|e| e.received_at)
            .min()
            .unwrap_or_else(Timestamp::now);
        let last_activity = session_events
            .iter()
            .map(|e| e.received_at)
            .max()
            .unwrap_or_else(Timestamp::now);

        // Build the Repository from the first event's payload or session_id parts
        let first_event = session_events[0];
        let repository = if let Some(full_name) = Self::repo_full_name(first_event) {
            // Use the canonical Repository type from the payload when available
            use queue_keeper_core::{Repository, RepositoryId, User, UserId, UserType};
            let repo_data = first_event.payload.get("repository");
            let id = repo_data
                .and_then(|r| r.get("id"))
                .and_then(|i| i.as_u64())
                .unwrap_or(0);
            let (owner_login, name) = full_name
                .split_once('/')
                .map(|(o, n)| (o.to_string(), n.to_string()))
                .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));
            Repository::new(
                RepositoryId::new(id),
                name,
                full_name,
                User {
                    id: UserId::new(0),
                    login: owner_login,
                    user_type: UserType::User,
                },
                false,
            )
        } else {
            use queue_keeper_core::{Repository, RepositoryId, User, UserId, UserType};
            let (owner, repo) = if parts.len() >= 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                ("unknown".to_string(), "unknown".to_string())
            };
            Repository::new(
                RepositoryId::new(0),
                repo.clone(),
                repo_full_name.clone(),
                User {
                    id: UserId::new(0),
                    login: owner,
                    user_type: UserType::User,
                },
                false,
            )
        };

        let event_summaries: Vec<EventSummary> =
            session_events.iter().map(|e| Self::to_event_summary(e)).collect();

        Ok(SessionDetails {
            session_id: session_id.clone(),
            repository,
            entity_type,
            entity_id,
            status: "active".to_string(),
            created_at,
            last_activity,
            event_count: event_summaries.len() as u32,
            events: event_summaries,
        })
    }

    async fn get_statistics(&self) -> Result<StatisticsResponse, QueueKeeperError> {
        let all_events = self
            .storage
            .list_payloads(&PayloadFilter::default())
            .await
            .map_err(Self::map_storage_error)?;

        let total_events = all_events.len() as u64;

        // Estimate events per hour from oldest and newest timestamps
        let events_per_hour = if total_events >= 2 {
            let timestamps: Vec<Timestamp> =
                all_events.iter().map(|m| m.created_at).collect();
            let oldest = timestamps.iter().min().copied();
            let newest = timestamps.iter().max().copied();
            if let (Some(oldest), Some(newest)) = (oldest, newest) {
                let span_secs = newest
                    .as_datetime()
                    .signed_duration_since(oldest.as_datetime())
                    .num_seconds()
                    .max(1) as f64;
                let span_hours = span_secs / 3600.0;
                (total_events as f64) / span_hours
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Count unique session IDs from stored blob metadata event types
        // (session_id is not in BlobMetadata, so we use a heuristic: assume
        // "active sessions" ≈ distinct event counts within the last hour)
        // A full count would require loading all bodies, which we avoid here.
        let active_sessions = 0u64; // Conservative default without loading bodies

        let uptime_seconds = self.started_at.elapsed().as_secs();

        Ok(StatisticsResponse {
            total_events,
            events_per_hour,
            active_sessions,
            error_rate: 0.0,
            uptime_seconds,
        })
    }
}

/// Default event store implementation
pub struct DefaultEventStore;

#[async_trait::async_trait]
impl EventStore for DefaultEventStore {
    async fn list_events(
        &self,
        params: EventListParams,
    ) -> Result<EventListResponse, QueueKeeperError> {
        // For now, return empty list - implementation will come with storage integration
        Ok(EventListResponse {
            events: vec![],
            total: 0,
            page: params.page.unwrap_or(1),
            per_page: params.per_page.unwrap_or(50),
        })
    }

    async fn get_event(&self, event_id: &EventId) -> Result<WrappedEvent, QueueKeeperError> {
        // For now, return not found - implementation will come with storage integration
        Err(QueueKeeperError::NotFound {
            resource: "event".to_string(),
            id: event_id.to_string(),
        })
    }

    async fn list_sessions(
        &self,
        params: SessionListParams,
    ) -> Result<SessionListResponse, QueueKeeperError> {
        // For now, return empty list - implementation will come with storage integration
        let _ = params; // Silence unused warning
        Ok(SessionListResponse {
            sessions: vec![],
            total: 0,
        })
    }

    async fn get_session(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionDetails, QueueKeeperError> {
        // For now, return not found - implementation will come with storage integration
        Err(QueueKeeperError::NotFound {
            resource: "session".to_string(),
            id: session_id.to_string(),
        })
    }

    async fn get_statistics(&self) -> Result<StatisticsResponse, QueueKeeperError> {
        // For now, return zero statistics - implementation will come with storage integration
        Ok(StatisticsResponse {
            total_events: 0,
            events_per_hour: 0.0,
            active_sessions: 0,
            error_rate: 0.0,
            uptime_seconds: 0,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "responses_tests.rs"]
mod tests;
