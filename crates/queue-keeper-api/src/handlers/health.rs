//! Health check handlers.
//!
//! Implements the Kubernetes-compatible health probe endpoints:
//!
//! | Path | Purpose | Kubernetes probe |
//! |------|---------|-----------------|
//! | `/health` | Basic health check | — |
//! | `/health/deep` | Dependency health check | — |
//! | `/health/live` | Liveness probe | `livenessProbe` |
//! | `/ready` | Readiness probe | `readinessProbe` |

use crate::{
    responses::{HealthResponse, ReadinessResponse},
    AppState,
};
use axum::{extract::State, http::StatusCode, response::Json};
use queue_keeper_core::Timestamp;
use std::collections::HashMap;
use tracing::instrument;

/// Basic health check endpoint (`GET /health`).
///
/// Returns HTTP 200 with `{"status": "healthy"}` when the service is healthy,
/// or HTTP 503 with `{"status": "unhealthy"}` otherwise.
#[instrument(skip(state))]
pub async fn handle_health_check(
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

/// Deep health check with dependency validation (`GET /health/deep`).
///
/// Performs dependency checks in addition to the basic service check.
/// Returns HTTP 503 when any dependency is unavailable.
#[instrument(skip(state))]
pub async fn handle_deep_health_check(
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

/// Readiness check for Kubernetes (`GET /ready`).
///
/// Returns HTTP 200 when the service is ready to accept traffic — at least one
/// webhook provider is registered and required dependencies are initialised.
/// Returns HTTP 503 when the service is not yet ready (e.g. configuration
/// still loading or no providers configured).
#[instrument(skip(state))]
pub async fn handle_readiness_check(
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

/// Liveness check for Kubernetes (`GET /health/live`).
///
/// Always returns HTTP 200 with `{"status": "alive"}` as long as the process
/// can respond to HTTP requests. Unlike readiness, liveness does not check
/// downstream dependencies — a responsive process is considered alive.
#[instrument(skip(_state))]
pub async fn handle_liveness_check(State(_state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "alive".to_string(),
        timestamp: Timestamp::now(),
        checks: HashMap::new(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
