//! HTTP middleware for security and request handling.
//!
//! Provides:
//! - IP-based authentication failure rate limiting with three-tier escalation
//!   ([`IpFailureTracker`], [`IpTier`], [`ip_rate_limit_middleware`]) — spec
//!   assertion #19 and `specs/security/rate-limiting.md` §"Security Response
//!   Escalation"
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
use tracing::{info, warn};

use crate::AppState;

// ============================================================================
// IP Escalation Tier
// ============================================================================

/// The current access tier for a source IP address.
///
/// Tier transitions are triggered by authentication failure counts:
///
/// | Failures (5 min window) | Tier | Duration |
/// |-------------------------|------|----------|
/// | < 10 | [`Normal`] | — |
/// | 10 – 50 | [`RateRestricted`] | 1 hour |
/// | > 50 | [`Blocked`] | 24 hours |
///
/// Once an IP enters [`RateRestricted`] or [`Blocked`], the tier is held for
/// the fixed duration regardless of subsequent sliding-window counts. After
/// the duration elapses the IP returns to [`Normal`].  Escalation upward
/// (from [`RateRestricted`] to [`Blocked`]) can happen before the lower tier
/// expires.
///
/// # Spec Reference
///
/// `specs/security/rate-limiting.md` §"Security Response Escalation"
///
/// [`Normal`]: IpTier::Normal
/// [`RateRestricted`]: IpTier::RateRestricted
/// [`Blocked`]: IpTier::Blocked
#[derive(Debug, Clone)]
pub enum IpTier {
    /// Fewer than 10 failures in the 5-minute window — requests pass through.
    Normal,
    /// 10–50 failures — requests are rejected with HTTP 429 for 1 hour.
    RateRestricted {
        /// Monotonic instant at which this restriction expires.
        until: Instant,
    },
    /// More than 50 failures — requests are rejected with HTTP 429 for 24 hours.
    Blocked {
        /// Monotonic instant at which this block expires.
        until: Instant,
    },
}

impl IpTier {
    /// Returns `true` when the tier causes requests to be rejected.
    pub fn is_restricted(&self) -> bool {
        !matches!(self, Self::Normal)
    }

    /// Seconds the caller should wait before retrying based on the **remaining**
    /// penalty time, as required by RFC 7231 §7.1.3.
    ///
    /// Returns `0` for [`Normal`] (no waiting required) or when the tier has
    /// already expired (defensive — `check_tier` auto-expires before returning).
    ///
    /// [`Normal`]: IpTier::Normal
    pub fn retry_after_secs(&self) -> u64 {
        match self {
            Self::Normal => 0,
            Self::RateRestricted { until } | Self::Blocked { until } => {
                until.saturating_duration_since(Instant::now()).as_secs()
            }
        }
    }

    /// Returns `true` when this tier has a fixed-duration expiry that has
    /// already passed relative to `now`.
    pub(crate) fn is_expired(&self, now: Instant) -> bool {
        match self {
            Self::Normal => false,
            Self::RateRestricted { until } | Self::Blocked { until } => now >= *until,
        }
    }
}

// ============================================================================
// Manual PartialEq / Eq for IpTier
// ============================================================================

/// Compare only the variant, ignoring the `until` timestamp.
///
/// Two `RateRestricted` or two `Blocked` values are equal regardless of when
/// their penalty expires. This prevents a foot-gun where two independently
/// created tiers with identical semantics but different expiry `Instant`s
/// compare as unequal.
impl PartialEq for IpTier {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Normal, Self::Normal)
                | (Self::RateRestricted { .. }, Self::RateRestricted { .. })
                | (Self::Blocked { .. }, Self::Blocked { .. })
        )
    }
}

impl Eq for IpTier {}

// ============================================================================
// Per-IP State (private)
// ============================================================================

/// Internal state held per source IP address.
#[derive(Debug)]
struct IpState {
    /// Timestamps of authentication failures within the sliding window.
    failures: Vec<Instant>,
    /// Current escalation tier (may have a fixed-duration expiry).
    tier: IpTier,
}

impl IpState {
    fn new() -> Self {
        Self {
            failures: Vec::new(),
            tier: IpTier::Normal,
        }
    }
}

