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
pub mod azure_config;
pub mod config;
pub mod dlq_storage;
pub mod errors;
pub mod metrics;
pub mod provider_registry;
pub mod queue_delivery;
pub mod responses;
pub mod retry;

// Private modules (not yet fully extracted)
// mod handlers;
// mod metrics;
// mod middleware;
// mod responses;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{Json, Response},
    routing::{get, post, put},
    Router,
};
use bytes::Bytes;
use prometheus::TextEncoder;
use queue_keeper_core::{
    monitoring::MetricsCollector,
    webhook::{WebhookHeaders, WebhookRequest},
    EventId, SessionId, Timestamp,
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, instrument, warn};

// Re-export public types
pub use azure_config::{
    AzureBlobStorageConfig, AzureConfigError, AzureKeyVaultConfig, AzureProductionConfig,
    AzureServiceBusConfig, AzureTelemetryConfig,
};
pub use config::{
    LoggingConfig, ProviderConfig, ProviderSecretConfig, SecurityConfig, ServerConfig,
    ServiceConfig, WebhookConfig,
};
pub use errors::{ConfigError, ServiceError, WebhookHandlerError};
pub use metrics::{ServiceMetrics, TelemetryConfig};
pub use provider_registry::{InvalidProviderIdError, ProviderId, ProviderRegistry};
pub use responses::*;

// ============================================================================
// Application State
// ============================================================================

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Configuration for the service
    pub config: ServiceConfig,

    /// Registry of provider-specific webhook processors
    pub provider_registry: Arc<ProviderRegistry>,

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
        provider_registry: Arc<ProviderRegistry>,
        health_checker: Arc<dyn HealthChecker>,
        event_store: Arc<dyn EventStore>,
        metrics: Arc<ServiceMetrics>,
        telemetry_config: Arc<TelemetryConfig>,
    ) -> Self {
        Self {
            config,
            provider_registry,
            health_checker,
            event_store,
            metrics,
            telemetry_config,
        }
    }
}

// ============================================================================
// HTTP Server
// ============================================================================

