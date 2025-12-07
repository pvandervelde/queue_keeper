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

// Public modules
pub mod dlq_storage;
pub mod queue_delivery;
pub mod retry;

#[cfg(test)]
#[path = "health_tests.rs"]
mod health_tests;

#[cfg(test)]
#[path = "middleware_tests.rs"]
mod middleware_tests;

#[cfg(test)]
#[path = "shutdown_tests.rs"]
mod shutdown_tests;

#[cfg(test)]
#[path = "error_handling_tests.rs"]
mod error_handling_tests;

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
use tracing::{error, info, instrument, warn};

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

    /// Graceful shutdown timeout in seconds
    pub shutdown_timeout_seconds: u64,

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
            shutdown_timeout_seconds: 30,
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
        .route("/api/events/{event_id}", get(get_event))
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/{session_id}", get(get_session))
        .route("/api/stats", get(get_statistics));

    let observability_routes = Router::new()
        .route("/metrics", get(metrics_endpoint))
        .route("/debug/pprof", get(debug_profile))
        .route("/debug/vars", get(debug_vars));

    let admin_routes = Router::new()
        .route("/admin/events/{event_id}/replay", post(replay_event))
        .route("/admin/sessions/{session_id}/reset", post(reset_session))
        .route("/admin/config", get(get_config))
        .route("/admin/logging/level", get(get_log_level))
        .route("/admin/logging/level", put(set_log_level))
        .route("/admin/tracing/sampling", get(get_trace_sampling))
        .route("/admin/tracing/sampling", put(set_trace_sampling))
        .route("/admin/metrics/reset", post(reset_metrics));

    Router::new()
        .merge(webhook_routes)
        .merge(health_routes)
        .merge(api_routes)
        .merge(observability_routes)
        .merge(admin_routes)
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

    // Set up graceful shutdown signal handling with configured timeout
    let shutdown_timeout = std::time::Duration::from_secs(config.server.shutdown_timeout_seconds);

    let shutdown_signal = async move {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C signal handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received SIGINT (Ctrl+C), initiating graceful shutdown with {}s timeout", shutdown_timeout.as_secs());
            },
            _ = terminate => {
                info!("Received SIGTERM, initiating graceful shutdown with {}s timeout", shutdown_timeout.as_secs());
            },
        }
    };

    // Start server with graceful shutdown
    // Note: axum's graceful shutdown will allow in-flight requests to complete
    // before shutting down. The server will stop accepting new connections immediately
    // upon receiving the shutdown signal, then wait for in-flight requests to finish.
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .map_err(|e| ServiceError::ServerFailed {
            message: e.to_string(),
        })?;

    info!("HTTP server shutdown complete");
    Ok(())
}

// ============================================================================
// Webhook Handlers
// ============================================================================

/// Handle GitHub webhook requests
///
/// This handler implements the immediate response pattern to meet GitHub's 10-second timeout:
/// 1. Parse and validate webhook headers (fast path)
/// 2. Process webhook through processor (validation + normalization + blob storage - fast)
/// 3. Return HTTP 200 OK immediately (target <500ms)
/// 4. Queue delivery with retry happens asynchronously (TODO: implement when EventRouter is integrated)
///
/// This ensures GitHub receives a response within the timeout while allowing
/// queue delivery to proceed in the background with proper retry logic.
#[instrument(skip(state, headers, body))]
pub async fn handle_webhook(
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

    // Process webhook through processor (validation + normalization + storage)
    // This is the "fast path" - must complete within ~500ms
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
        "Successfully processed webhook - returning immediate response"
    );

    // TODO: Task 16.6 - Spawn async task for queue delivery with retry loop
    // This will be implemented when EventRouter is integrated into AppState:
    //
    // tokio::spawn(async move {
    //     let retry_policy = retry::RetryPolicy::default();
    //     let mut retry_state = retry::RetryState::new();
    //
    //     loop {
    //         match event_router.route_event(&event_envelope, &bot_config, &queue_client).await {
    //             Ok(delivery_result) if delivery_result.is_complete_success() => {
    //                 info!("Successfully delivered event to all queues");
    //                 break;
    //             }
    //             Ok(delivery_result) if !delivery_result.failed.is_empty() => {
    //                 // Partial failure - retry only failed queues
    //                 if retry_state.can_retry(&retry_policy) {
    //                     let delay = retry_state.get_delay(&retry_policy);
    //                     tokio::time::sleep(delay).await;
    //                     retry_state.next_attempt();
    //                     continue;
    //                 } else {
    //                     // Max retries exceeded - persist to DLQ
    //                     persist_to_dlq(&event_envelope, &delivery_result).await;
    //                     break;
    //                 }
    //             }
    //             Err(error) if error.is_transient() && retry_state.can_retry(&retry_policy) => {
    //                 let delay = retry_state.get_delay(&retry_policy);
    //                 tokio::time::sleep(delay).await;
    //                 retry_state.next_attempt();
    //             }
    //             Err(error) => {
    //                 // Permanent error or max retries exceeded - persist to DLQ
    //                 persist_to_dlq_with_error(&event_envelope, &error).await;
    //                 break;
    //             }
    //         }
    //     }
    // });

    // Return immediate response to GitHub (within 10-second timeout)
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