// ============================================================================
// IP-Based Authentication Failure Rate Limiter
// ============================================================================

/// Three-tier IP authentication failure tracker.
///
/// Counts authentication failures per source IP within a sliding window and
/// escalates the IP through three restriction tiers based on cumulative failure
/// counts:
///
/// 1. **Normal** — fewer than `rate_restrict_threshold` failures: allow all
///    requests.
/// 2. **RateRestricted** — `rate_restrict_threshold`–`block_threshold`
///    failures: reject with HTTP 429 for `rate_restrict_duration`.
/// 3. **Blocked** — more than `block_threshold` failures: reject with HTTP 429
///    for `block_duration`.
///
/// Tier upgrades happen immediately when the failure count crosses a threshold.
/// Tier downgrades happen only when the fixed-duration penalty expires; the IP
/// then returns to **Normal** regardless of stale window entries.
///
/// # Thread Safety
///
/// All state is locked behind a `Mutex`; the tracker is `Send + Sync` and can
/// be wrapped in `Arc` for shared ownership across Axum handler tasks.
///
/// # Spec Reference
///
/// `specs/security/rate-limiting.md` §"Security Response Escalation";
/// spec assertion #19.
#[derive(Debug)]
pub struct IpFailureTracker {
    /// Per-IP state: failure timestamps and current tier.
    states: Mutex<HashMap<String, IpState>>,
    /// Duration of the sliding failure-counting window.
    window: Duration,
    /// Failure count that triggers the [`IpTier::RateRestricted`] tier.
    rate_restrict_threshold: usize,
    /// Failure count that triggers the [`IpTier::Blocked`] tier.
    block_threshold: usize,
    /// How long an IP stays in the [`IpTier::RateRestricted`] tier.
    rate_restrict_duration: Duration,
    /// How long an IP stays in the [`IpTier::Blocked`] tier.
    block_duration: Duration,
}