/// Create HTTP router with all endpoints
pub fn create_router(state: AppState) -> Router {
    let webhook_routes = Router::new()
        .route("/webhook/{provider}", post(handle_provider_webhook));

    let health_routes = Router::new()
        .route("/health", get(handle_health_check))
        .route("/health/deep", get(handle_deep_health_check))
        .route("/health/live", get(handle_liveness_check))
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
    provider_registry: ProviderRegistry,
    health_checker: Arc<dyn HealthChecker>,
    event_store: Arc<dyn EventStore>,
) -> Result<(), ServiceError> {
    // Validate configuration before initializing any infrastructure
    config.validate().map_err(ServiceError::Configuration)?;

    // Warn when literal secrets are present — they should only be used in
    // development or testing, never in production deployments.
    for provider in &config.providers {
        if let Some(config::ProviderSecretConfig::Literal { .. }) = &provider.secret {
            warn!(
                provider = %provider.id,
                "Provider is configured with a literal webhook secret. \
                 Literal secrets are for development and testing only. \
                 Use a Key Vault secret source for production deployments."
            );
        }
    }

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
        Arc::new(provider_registry),
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

/// Handle a webhook for a specific provider.
///
/// Routes `POST /webhook/{provider}` to the processor registered under that
/// provider name. Returns `404 Not Found` when the provider is unknown.
///
/// # Request Flow
///
/// 1. Extract provider name from the URL path.
/// 2. Look it up in the [`ProviderRegistry`]; return 404 if absent.
/// 3. Parse provider-agnostic webhook headers.
/// 4. Delegate to the provider's [`WebhookProcessor::process_webhook`].
/// 5. Return `200 OK` with [`WebhookResponse`] on success.
///
/// # Errors
///
/// - [`WebhookHandlerError::ProviderNotFound`] when the provider is not registered.
/// - [`WebhookHandlerError::InvalidHeaders`] when required headers are missing or malformed.
/// - [`WebhookHandlerError::ProcessingFailed`] when the processor pipeline fails.
#[instrument(skip(state, headers, body), fields(provider = %provider))]
pub async fn handle_provider_webhook(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>, WebhookHandlerError> {
    info!(provider = %provider, "Received webhook request");

    // Resolve provider – return 404 for unknown providers before any further work
    let processor = state
        .provider_registry
        .get(&provider)
        .ok_or_else(|| WebhookHandlerError::ProviderNotFound {
            provider: provider.clone(),
        })?;

    // Start timing for metrics
    let start = std::time::Instant::now();

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
    let webhook_headers = match WebhookHeaders::from_http_headers(&header_map) {
        Ok(h) => h,
        Err(e) => {
            let duration = start.elapsed();
            state.metrics.record_webhook_request(duration, false);
            state.metrics.record_webhook_validation_failure();
            return Err(WebhookHandlerError::InvalidHeaders(e));
        }
    };

    // Enforce per-provider allowed_event_types if configured.
    // An empty list means all event types are accepted.
    //
    // Note: require_signature enforcement is delegated to the processor's
    // SignatureValidator. When a SignatureValidator is wired into the
    // DefaultWebhookProcessor it will reject requests with an invalid or
    // missing signature regardless of the ProviderConfig setting.
    let provider_config = state.config.providers.iter().find(|p| p.id == provider);
    if let Some(pc) = provider_config {
        if !pc.allowed_event_types.is_empty()
            && !pc.allowed_event_types.contains(&webhook_headers.event_type)
        {
            let duration = start.elapsed();
            state.metrics.record_webhook_request(duration, false);
            state.metrics.record_webhook_validation_failure();
            return Err(WebhookHandlerError::InvalidHeaders(
                queue_keeper_core::ValidationError::InvalidFormat {
                    // Use a provider-neutral field name so non-GitHub providers
                    // receive a sensible error rather than a GitHub header name.
                    field: "event-type".to_string(),
                    message: format!(
                        "event type '{}' is not in the allowed list for provider '{}'",
                        webhook_headers.event_type, provider
                    ),
                },
            ));
        }
    }

    // Create webhook request
    let webhook_request = WebhookRequest::new(webhook_headers, body);

    // Delegate to the provider-specific processor
    let event_envelope = match processor.process_webhook(webhook_request).await {
        Ok(envelope) => envelope,
        Err(e) => {
            let duration = start.elapsed();
            state.metrics.record_webhook_request(duration, false);
            return Err(WebhookHandlerError::ProcessingFailed(e));
        }
    };

    info!(
        event_id = %event_envelope.event_id,
        event_type = %event_envelope.event_type,
        repository = %event_envelope.repository.full_name,
        session_id = %event_envelope.session_id,
        provider = %provider,
        "Successfully processed webhook - returning immediate response"
    );

    let duration = start.elapsed();
    state.metrics.record_webhook_request(duration, true);

    Ok(Json(WebhookResponse {
        event_id: event_envelope.event_id,
        session_id: event_envelope.session_id,
        status: "processed".to_string(),
        message: "Webhook processed successfully".to_string(),
    }))
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

/// Liveness check endpoint (for Kubernetes)
#[instrument(skip(_state))]
async fn handle_liveness_check(State(_state): State<AppState>) -> Json<HealthResponse> {
    // Liveness check is simpler than readiness - just verify the process is alive
    // If we can respond, we're alive (unlike readiness which checks dependencies)
    Json(HealthResponse {
        status: "alive".to_string(),
        timestamp: Timestamp::now(),
        checks: HashMap::new(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// ============================================================================
// API Handlers (Stubs)
// ============================================================================

/// List recent events
#[instrument(skip(state))]
async fn list_events(
    State(state): State<AppState>,
    Query(params): Query<EventListParams>,
) -> Result<Json<EventListResponse>, StatusCode> {
    match state.event_store.list_events(params).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!(error = %e, "Failed to list events");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get specific event details
#[instrument(skip(state))]
async fn get_event(
    State(state): State<AppState>,
    Path(event_id_str): Path<String>,
) -> Result<Json<EventDetailResponse>, StatusCode> {
    // Parse event ID from ULID string
    let event_id: EventId = match event_id_str.parse() {
        Ok(id) => id,
        Err(e) => {
            warn!(error = %e, "Invalid event ID format");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    match state.event_store.get_event(&event_id).await {
        Ok(envelope) => Ok(Json(EventDetailResponse { event: envelope })),
        Err(e) => {
            error!(error = %e, event_id = %event_id, "Failed to get event");
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// List active sessions
#[instrument(skip(state))]
async fn list_sessions(
    State(state): State<AppState>,
    Query(params): Query<SessionListParams>,
) -> Result<Json<SessionListResponse>, StatusCode> {
    match state.event_store.list_sessions(params).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!(error = %e, "Failed to list sessions");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get specific session details
#[instrument(skip(state))]
async fn get_session(
    State(state): State<AppState>,
    Path(session_id_str): Path<String>,
) -> Result<Json<SessionDetailResponse>, StatusCode> {
    // Parse session ID - it's a string in owner/repo/type/id format
    let session_id = match SessionId::new(session_id_str.clone()) {
        Ok(id) => id,
        Err(e) => {
            warn!(error = %e, "Invalid session ID format");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    match state.event_store.get_session(&session_id).await {
        Ok(details) => Ok(Json(SessionDetailResponse { session: details })),
        Err(e) => {
            error!(error = %e, session_id = %session_id, "Failed to get session");
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// Get system statistics
#[instrument(skip(state))]
async fn get_statistics(
    State(state): State<AppState>,
) -> Result<Json<StatisticsResponse>, StatusCode> {
    match state.event_store.get_statistics().await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!(error = %e, "Failed to get statistics");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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
    tracing::Span::current().record("correlation_id", correlation_id.as_str());

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
    tracing::Span::current().record("path", normalized_path.as_str());

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
            // Check if segment looks like a numeric ID or UUID (8-4-4-4-12 pattern)
            else if (!segment.is_empty() && segment.chars().all(|c| c.is_ascii_digit()))
                || is_uuid_like(segment)
            {
                ":id".to_string()
            } else {
                segment.to_string()
            }
        })
        .collect();

    normalized.join("/")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