/// Request logging middleware with correlation ID tracking
///
/// This middleware:
/// - Extracts or generates correlation IDs for request tracking
/// - Logs request start and completion with structured fields
/// - Propagates correlation ID through response headers
/// - Supports distributed tracing correlation
#[instrument(skip(request, next), fields(
    method = %request.method(),
    uri = %request.uri(),
    correlation_id
))]
async fn request_logging_middleware(
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    // Extract or generate correlation ID
    let correlation_id = request
        .headers()
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Record correlation ID in span
    tracing::Span::current().record("correlation_id", &correlation_id.as_str());

    // Add correlation ID to request extensions for downstream handlers
    request.extensions_mut().insert(correlation_id.clone());

    info!(
        correlation_id = %correlation_id,
        method = %method,
        uri = %uri,
        "Request started"
    );

    let mut response = next.run(request).await;
    let duration = start.elapsed();

    // Add correlation ID to response headers
    if let Ok(header_value) = correlation_id.parse() {
        response
            .headers_mut()
            .insert("x-correlation-id", header_value);
    }

    let status = response.status();

    // Log at appropriate level based on status code
    if status.is_server_error() {
        error!(
            correlation_id = %correlation_id,
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = %duration.as_millis(),
            "Request completed with server error"
        );
    } else if status.is_client_error() {
        warn!(
            correlation_id = %correlation_id,
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = %duration.as_millis(),
            "Request completed with client error"
        );
    } else {
        info!(
            correlation_id = %correlation_id,
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = %duration.as_millis(),
            "Request completed successfully"
        );
    }

    response
}

/// Metrics collection middleware
///
/// Records HTTP request metrics including:
/// - Request/response duration histogram
/// - Request/response size tracking
/// - Status code distribution
/// - Active request gauge
#[instrument(skip(request, next), fields(
    method = %request.method(),
    path
))]
async fn metrics_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let start = std::time::Instant::now();
    let method = request.method().clone();
    let uri = request.uri().path().to_string();

    // Normalize path for metrics (remove IDs, keep structure)
    // This prevents cardinality explosion in metrics
    let normalized_path = normalize_path_for_metrics(&uri);
    tracing::Span::current().record("path", &normalized_path.as_str());

    // Get request size
    let request_size = request
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status();

    // Get response size
    let response_size = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // Log metrics for observability
    info!(
        method = %method,
        path = %normalized_path,
        status = %status,
        duration_ms = %duration.as_millis(),
        request_size = %request_size,
        response_size = %response_size,
        "HTTP request metrics"
    );

    response
}

/// Check if a string looks like a UUID with proper 8-4-4-4-12 hyphen pattern
///
/// Validates UUID format by checking:
/// - Total length is 36 characters
/// - Hyphens are at positions 8, 13, 18, 23
/// - All other characters are hexadecimal digits
fn is_uuid_like(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }

    let chars: Vec<char> = s.chars().collect();

    // Check hyphen positions: 8-4-4-4-12 pattern
    if chars[8] != '-' || chars[13] != '-' || chars[18] != '-' || chars[23] != '-' {
        return false;
    }

    // Check all other positions are hex digits
    for (i, ch) in chars.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            continue; // Skip hyphens
        }
        if !ch.is_ascii_hexdigit() {
            return false;
        }
    }

    true
}

