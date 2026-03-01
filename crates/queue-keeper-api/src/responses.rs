//! Response types, query parameters, and supporting types for the API.

use queue_keeper_core::webhook::WrappedEvent;
use queue_keeper_core::{EventId, QueueKeeperError, Repository, SessionId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
