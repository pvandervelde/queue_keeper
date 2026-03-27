//! HTTP middleware for security and request handling.
//!
//! Provides:
//! - IP-based authentication failure rate limiting ([`IpFailureTracker`],
//!   [`ip_rate_limit_middleware`]) — spec assertion #19
//! - Admin endpoint authentication ([`admin_auth_middleware`])

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use tracing::warn;

use crate::AppState;

// ============================================================================
// IP-Based Authentication Failure Rate Limiter
// ============================================================================

/// Sliding-window counter of authentication failures per source IP.
///
/// Every call to [`is_blocked`] and [`record_failure`] prunes entries older
/// than `window` before operating, so memory usage is bounded by the number
/// of distinct IPs that have transmitted requests within the window.
///
/// # Spec Reference
///
/// Assertion #19: "Repeated authentication failures from the same IP address
/// MUST trigger rate limiting after 10 failures in 5 minutes."
///
/// [`is_blocked`]: IpFailureTracker::is_blocked
/// [`record_failure`]: IpFailureTracker::record_failure
#[derive(Debug)]
pub struct IpFailureTracker {
    /// Failure timestamps keyed by IP string.
    failures: Mutex<HashMap<String, Vec<Instant>>>,
    /// Duration of the sliding window.
    window: Duration,
    /// Number of failures that triggers blocking.
    max_failures: usize,
}

impl IpFailureTracker {
    /// Create a new tracker with the given failure threshold and time window.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use queue_keeper_api::middleware::IpFailureTracker;
    ///
    /// // Block after 10 failures in 5 minutes (assertion #19)
    /// let tracker = IpFailureTracker::new(10, Duration::from_secs(300));
    /// ```
    pub fn new(max_failures: usize, window: Duration) -> Self {
        Self {
            failures: Mutex::new(HashMap::new()),
            window,
            max_failures,
        }
    }

    /// Return `true` if `ip` has reached or exceeded the failure threshold
    /// within the current sliding window.
    pub fn is_blocked(&self, ip: &str) -> bool {
        let mut map = self.failures.lock().unwrap();
        let now = Instant::now();
        let window = self.window;
        let entry = map.entry(ip.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        entry.len() >= self.max_failures
    }

    /// Record one authentication failure for `ip`.
    pub fn record_failure(&self, ip: &str) {
        let mut map = self.failures.lock().unwrap();
        let now = Instant::now();
        let window = self.window;
        let entry = map.entry(ip.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        entry.push(now);
    }

    /// Return the number of failures recorded within the current window for `ip`.
    ///
    /// Exposed for monitoring and testing.
    pub fn failure_count(&self, ip: &str) -> usize {
        let mut map = self.failures.lock().unwrap();
        let now = Instant::now();
        let window = self.window;
        let entry = map.entry(ip.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        entry.len()
    }
}

// ============================================================================
// Middleware Functions
// ============================================================================

/// IP-based authentication failure rate limiting middleware.
///
/// Before forwarding a request to the inner handler, checks whether the source
/// IP has accumulated enough 401 responses within the configured sliding window.
/// If the failure count has reached the threshold, returns HTTP 429 immediately
/// with a `Retry-After: 300` header.
///
/// After the handler responds, any HTTP 401 is interpreted as an authentication
/// failure and increments the per-IP counter via [`IpFailureTracker::record_failure`].
///
/// The middleware is a transparent pass-through when `AppState::ip_rate_limiter`
/// is `None` (i.e. when [`SecurityConfig::enable_ip_rate_limiting`] is `false`).
///
/// # Spec Reference
///
/// Assertion #19: "Repeated authentication failures from the same IP address
/// MUST trigger rate limiting after 10 failures in 5 minutes."
///
/// [`SecurityConfig::enable_ip_rate_limiting`]: crate::config::SecurityConfig::enable_ip_rate_limiting
pub async fn ip_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let tracker = match &state.ip_rate_limiter {
        Some(t) => Arc::clone(t),
        None => return next.run(request).await,
    };

    let client_ip = extract_client_ip(request.headers());

    if tracker.is_blocked(&client_ip) {
        warn!(
            client_ip = %client_ip,
            "IP rate limited: too many authentication failures"
        );
        return build_too_many_requests_response();
    }

    let response = next.run(request).await;

    if response.status() == StatusCode::UNAUTHORIZED {
        tracker.record_failure(&client_ip);
        warn!(
            client_ip = %client_ip,
            "Authentication failure recorded for IP rate limiter"
        );
    }

    response
}

/// Admin endpoint authentication middleware.
///
/// When [`AppState::admin_api_key`] is `Some`, every request must carry a
/// matching `Authorization: Bearer <key>` header. Requests that are absent or
/// carry an incorrect key receive HTTP 401 without reaching the handler.
///
/// When `admin_api_key` is `None`, the middleware is a transparent pass-through
/// so that deployments without an explicit admin key remain accessible.
///
/// The key comparison uses constant-time equality to prevent timing side-channels.
///
/// [`AppState::admin_api_key`]: crate::AppState::admin_api_key
pub async fn admin_auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let expected = match &state.admin_api_key {
        Some(k) => k.clone(),
        None => return next.run(request).await,
    };

    match extract_bearer_token(request.headers()) {
        Some(provided) if constant_time_eq(provided.as_bytes(), expected.as_bytes()) => {
            next.run(request).await
        }
        _ => build_admin_unauthorized_response(),
    }
}

// ============================================================================
// Private Helpers
// ============================================================================

/// Extract the client IP from proxy headers, falling back to `"unknown"`.
///
/// Priority:
/// 1. First (leftmost, original client) IP in `X-Forwarded-For`
/// 2. `X-Real-IP`
/// 3. `"unknown"`
pub fn extract_client_ip(headers: &HeaderMap) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let ip = first.trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let ip = real_ip.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }

    "unknown".to_string()
}

/// Extract the bearer token from `Authorization: Bearer <token>`.
fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    auth.strip_prefix("Bearer ").map(|t| t.to_string())
}

/// Constant-time byte slice comparison to mitigate timing attacks.
///
/// Returns `true` only when both slices have equal length and identical bytes.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn build_too_many_requests_response() -> Response {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("content-type", "application/json")
        .header("retry-after", "300")
        .body(Body::from(
            r#"{"error":"Too many authentication failures","retry_after_seconds":300}"#,
        ))
        .unwrap()
}

fn build_admin_unauthorized_response() -> Response {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("content-type", "application/json")
        .header("www-authenticate", r#"Bearer realm="admin""#)
        .body(Body::from(
            r#"{"error":"Authentication required","message":"Provide a valid admin API key in the Authorization: Bearer header"}"#,
        ))
        .unwrap()
}

#[cfg(test)]
#[path = "middleware_tests.rs"]
mod tests;