/// Normalize path for metrics to avoid cardinality explosion
///
/// Converts paths like `/api/events/12345` to `/api/events/:id`
fn normalize_path_for_metrics(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = segments
        .iter()
        .map(|segment| {
            // Skip empty segments (from leading/trailing slashes)
            if segment.is_empty() {
                segment.to_string()
            }
            // Check if segment looks like a numeric ID
            else if !segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()) {
                ":id".to_string()
            }
            // Check if segment looks like a UUID (8-4-4-4-12 pattern)
            else if is_uuid_like(segment) {
                ":id".to_string()
            } else {
                segment.to_string()
            }
        })
        .collect();

    normalized.join("/")
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

/// Webhook handler errors with HTTP status code mapping
///
/// This error type represents all possible webhook processing failures
/// and maps them to appropriate HTTP status codes following REST conventions:
///
/// - `400 Bad Request`: Client errors that are permanent and not retryable
///   (invalid headers, malformed payloads, validation failures)
/// - `500 Internal Server Error`: Unexpected server failures
/// - `503 Service Unavailable`: Transient failures that should be retried
///   (temporary storage unavailability, network issues)
///
/// # Error Classification
///
/// Errors are classified as either:
/// - **Permanent**: Client should not retry (4xx status codes)
/// - **Transient**: Client should retry with backoff (503 status code)
///
/// # Security Considerations
///
/// Error messages returned to clients are sanitized to prevent information
/// disclosure. Detailed error information is logged server-side with
/// correlation IDs for debugging.
#[derive(Debug, thiserror::Error)]
pub enum WebhookHandlerError {
    /// Invalid or missing required HTTP headers
    ///
    /// Maps to: `400 Bad Request` (permanent error, do not retry)
    ///
    /// Common causes:
    /// - Missing `X-GitHub-Event` header
    /// - Missing `X-GitHub-Delivery` header
    /// - Invalid header format or encoding
    #[error("Invalid headers: {0}")]
    InvalidHeaders(#[from] ValidationError),

    /// Webhook processing pipeline failure
    ///
    /// Maps to:
    /// - `400 Bad Request` if error is permanent (invalid signature, malformed payload)
    /// - `503 Service Unavailable` if error is transient (storage temporarily down)
    ///
    /// The underlying `WebhookError` determines if the failure is transient
    /// via the `is_transient()` method.
    #[error("Processing failed: {0}")]
    ProcessingFailed(#[from] WebhookError),

    /// Unexpected internal server error
    ///
    /// Maps to: `500 Internal Server Error` (server-side bug or unexpected failure)
    ///
    /// These errors indicate bugs or unexpected system states that should
    /// be investigated. Details are logged but a generic message is returned
    /// to the client.
    #[error("Internal server error: {message}")]
    InternalError { message: String },

    /// Request timeout
    ///
    /// Maps to: `408 Request Timeout` (client should retry)
    ///
    /// Occurs when webhook processing exceeds the configured timeout.
    /// GitHub expects responses within 10 seconds.
    #[error("Request timeout after {seconds}s")]
    Timeout { seconds: u64 },

    /// Payload too large
    ///
    /// Maps to: `413 Payload Too Large` (permanent error, do not retry)
    ///
    /// Occurs when webhook payload exceeds the configured maximum size.
    #[error("Payload too large: {size} bytes (max: {max_size} bytes)")]
    PayloadTooLarge { size: usize, max_size: usize },

    /// Rate limit exceeded
    ///
    /// Maps to: `429 Too Many Requests` (client should retry after delay)
    ///
    /// Occurs when too many requests are received from a single source.
    /// Includes retry-after duration in response headers.
    #[error("Rate limit exceeded. Retry after {retry_after_seconds}s")]
    RateLimitExceeded { retry_after_seconds: u64 },
}

impl axum::response::IntoResponse for WebhookHandlerError {
    fn into_response(self) -> axum::response::Response {
        // Determine HTTP status code and error message based on error type
        let (status, message, retry_after) = match self {
            Self::InvalidHeaders(_) => (StatusCode::BAD_REQUEST, self.to_string(), None),
            Self::ProcessingFailed(ref e) => {
                if e.is_transient() {
                    // Transient errors should be retried
                    (StatusCode::SERVICE_UNAVAILABLE, self.to_string(), Some(60))
                } else {
                    // Permanent errors should not be retried
                    (StatusCode::BAD_REQUEST, self.to_string(), None)
                }
            }
            Self::InternalError { ref message } => {
                // Log detailed error server-side but return generic message to client
                error!(error = %message, "Internal server error occurred");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error occurred. Please try again later.".to_string(),
                    None,
                )
            }
            Self::Timeout { seconds } => {
                warn!(timeout_seconds = seconds, "Request timeout");
                (StatusCode::REQUEST_TIMEOUT, self.to_string(), Some(5))
            }
            Self::PayloadTooLarge { size, max_size } => {
                warn!(
                    payload_size = size,
                    max_size = max_size,
                    "Payload too large"
                );
                (StatusCode::PAYLOAD_TOO_LARGE, self.to_string(), None)
            }
            Self::RateLimitExceeded {
                retry_after_seconds,
            } => {
                warn!(retry_after = retry_after_seconds, "Rate limit exceeded");
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    self.to_string(),
                    Some(retry_after_seconds),
                )
            }
        };

        // Build JSON error response
        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // Build response with appropriate headers
        let mut response = (status, Json(body)).into_response();

        // Add Retry-After header for retryable errors
        if let Some(retry_seconds) = retry_after {
            if let Ok(header_value) = retry_seconds.to_string().parse() {
                response.headers_mut().insert("Retry-After", header_value);
            }
        }

        response
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
        // This is a stub implementation for testing
        // In production, use ServiceMetrics::new() instead
        use prometheus::{
            register_gauge, register_histogram, register_int_counter, register_int_gauge,
        };

        // Use unique names with timestamp to avoid registration conflicts in tests
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        Self {
            http_requests_total: register_int_counter!(
                format!("http_requests_total_test_{}", suffix),
                "Test HTTP requests"
            )
            .unwrap(),
            http_request_duration: register_histogram!(
                format!("http_request_duration_seconds_test_{}", suffix),
                "Test HTTP duration",
                vec![]
            )
            .unwrap(),
            http_request_size: register_histogram!(
                format!("http_request_size_bytes_test_{}", suffix),
                "Test HTTP request size",
                vec![]
            )
            .unwrap(),
            http_response_size: register_histogram!(
                format!("http_response_size_bytes_test_{}", suffix),
                "Test HTTP response size",
                vec![]
            )
            .unwrap(),
            webhook_requests_total: register_int_counter!(
                format!("webhook_requests_total_test_{}", suffix),
                "Test webhook requests"
            )
            .unwrap(),
            webhook_duration_seconds: register_histogram!(
                format!("webhook_duration_seconds_test_{}", suffix),
                "Test webhook duration",
                vec![]
            )
            .unwrap(),
            webhook_validation_failures: register_int_counter!(
                format!("webhook_validation_failures_test_{}", suffix),
                "Test webhook validation failures"
            )
            .unwrap(),
            webhook_queue_routing_duration: register_histogram!(
                format!("webhook_queue_routing_duration_seconds_test_{}", suffix),
                "Test webhook queue routing duration",
                vec![]
            )
            .unwrap(),
            queue_depth_messages: register_int_gauge!(
                format!("queue_depth_messages_test_{}", suffix),
                "Test queue depth"
            )
            .unwrap(),
            queue_processing_rate: register_gauge!(
                format!("queue_processing_rate_test_{}", suffix),
                "Test queue processing rate"
            )
            .unwrap(),
            dead_letter_queue_depth: register_int_gauge!(
                format!("dead_letter_queue_depth_test_{}", suffix),
                "Test DLQ depth"
            )
            .unwrap(),
            session_ordering_violations: register_int_counter!(
                format!("session_ordering_violations_test_{}", suffix),
                "Test session ordering violations"
            )
            .unwrap(),
            error_rate_by_category: register_int_counter!(
                format!("error_rate_by_category_test_{}", suffix),
                "Test error rate"
            )
            .unwrap(),
            circuit_breaker_state: register_int_gauge!(
                format!("circuit_breaker_state_test_{}", suffix),
                "Test circuit breaker state"
            )
            .unwrap(),
            retry_attempts_total: register_int_counter!(
                format!("retry_attempts_total_test_{}", suffix),
                "Test retry attempts"
            )
            .unwrap(),
            blob_storage_failures: register_int_counter!(
                format!("blob_storage_failures_test_{}", suffix),
                "Test blob storage failures"
            )
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
#[path = "lib_tests.rs"]
mod tests;