impl IpFailureTracker {
    /// Create a tracker with fully specified thresholds and durations.
    ///
    /// # Parameters
    ///
    /// - `rate_restrict_threshold` — failure count (within `window`) that
    ///   triggers the [`IpTier::RateRestricted`] tier. Spec default: 10.
    /// - `block_threshold` — failure count that triggers the
    ///   [`IpTier::Blocked`] tier. Spec default: 50.
    /// - `window` — sliding window for failure counting. Spec default: 5 min.
    /// - `rate_restrict_duration` — how long an IP is rate-restricted. Spec
    ///   default: 1 hour.
    /// - `block_duration` — how long an IP is completely blocked. Spec
    ///   default: 24 hours.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use queue_keeper_api::middleware::IpFailureTracker;
    ///
    /// // Spec-specified defaults
    /// let tracker = IpFailureTracker::new(
    ///     10,
    ///     50,
    ///     Duration::from_secs(300),
    ///     Duration::from_secs(3_600),
    ///     Duration::from_secs(86_400),
    /// );
    /// ```
    pub fn new(
        rate_restrict_threshold: usize,
        block_threshold: usize,
        window: Duration,
        rate_restrict_duration: Duration,
        block_duration: Duration,
    ) -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            window,
            rate_restrict_threshold,
            block_threshold,
            rate_restrict_duration,
            block_duration,
        }
    }

    /// Threshold at which an IP enters the [`IpTier::RateRestricted`] tier.
    pub fn rate_restrict_threshold(&self) -> usize {
        self.rate_restrict_threshold
    }

    /// Threshold at which an IP enters the [`IpTier::Blocked`] tier.
    pub fn block_threshold(&self) -> usize {
        self.block_threshold
    }

    /// Duration of the sliding failure-counting window.
    pub fn window(&self) -> Duration {
        self.window
    }

    /// How long an IP stays in the [`IpTier::RateRestricted`] tier.
    pub fn rate_restrict_duration(&self) -> Duration {
        self.rate_restrict_duration
    }

    /// How long an IP stays in the [`IpTier::Blocked`] tier.
    pub fn block_duration(&self) -> Duration {
        self.block_duration
    }

    /// Return the current [`IpTier`] for `ip`.
    ///
    /// If a timed tier ([`IpTier::RateRestricted`] or [`IpTier::Blocked`]) has
    /// expired, the IP is automatically reset to [`IpTier::Normal`] and its
    /// failure history is pruned.
    pub fn check_tier(&self, ip: &str) -> IpTier {
        let mut states = self.states.lock().unwrap();
        let now = Instant::now();
        let window = self.window;

        if let Some(state) = states.get_mut(ip) {
            // Expire timed tiers that have passed their deadline.
            if state.tier.is_expired(now) {
                state.tier = IpTier::Normal;
                state.failures.retain(|t| now.duration_since(*t) < window);
                if state.failures.is_empty() {
                    states.remove(ip);
                    return IpTier::Normal;
                }
            }
            state.tier.clone()
        } else {
            IpTier::Normal
        }
    }

    /// Return `true` if `ip` is currently in a restricted tier.
    ///
    /// Convenience wrapper around [`check_tier`].
    ///
    /// [`check_tier`]: IpFailureTracker::check_tier
    pub fn is_blocked(&self, ip: &str) -> bool {
        self.check_tier(ip).is_restricted()
    }

    /// Record one authentication failure for `ip` and escalate its tier if
    /// the new failure count crosses a threshold.
    ///
    /// Escalation rules (evaluated after appending the new failure):
    ///
    /// - Count > `block_threshold` AND tier is not already [`IpTier::Blocked`]
    ///   → upgrade to **Blocked** for `block_duration`, overwriting
    ///   [`IpTier::RateRestricted`] if present.
    /// - Count ≥ `rate_restrict_threshold` AND tier is [`IpTier::Normal`]
    ///   → upgrade to **RateRestricted** for `rate_restrict_duration`.
    ///
    /// The tier is never downgraded by this method; downgrading happens only
    /// when a timed tier expires (see [`check_tier`]).
    ///
    /// Returns the new [`IpTier`] for `ip` after recording the failure, so
    /// callers do not need a second lock acquisition to read the updated state.
    ///
    /// After modifying the target IP's state this method also sweeps expired
    /// entries from the map to prevent unbounded memory growth under sustained
    /// attacks from many distinct source addresses.
    ///
    /// [`check_tier`]: IpFailureTracker::check_tier
    pub fn record_failure(&self, ip: &str) -> IpTier {
        let mut states = self.states.lock().unwrap();
        let now = Instant::now();
        let window = self.window;

        // Update the target IP's state and capture the new tier.
        let new_tier = {
            let state = states.entry(ip.to_string()).or_insert_with(IpState::new);

            // Prune failures outside the sliding window.
            state.failures.retain(|t| now.duration_since(*t) < window);
            state.failures.push(now);

            let count = state.failures.len();

            // Expire the current tier if its deadline has passed before evaluating
            // whether to escalate, so we don't skip transitions on expiry.
            if state.tier.is_expired(now) {
                state.tier = IpTier::Normal;
            }

            // Escalate based on the new failure count.
            if count > self.block_threshold && !matches!(state.tier, IpTier::Blocked { .. }) {
                state.tier = IpTier::Blocked {
                    until: now + self.block_duration,
                };
            } else if count >= self.rate_restrict_threshold && matches!(state.tier, IpTier::Normal)
            {
                state.tier = IpTier::RateRestricted {
                    until: now + self.rate_restrict_duration,
                };
            }

            state.tier.clone()
        };

        // Sweep all entries to evict those whose tier has expired and whose
        // sliding window is empty, preventing unbounded HashMap growth under
        // attacks from many distinct source IPs.
        states.retain(|_, s| {
            if s.tier.is_expired(now) {
                s.tier = IpTier::Normal;
                s.failures.retain(|t| now.duration_since(*t) < window);
            }
            !s.failures.is_empty() || s.tier.is_restricted()
        });

        new_tier
    }

    /// Return the number of failures recorded within the current sliding
    /// window for `ip`.
    ///
    /// This reflects only the raw sliding-window count and does not consider
    /// active timed tiers. Exposed for monitoring and testing.
    pub fn failure_count(&self, ip: &str) -> usize {
        let mut states = self.states.lock().unwrap();
        let now = Instant::now();
        let window = self.window;

        if let Some(state) = states.get_mut(ip) {
            state.failures.retain(|t| now.duration_since(*t) < window);
            if state.failures.is_empty() && matches!(state.tier, IpTier::Normal) {
                states.remove(ip);
                return 0;
            }
            state.failures.len()
        } else {
            0
        }
    }
}

