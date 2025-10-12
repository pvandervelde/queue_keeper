//! # Queue-Keeper HTTP Service
//!
//! HTTP server for receiving GitHub webhooks and processing them through the Queue-Keeper system.
//!
//! This service provides:
//! - GitHub webhook endpoint with signature validation
//! - Health check endpoints
//! - Status and monitoring endpoints
//! - Admin API for event management
//!
//! See specs/interfaces/http-service.md for complete specification.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{Json, Response},
    routing::{get, post, put},
    Router,
};
use bytes::Bytes;
use prometheus::{Gauge, Histogram, IntCounter, IntGauge, TextEncoder};
use queue_keeper_core::{
    webhook::{EventEnvelope, WebhookError, WebhookHeaders, WebhookProcessor, WebhookRequest},
    EventId, QueueKeeperError, Repository, SessionId, Timestamp, ValidationError,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, instrument};

// ============================================================================
// Application State
// ============================================================================

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Configuration for the service
    pub config: ServiceConfig,

    /// Webhook processor for handling GitHub events
    pub webhook_processor: Arc<dyn WebhookProcessor>,

    /// Health checker for system monitoring
    pub health_checker: Arc<dyn HealthChecker>,

    /// Event store for querying processed events
    pub event_store: Arc<dyn EventStore>,

    /// Metrics collector for observability
    pub metrics: Arc<ServiceMetrics>,

    /// OpenTelemetry configuration for tracing
    pub telemetry_config: Arc<TelemetryConfig>,
}

impl AppState {
    /// Create new application state
    pub fn new(
        config: ServiceConfig,
        webhook_processor: Arc<dyn WebhookProcessor>,
        health_checker: Arc<dyn HealthChecker>,
        event_store: Arc<dyn EventStore>,
        metrics: Arc<ServiceMetrics>,
        telemetry_config: Arc<TelemetryConfig>,
    ) -> Self {
        Self {
            config,
            webhook_processor,
            health_checker,
            event_store,
            metrics,
            telemetry_config,
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// HTTP server settings
    pub server: ServerConfig,

    /// Webhook processing settings
    pub webhooks: WebhookConfig,

    /// Security settings
    pub security: SecurityConfig,

    /// Logging configuration
    pub logging: LoggingConfig,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            webhooks: WebhookConfig::default(),
            security: SecurityConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Maximum request size in bytes
    pub max_body_size: usize,

    /// Enable CORS
    pub enable_cors: bool,

    /// Enable compression
    pub enable_compression: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            timeout_seconds: 30,
            max_body_size: 10 * 1024 * 1024, // 10MB
            enable_cors: true,
            enable_compression: true,
        }
    }
}

/// Webhook processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook endpoint path
    pub endpoint_path: String,

    /// Require signature validation
    pub require_signature: bool,

    /// Enable payload storage for audit
    pub store_payloads: bool,

    /// Supported event types (empty = all)
    pub allowed_event_types: Vec<String>,

    /// Maximum events per repository per minute
    pub rate_limit_per_repo: Option<u32>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            endpoint_path: "/webhook".to_string(),
            require_signature: true,
            store_payloads: true,
            allowed_event_types: vec![], // All events allowed by default
            rate_limit_per_repo: Some(100), // 100 events per minute per repo
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable request rate limiting
    pub enable_rate_limiting: bool,

    /// Global rate limit (requests per minute)
    pub global_rate_limit: u32,

    /// Enable IP-based rate limiting
    pub enable_ip_rate_limiting: bool,

    /// IP rate limit (requests per minute per IP)
    pub ip_rate_limit: u32,

    /// Enable request logging
    pub log_requests: bool,

    /// Log request bodies (security risk)
    pub log_request_bodies: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_rate_limiting: true,
            global_rate_limit: 1000,
            enable_ip_rate_limiting: true,
            ip_rate_limit: 100,
            log_requests: true,
            log_request_bodies: false,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Logging level
    pub level: String,

    /// Enable JSON structured logging
    pub json_format: bool,

    /// Log file path (optional)
    pub file_path: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            file_path: None,
        }
    }
}

// ============================================================================
// HTTP Server
// ============================================================================

/// Create HTTP router with all endpoints
pub fn create_router(state: AppState) -> Router {
    let webhook_routes = Router::new()
        .route(&state.config.webhooks.endpoint_path, post(handle_webhook))
        .route("/webhook/test", post(handle_webhook_test));

    let health_routes = Router::new()
        .route("/health", get(handle_health_check))
        .route("/health/deep", get(handle_deep_health_check))
        .route("/ready", get(handle_readiness_check));

    let api_routes = Router::new()
        .route("/api/events", get(list_events))
        .route("/api/events/:event_id", get(get_event))
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/:session_id", get(get_session))
        .route("/api/stats", get(get_statistics));

    let observability_routes = Router::new()
        .route("/metrics", get(metrics_endpoint))
        .route("/debug/pprof", get(debug_profile))
        .route("/debug/vars", get(debug_vars));

    let admin_routes = Router::new()
        .route("/admin/events/:event_id/replay", post(replay_event))
        .route("/admin/sessions/:session_id/reset", post(reset_session))
        .route("/admin/config", get(get_config))
        .route("/admin/logging/level", get(get_log_level))
        .route("/admin/logging/level", put(set_log_level))
        .route("/admin/tracing/sampling", get(get_trace_sampling))
        .route("/admin/tracing/sampling", put(set_trace_sampling))
        .route("/admin/metrics/reset", post(reset_metrics));

    Router::new()
        .nest("/", webhook_routes)
        .nest("/", health_routes)
        .nest("/", api_routes)
        .nest("/", observability_routes)
        .nest("/", admin_routes)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(CorsLayer::permissive())
                .layer(middleware::from_fn(request_logging_middleware))
                .layer(middleware::from_fn(metrics_middleware))
                .into_inner(),
        )
        .with_state(state)
}

/// Start HTTP server
pub async fn start_server(
    config: ServiceConfig,
    webhook_processor: Arc<dyn WebhookProcessor>,
    health_checker: Arc<dyn HealthChecker>,
    event_store: Arc<dyn EventStore>,
) -> Result<(), ServiceError> {
    // Initialize observability components
    let metrics = ServiceMetrics::new().map_err(|e| {
        ServiceError::Configuration(ConfigError::Invalid {
            message: format!("Failed to initialize metrics: {}", e),
        })
    })?;

    let telemetry_config = Arc::new(TelemetryConfig::new(
        "queue-keeper".to_string(),
        "development".to_string(), // TODO: Get from environment
    ));

    let state = AppState::new(
        config.clone(),
        webhook_processor,
        health_checker,
        event_store,
        metrics,
        telemetry_config,
    );
    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    let listener =
        tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| ServiceError::BindFailed {
                address: addr.to_string(),
                message: e.to_string(),
            })?;

    info!("Starting HTTP server on {}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| ServiceError::ServerFailed {
            message: e.to_string(),
        })
}

// ============================================================================
// Webhook Handlers
// ============================================================================

/// Handle GitHub webhook requests
#[instrument(skip(state, headers, body))]
async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>, WebhookHandlerError> {
    info!("Received webhook request");

    // Convert headers to HashMap
    let header_map: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    // Parse webhook headers
    let webhook_headers = WebhookHeaders::from_http_headers(&header_map)
        .map_err(WebhookHandlerError::InvalidHeaders)?;

    // Create webhook request
    let webhook_request = WebhookRequest::new(webhook_headers, body);

    // Process webhook through processor
    let event_envelope = state
        .webhook_processor
        .process_webhook(webhook_request)
        .await
        .map_err(WebhookHandlerError::ProcessingFailed)?;

    info!(
        event_id = %event_envelope.event_id,
        event_type = %event_envelope.event_type,
        repository = %event_envelope.repository.full_name,
        session_id = %event_envelope.session_id,
        "Successfully processed webhook"
    );

    Ok(Json(WebhookResponse {
        event_id: event_envelope.event_id,
        session_id: event_envelope.session_id,
        status: "processed".to_string(),
        message: "Webhook processed successfully".to_string(),
    }))
}