// ============================================================================
// Middleware Functions
// ============================================================================

/// IP-based authentication failure rate limiting middleware.
///
/// Implements the three-tier progressive response defined in
/// `specs/security/rate-limiting.md` §"Security Response Escalation":
///
/// | Tier | Condition | HTTP Response |
/// |------|-----------|---------------|
/// | Normal | < 10 failures | Pass through |
/// | RateRestricted | 10–50 failures | 429, `Retry-After: 3600` |
/// | Blocked | > 50 failures | 429, `Retry-After: 86400` |
///
/// Before forwarding a request, the middleware resolves the source IP's current
/// [`IpTier`]. If restricted, it returns HTTP 429 immediately.  After the
/// handler responds, any HTTP 401 is recorded as an authentication failure via
/// [`IpFailureTracker::record_failure`], which may escalate the IP's tier.
///
/// The middleware is a transparent pass-through when `AppState::ip_rate_limiter`
/// is `None` (i.e. when [`SecurityConfig::enable_ip_rate_limiting`] is `false`).
///
/// # Spec Reference
///
/// Spec assertion #19; `specs/security/rate-limiting.md` §"Security Response
/// Escalation".
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
    let tier = tracker.check_tier(&client_ip);

    if tier.is_restricted() {
        let retry_after = tier.retry_after_secs();
        warn!(
            client_ip = %client_ip,
            tier = ?tier,
            retry_after_secs = retry_after,
            "IP rate limited: too many authentication failures"
        );
        return build_too_many_requests_response(retry_after);
    }

    let response = next.run(request).await;

    if response.status() == StatusCode::UNAUTHORIZED {
        // record_failure returns the new tier in the same lock acquisition,
        // eliminating the need for a separate check_tier call.
        match tracker.record_failure(&client_ip) {
            IpTier::Normal => {
                info!(
                    client_ip = %client_ip,
                    "Authentication failure recorded for IP"
                );
            }
            IpTier::RateRestricted { .. } => {
                warn!(
                    client_ip = %client_ip,
                    "IP escalated to RateRestricted tier after authentication failure"
                );
            }
            IpTier::Blocked { .. } => {
                warn!(
                    client_ip = %client_ip,
                    "IP escalated to Blocked tier after authentication failure"
                );
            }
        }
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
///
/// # Security
///
/// These headers are **fully controlled by the sender** and can be trivially
/// spoofed unless the service is deployed behind a reverse proxy that strips
/// and re-appends them. Ensure a trusted proxy (e.g. an ingress controller or
/// load balancer) is the only entity that can set `X-Forwarded-For` and
/// `X-Real-IP` before this service receives the request.
pub fn extract_client_ip(headers: &HeaderMap) -> String {
    // 45 chars is the maximum length of a valid IP address string
    // (IPv6 mapped IPv4, e.g. "0000:0000:0000:0000:0000:0000:255.255.255.255").
    // Values longer than this are not valid IPs and must not be used as map
    // keys to prevent adversarially large strings from bloating the tracker.
    const MAX_IP_LEN: usize = 45;

    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let ip = first.trim();
            if !ip.is_empty() && ip.len() <= MAX_IP_LEN {
                return ip.to_string();
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let ip = real_ip.trim();
        if !ip.is_empty() && ip.len() <= MAX_IP_LEN {
            return ip.to_string();
        }
    }

    "unknown".to_string()
}

/// Extract the bearer token from `Authorization: Bearer <token>`.
///
/// # Note
///
/// The scheme prefix (`Bearer `) comparison is **case-sensitive**. A header
/// such as `Authorization: bearer <token>` (lowercase) will not be recognised.
/// Clients must use the canonical capitalisation as defined in RFC 6750.
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

fn build_too_many_requests_response(retry_after_secs: u64) -> Response {
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("content-type", "application/json")
        .header("retry-after", retry_after_secs.to_string())
        .header("x-ratelimit-remaining", "0")
        .body(Body::from(format!(
            r#"{{"error":"Too many authentication failures","retry_after_seconds":{}}}"#,
            retry_after_secs
        )))
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