/// Handle webhook test requests (for GitHub webhook setup)
#[instrument(skip(state))]
async fn handle_webhook_test(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookTestResponse>, WebhookHandlerError> {
    info!("Received webhook test request");

    // For ping events, just validate headers and return success
    let header_map: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    let webhook_headers = WebhookHeaders::from_http_headers(&header_map)
        .map_err(WebhookHandlerError::InvalidHeaders)?;

    if webhook_headers.event_type == "ping" {
        info!("Processing ping event for webhook setup");
        return Ok(Json(WebhookTestResponse {
            status: "success".to_string(),
            message: "Webhook test successful".to_string(),
            event_type: "ping".to_string(),
        }));
    }

    // For other test events, process normally
    handle_webhook(State(state), headers, body)
        .await
        .map(|_response| {
            Json(WebhookTestResponse {
                status: "success".to_string(),
                message: "Test webhook processed successfully".to_string(),
                event_type: webhook_headers.event_type,
            })
        })
}

// ============================================================================
// Health Check Handlers
// ============================================================================

/// Basic health check endpoint
#[instrument(skip(state))]
async fn handle_health_check(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let status = state.health_checker.check_basic_health().await;

    let response = HealthResponse {
        status: if status.is_healthy {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        timestamp: Timestamp::now(),
        checks: status.checks,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    if status.is_healthy {
        Ok(Json(response))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Deep health check with dependency validation
#[instrument(skip(state))]
async fn handle_deep_health_check(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let status = state.health_checker.check_deep_health().await;

    let response = HealthResponse {
        status: if status.is_healthy {
            "healthy".to_string()
        } else {
            "unhealthy".to_string()
        },
        timestamp: Timestamp::now(),
        checks: status.checks,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    if status.is_healthy {
        Ok(Json(response))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Readiness check for Kubernetes
#[instrument(skip(state))]
async fn handle_readiness_check(
    State(state): State<AppState>,
) -> Result<Json<ReadinessResponse>, StatusCode> {
    let is_ready = state.health_checker.check_readiness().await;

    let response = ReadinessResponse {
        ready: is_ready,
        timestamp: Timestamp::now(),
    };

    if is_ready {
        Ok(Json(response))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

// ============================================================================
// API Handlers (Stubs)
// ============================================================================

/// List recent events
async fn list_events(
    State(_state): State<AppState>,
    Query(_params): Query<EventListParams>,
) -> Result<Json<EventListResponse>, StatusCode> {
    // TODO: Implement event listing
    // See specs/interfaces/http-service.md
    unimplemented!("Event listing not yet implemented")
}

/// Get specific event details
async fn get_event(
    State(_state): State<AppState>,
    Path(_event_id): Path<String>,
) -> Result<Json<EventDetailResponse>, StatusCode> {
    // TODO: Implement event details
    // See specs/interfaces/http-service.md
    unimplemented!("Event details not yet implemented")
}

/// List active sessions
async fn list_sessions(
    State(_state): State<AppState>,
    Query(_params): Query<SessionListParams>,
) -> Result<Json<SessionListResponse>, StatusCode> {
    // TODO: Implement session listing
    // See specs/interfaces/http-service.md
    unimplemented!("Session listing not yet implemented")
}

/// Get specific session details
async fn get_session(
    State(_state): State<AppState>,
    Path(_session_id): Path<String>,
) -> Result<Json<SessionDetailResponse>, StatusCode> {
    // TODO: Implement session details
    // See specs/interfaces/http-service.md
    unimplemented!("Session details not yet implemented")
}

/// Get system statistics
async fn get_statistics(
    State(_state): State<AppState>,
) -> Result<Json<StatisticsResponse>, StatusCode> {
    // TODO: Implement statistics
    // See specs/interfaces/http-service.md
    unimplemented!("Statistics not yet implemented")
}

// ============================================================================
// Observability Handlers
// ============================================================================

/// Prometheus metrics endpoint
#[instrument(skip_all)]
async fn metrics_endpoint(State(_state): State<AppState>) -> Result<String, StatusCode> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();

    encoder
        .encode_to_string(&metric_families)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Debug profiling endpoint (pprof compatible)
#[instrument(skip_all)]
async fn debug_profile(
    State(_state): State<AppState>,
) -> Result<Json<DebugProfileResponse>, StatusCode> {
    // TODO: Implement profiling data collection
    // See specs/interfaces/http-service.md
    Ok(Json(DebugProfileResponse {
        profile_type: "cpu".to_string(),
        duration_seconds: 30,
        samples: 0,
        message: "Profiling not yet implemented".to_string(),
    }))
}

/// Debug variables endpoint
#[instrument(skip_all)]
async fn debug_vars(State(state): State<AppState>) -> Json<DebugVarsResponse> {
    let mut vars = HashMap::new();
    vars.insert(
        "service_name".to_string(),
        state.telemetry_config.service_name.clone(),
    );
    vars.insert(
        "service_version".to_string(),
        state.telemetry_config.service_version.clone(),
    );
    vars.insert(
        "environment".to_string(),
        state.telemetry_config.environment.clone(),
    );
    vars.insert(
        "log_level".to_string(),
        state.telemetry_config.log_level.clone(),
    );
    vars.insert(
        "sampling_ratio".to_string(),
        state.telemetry_config.sampling_ratio.to_string(),
    );
    vars.insert(
        "json_logging".to_string(),
        state.telemetry_config.json_logging.to_string(),
    );

    Json(DebugVarsResponse { vars })
}

// ============================================================================
// Admin Handlers (Stubs)
// ============================================================================

/// Replay an event
async fn replay_event(
    State(_state): State<AppState>,
    Path(_event_id): Path<String>,
) -> Result<Json<ReplayResponse>, StatusCode> {
    // TODO: Implement event replay
    // See specs/interfaces/http-service.md
    unimplemented!("Event replay not yet implemented")
}

/// Reset session state
async fn reset_session(
    State(_state): State<AppState>,
    Path(_session_id): Path<String>,
) -> Result<Json<ResetResponse>, StatusCode> {
    // TODO: Implement session reset
    // See specs/interfaces/http-service.md
    unimplemented!("Session reset not yet implemented")
}

/// Get current configuration
async fn get_config(State(state): State<AppState>) -> Json<ServiceConfig> {
    Json(state.config)
}

/// Get current log level
async fn get_log_level(State(_state): State<AppState>) -> Json<LogLevelResponse> {
    Json(LogLevelResponse {
        level: "info".to_string(), // TODO: Get actual current log level
    })
}

/// Set log level at runtime
async fn set_log_level(
    State(_state): State<AppState>,
    Json(request): Json<SetLogLevelRequest>,
) -> Result<Json<LogLevelResponse>, StatusCode> {
    // In a real implementation, this would update the global tracing subscriber
    // For now, we just validate the level
    match request.level.to_lowercase().as_str() {
        "trace" | "debug" | "info" | "warn" | "error" => {
            // TODO: Update global tracing subscriber level
            info!("Log level change requested: {}", request.level);
            Ok(Json(LogLevelResponse {
                level: request.level,
            }))
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

/// Get current trace sampling configuration
async fn get_trace_sampling(State(_state): State<AppState>) -> Json<TraceSamplingResponse> {
    Json(TraceSamplingResponse {
        sampling_ratio: 1.0, // TODO: Get actual sampling ratio
        service_name: "queue-keeper".to_string(),
    })
}

/// Set trace sampling ratio at runtime
async fn set_trace_sampling(
    State(_state): State<AppState>,
    Json(request): Json<SetTraceSamplingRequest>,
) -> Result<Json<TraceSamplingResponse>, StatusCode> {
    if !(0.0..=1.0).contains(&request.sampling_ratio) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: Update OpenTelemetry sampler configuration
    info!(
        "Trace sampling change requested: {}",
        request.sampling_ratio
    );

    Ok(Json(TraceSamplingResponse {
        sampling_ratio: request.sampling_ratio,
        service_name: "queue-keeper".to_string(),
    }))
}

/// Reset metrics (for development/testing)
async fn reset_metrics(
    State(_state): State<AppState>,
) -> Result<Json<MetricsResetResponse>, StatusCode> {
    // TODO: Implement metrics reset
    // This would clear all prometheus metrics registries
    info!("Metrics reset requested");

    Ok(Json(MetricsResetResponse {
        status: "success".to_string(),
        message: "Metrics reset not yet implemented".to_string(),
        timestamp: Timestamp::now(),
    }))
}

// ============================================================================
// Middleware
// ============================================================================

/// Request logging middleware
async fn request_logging_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    info!("Request started: {} {}", method, uri);

    let response = next.run(request).await;
    let duration = start.elapsed();

    info!(
        "Request completed: {} {} - {} - {:?}",
        method,
        uri,
        response.status(),
        duration
    );

    response
}

/// Metrics collection middleware
async fn metrics_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let start = std::time::Instant::now();
    let method = request.method().clone();
    let uri = request.uri().path().to_string();

    // Get request size
    let request_size = request
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let response = next.run(request).await;
    let duration = start.elapsed();

    // Get response size (simplified - in real implementation would need to intercept response body)
    let response_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // TODO: Record metrics to global metrics collector
    // This would be done via the state in a real implementation
    info!(
        method = %method,
        uri = %uri,
        status = %response.status(),
        duration_ms = %duration.as_millis(),
        request_size = %request_size,
        response_size = %response_size,
        "HTTP request metrics"
    );

    response
}

// ============================================================================
// Response Types
// ============================================================================

/// Webhook processing response
#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub event_id: EventId,
    pub session_id: SessionId,
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
    pub event: EventEnvelope,
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
// Supporting Types (Stubs)
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
// Error Types
// ============================================================================

/// Webhook handler errors
#[derive(Debug, thiserror::Error)]
pub enum WebhookHandlerError {
    #[error("Invalid headers: {0}")]
    InvalidHeaders(#[from] ValidationError),

    #[error("Processing failed: {0}")]
    ProcessingFailed(#[from] WebhookError),

    #[error("Internal server error: {message}")]
    InternalError { message: String },
}

impl axum::response::IntoResponse for WebhookHandlerError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::InvalidHeaders(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::ProcessingFailed(ref e) => {
                if e.is_transient() {
                    (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
                } else {
                    (StatusCode::BAD_REQUEST, self.to_string())
                }
            }
            Self::InternalError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16()
        });

        (status, Json(body)).into_response()
    }
}

/// Service-level errors
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Failed to bind to address {address}: {message}")]
    BindFailed { address: String, message: String },

    #[error("Server failed: {message}")]
    ServerFailed { message: String },

    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigError),

    #[error("Health check failed: {message}")]
    HealthCheckFailed { message: String },
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {message}")]
    Invalid { message: String },

    #[error("Missing required configuration: {key}")]
    Missing { key: String },

    #[error("Configuration parsing failed: {0}")]
    Parsing(#[from] toml::de::Error),
}

// ============================================================================
// Trait Definitions (Stubs)
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
    async fn get_event(&self, event_id: &EventId) -> Result<EventEnvelope, QueueKeeperError>;

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
// Default Implementations (Stubs)
// ============================================================================

/// Default health checker implementation
pub struct DefaultHealthChecker;

#[async_trait::async_trait]
impl HealthChecker for DefaultHealthChecker {
    async fn check_basic_health(&self) -> HealthStatus {
        // TODO: Implement basic health check
        // See specs/interfaces/http-service.md
        unimplemented!("Basic health check not yet implemented")
    }

    async fn check_deep_health(&self) -> HealthStatus {
        // TODO: Implement deep health check
        // See specs/interfaces/http-service.md
        unimplemented!("Deep health check not yet implemented")
    }

    async fn check_readiness(&self) -> bool {
        // TODO: Implement readiness check
        // See specs/interfaces/http-service.md
        unimplemented!("Readiness check not yet implemented")
    }
}

/// Default event store implementation
pub struct DefaultEventStore;

#[async_trait::async_trait]
impl EventStore for DefaultEventStore {
    async fn list_events(
        &self,
        _params: EventListParams,
    ) -> Result<EventListResponse, QueueKeeperError> {
        // TODO: Implement event listing
        // See specs/interfaces/http-service.md
        unimplemented!("Event listing not yet implemented")
    }

    async fn get_event(&self, _event_id: &EventId) -> Result<EventEnvelope, QueueKeeperError> {
        // TODO: Implement event retrieval
        // See specs/interfaces/http-service.md
        unimplemented!("Event retrieval not yet implemented")
    }

    async fn list_sessions(
        &self,
        _params: SessionListParams,
    ) -> Result<SessionListResponse, QueueKeeperError> {
        // TODO: Implement session listing
        // See specs/interfaces/http-service.md
        unimplemented!("Session listing not yet implemented")
    }

    async fn get_session(
        &self,
        _session_id: &SessionId,
    ) -> Result<SessionDetails, QueueKeeperError> {
        // TODO: Implement session retrieval
        // See specs/interfaces/http-service.md
        unimplemented!("Session retrieval not yet implemented")
    }

    async fn get_statistics(&self) -> Result<StatisticsResponse, QueueKeeperError> {
        // TODO: Implement statistics
        // See specs/interfaces/http-service.md
        unimplemented!("Statistics not yet implemented")
    }
}

// ============================================================================
// Observability Types and Implementations
// ============================================================================

/// Service metrics for observability
#[derive(Debug)]
pub struct ServiceMetrics {
    // HTTP request metrics
    pub http_requests_total: IntCounter,
    pub http_request_duration: Histogram,
    pub http_request_size: Histogram,
    pub http_response_size: Histogram,

    // Webhook processing metrics
    pub webhook_requests_total: IntCounter,
    pub webhook_duration_seconds: Histogram,
    pub webhook_validation_failures: IntCounter,
    pub webhook_queue_routing_duration: Histogram,

    // Queue management metrics
    pub queue_depth_messages: IntGauge,
    pub queue_processing_rate: Gauge,
    pub dead_letter_queue_depth: IntGauge,
    pub session_ordering_violations: IntCounter,

    // Error metrics
    pub error_rate_by_category: IntCounter,
    pub circuit_breaker_state: IntGauge,
    pub retry_attempts_total: IntCounter,
    pub blob_storage_failures: IntCounter,
}

impl ServiceMetrics {
    pub fn new() -> Result<Arc<Self>, prometheus::Error> {
        use prometheus::{
            register_gauge, register_histogram, register_int_counter, register_int_gauge,
        };

        Ok(Arc::new(Self {
            http_requests_total: register_int_counter!(
                "http_requests_total",
                "Total number of HTTP requests",
            )?,
            http_request_duration: register_histogram!(
                "http_request_duration_seconds",
                "HTTP request processing time",
                vec![0.001, 0.01, 0.1, 1.0, 10.0]
            )?,
            http_request_size: register_histogram!(
                "http_request_size_bytes",
                "HTTP request size in bytes",
                vec![100.0, 1000.0, 10000.0, 100000.0, 1000000.0]
            )?,
            http_response_size: register_histogram!(
                "http_response_size_bytes",
                "HTTP response size in bytes",
                vec![100.0, 1000.0, 10000.0, 100000.0, 1000000.0]
            )?,

            webhook_requests_total: register_int_counter!(
                "webhook_requests_total",
                "Total webhook requests received"
            )?,
            webhook_duration_seconds: register_histogram!(
                "webhook_duration_seconds",
                "Webhook processing time distribution",
                vec![0.001, 0.01, 0.1, 0.5, 1.0, 2.0, 5.0]
            )?,
            webhook_validation_failures: register_int_counter!(
                "webhook_validation_failures",
                "Invalid signature/payload count"
            )?,
            webhook_queue_routing_duration: register_histogram!(
                "webhook_queue_routing_duration_seconds",
                "Time to route to all bot queues",
                vec![0.001, 0.01, 0.1, 0.2, 0.5, 1.0]
            )?,

            queue_depth_messages: register_int_gauge!(
                "queue_depth_messages",
                "Messages waiting in each bot queue"
            )?,
            queue_processing_rate: register_gauge!(
                "queue_processing_rate",
                "Messages processed per minute"
            )?,
            dead_letter_queue_depth: register_int_gauge!(
                "dead_letter_queue_depth",
                "Failed messages requiring attention"
            )?,
            session_ordering_violations: register_int_counter!(
                "session_ordering_violations",
                "Events processed out of order"
            )?,

            error_rate_by_category: register_int_counter!(
                "error_rate_by_category",
                "Errors grouped by type (4xx, 5xx, network)"
            )?,
            circuit_breaker_state: register_int_gauge!(
                "circuit_breaker_state",
                "Service circuit breaker status"
            )?,
            retry_attempts_total: register_int_counter!(
                "retry_attempts_total",
                "Retry operations by service"
            )?,
            blob_storage_failures: register_int_counter!(
                "blob_storage_failures",
                "Audit trail storage failures"
            )?,
        }))
    }

    pub fn record_http_request(
        &self,
        duration: std::time::Duration,
        request_size: u64,
        response_size: u64,
    ) {
        self.http_requests_total.inc();
        self.http_request_duration.observe(duration.as_secs_f64());
        self.http_request_size.observe(request_size as f64);
        self.http_response_size.observe(response_size as f64);
    }

    pub fn record_webhook_request(&self, duration: std::time::Duration, success: bool) {
        self.webhook_requests_total.inc();
        self.webhook_duration_seconds
            .observe(duration.as_secs_f64());
        if !success {
            self.webhook_validation_failures.inc();
        }
    }
}

impl Default for ServiceMetrics {
    fn default() -> Self {
        // This is not safe for production - should handle errors properly
        // For now, we'll create a stub implementation
        use prometheus::{
            register_gauge, register_histogram, register_int_counter, register_int_gauge,
        };

        Self {
            http_requests_total: register_int_counter!("http_requests_total_default", "").unwrap(),
            http_request_duration: register_histogram!(
                "http_request_duration_seconds_default",
                "",
                vec![]
            )
            .unwrap(),
            http_request_size: register_histogram!("http_request_size_bytes_default", "", vec![])
                .unwrap(),
            http_response_size: register_histogram!("http_response_size_bytes_default", "", vec![])
                .unwrap(),
            webhook_requests_total: register_int_counter!("webhook_requests_total_default", "")
                .unwrap(),
            webhook_duration_seconds: register_histogram!(
                "webhook_duration_seconds_default",
                "",
                vec![]
            )
            .unwrap(),
            webhook_validation_failures: register_int_counter!(
                "webhook_validation_failures_default",
                ""
            )
            .unwrap(),
            webhook_queue_routing_duration: register_histogram!(
                "webhook_queue_routing_duration_seconds_default",
                "",
                vec![]
            )
            .unwrap(),
            queue_depth_messages: register_int_gauge!("queue_depth_messages_default", "").unwrap(),
            queue_processing_rate: register_gauge!("queue_processing_rate_default", "").unwrap(),
            dead_letter_queue_depth: register_int_gauge!("dead_letter_queue_depth_default", "")
                .unwrap(),
            session_ordering_violations: register_int_counter!(
                "session_ordering_violations_default",
                ""
            )
            .unwrap(),
            error_rate_by_category: register_int_counter!("error_rate_by_category_default", "")
                .unwrap(),
            circuit_breaker_state: register_int_gauge!("circuit_breaker_state_default", "")
                .unwrap(),
            retry_attempts_total: register_int_counter!("retry_attempts_total_default", "")
                .unwrap(),
            blob_storage_failures: register_int_counter!("blob_storage_failures_default", "")
                .unwrap(),
        }
    }
}

/// OpenTelemetry configuration for distributed tracing
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Service name for tracing
    pub service_name: String,

    /// Service version
    pub service_version: String,

    /// Environment (dev, staging, prod)
    pub environment: String,

    /// Trace sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,

    /// Enable JSON logging
    pub json_logging: bool,

    /// Current log level
    pub log_level: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "queue-keeper".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: "development".to_string(),
            sampling_ratio: 1.0, // 100% sampling in development
            json_logging: false,
            log_level: "info".to_string(),
        }
    }
}

impl TelemetryConfig {
    pub fn new(service_name: String, environment: String) -> Self {
        let is_production = environment == "production";
        Self {
            service_name,
            sampling_ratio: if is_production { 0.1 } else { 1.0 },
            json_logging: is_production,
            environment,
            ..Default::default()
        }
    }

    pub fn set_log_level(&mut self, level: String) -> Result<(), String> {
        match level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {
                self.log_level = level;
                Ok(())
            }
            _ => Err(format!("Invalid log level: {}", level)),
        }
    }

    pub fn set_sampling_ratio(&mut self, ratio: f64) -> Result<(), String> {
        if (0.0..=1.0).contains(&ratio) {
            self.sampling_ratio = ratio;
            Ok(())
        } else {
            Err("Sampling ratio must be between 0.0 and 1.0".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = ServiceConfig::default();
        let webhook_processor = Arc::new(queue_keeper_core::webhook::DefaultWebhookProcessor);
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

        let server = TestServer::new(app).unwrap();

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
}
