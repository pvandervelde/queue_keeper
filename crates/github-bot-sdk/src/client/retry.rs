// GENERATED FROM: github-bot-sdk-specs/interfaces/rate-limiting-retry.md
// Rate limiting and retry policy for GitHub API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Rate limit information from GitHub API.
///
/// GitHub returns rate limit info in response headers:
/// - X-RateLimit-Limit
/// - X-RateLimit-Remaining
/// - X-RateLimit-Reset (Unix timestamp)
///
/// See github-bot-sdk-specs/interfaces/rate-limiting-retry.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    /// Maximum number of requests allowed
    pub limit: u64,

    /// Number of requests remaining
    pub remaining: u64,

    /// Time when the rate limit resets
    pub reset_at: DateTime<Utc>,

    /// Whether currently rate limited
    pub is_limited: bool,
}

impl RateLimitInfo {
    /// Create rate limit info from response headers.
    ///
    /// See github-bot-sdk-specs/interfaces/rate-limiting-retry.md
    pub fn from_headers(
        limit: Option<&str>,
        remaining: Option<&str>,
        reset: Option<&str>,
    ) -> Option<Self> {
        let limit = limit?.parse::<u64>().ok()?;
        let remaining = remaining?.parse::<u64>().ok()?;
        let reset_timestamp = reset?.parse::<i64>().ok()?;

        let reset_at = DateTime::from_timestamp(reset_timestamp, 0)?;
        let is_limited = remaining == 0;

        Some(RateLimitInfo {
            limit,
            remaining,
            reset_at,
            is_limited,
        })
    }

    /// Check if we're approaching the rate limit.
    ///
    /// Returns true if remaining requests are below the threshold.
    pub fn is_near_limit(&self, threshold_pct: f64) -> bool {
        let threshold = (self.limit as f64 * threshold_pct) as u64;
        self.remaining < threshold
    }

    /// Get time until rate limit reset.
    pub fn time_until_reset(&self) -> Duration {
        let now = Utc::now();
        if self.reset_at > now {
            Duration::from_secs((self.reset_at - now).num_seconds() as u64)
        } else {
            Duration::from_secs(0)
        }
    }
}

/// Retry policy for transient errors.
///
/// Controls exponential backoff retry behavior.
///
/// See github-bot-sdk-specs/interfaces/rate-limiting-retry.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_retries: u32,

    /// Initial delay before first retry
    pub initial_delay: Duration,

    /// Maximum delay between retries
    pub max_delay: Duration,

    /// Backoff multiplier (e.g., 2.0 for doubling)
    pub backoff_multiplier: f64,

    /// Whether to add jitter to delays
    pub use_jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            use_jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with custom settings.
    pub fn new(max_retries: u32, initial_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            initial_delay,
            max_delay,
            backoff_multiplier: 2.0,
            use_jitter: true,
        }
    }

    /// Enable jitter (random variation) in retry delays.
    ///
    /// Jitter helps prevent thundering herd problems when multiple clients
    /// retry simultaneously. Adds ±25% randomization to calculated delays.
    ///
    /// # Examples
    ///
    /// ```
    /// use github_bot_sdk::client::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default().with_jitter();
    /// ```
    pub fn with_jitter(mut self) -> Self {
        self.use_jitter = true;
        self
    }

    /// Disable jitter (no random variation) in retry delays.
    ///
    /// Use this for deterministic testing or when precise timing is required.
    ///
    /// # Examples
    ///
    /// ```
    /// use github_bot_sdk::client::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default().without_jitter();
    /// ```
    pub fn without_jitter(mut self) -> Self {
        self.use_jitter = false;
        self
    }

    /// Calculate delay for a specific retry attempt.
    ///
    /// Uses exponential backoff with optional jitter.
    ///
    /// # Jitter
    ///
    /// When jitter is enabled (default), applies ±25% randomization to prevent
    /// thundering herd problems. For example, a 1000ms delay becomes 750-1250ms.
    ///
    /// # Examples
    ///
    /// ```
    /// use github_bot_sdk::client::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default();
    /// let delay = policy.calculate_delay(1);
    /// // First retry: ~100ms ±25%
    /// ```
    ///
    /// See github-bot-sdk-specs/interfaces/rate-limiting-retry.md
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_secs(0);
        }

        // Calculate exponential backoff
        let multiplier = self.backoff_multiplier.powi(attempt as i32 - 1);
        let delay_ms = (self.initial_delay.as_millis() as f64 * multiplier) as u64;
        let mut delay = Duration::from_millis(delay_ms);

        // Cap at max delay
        if delay > self.max_delay {
            delay = self.max_delay;
        }

        // Add jitter if enabled (±25% randomization)
        if self.use_jitter {
            use rand::Rng;
            let jitter_factor = rand::rng().random_range(0.75..=1.25);
            delay = Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64);
        }

        delay
    }

    /// Check if another retry attempt should be made.
    ///
    /// # Arguments
    ///
    /// * `attempt` - Current attempt number (0-indexed)
    ///
    /// # Returns
    ///
    /// `true` if attempt is below max_retries, `false` otherwise.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

/// Parse Retry-After header from HTTP response.
///
/// The Retry-After header can be in two formats:
/// - Delta-seconds: "60" (integer number of seconds)
/// - HTTP-date: "Wed, 21 Oct 2015 07:28:00 GMT" (RFC 7231 format)
///
/// # Arguments
///
/// * `retry_after` - The Retry-After header value
///
/// # Returns
///
/// `Some(Duration)` if the header is valid, `None` otherwise.
///
/// # Examples
///
/// ```
/// use github_bot_sdk::client::parse_retry_after;
/// use std::time::Duration;
///
/// // Delta-seconds format
/// let delay = parse_retry_after("60");
/// assert_eq!(delay, Some(Duration::from_secs(60)));
///
/// // Invalid format
/// let delay = parse_retry_after("invalid");
/// assert_eq!(delay, None);
/// ```
///
/// See github-bot-sdk-specs/interfaces/rate-limiting-retry.md
pub fn parse_retry_after(retry_after: &str) -> Option<Duration> {
    // Try parsing as delta-seconds first (most common for GitHub)
    if let Ok(seconds) = retry_after.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    // Try parsing as HTTP-date (RFC 7231 format)
    // Example: "Wed, 21 Oct 2015 07:28:00 GMT"
    if let Ok(date_time) = chrono::DateTime::parse_from_rfc2822(retry_after) {
        let now = Utc::now();
        let retry_time = date_time.with_timezone(&Utc);

        if retry_time > now {
            let duration = (retry_time - now).num_seconds();
            if duration > 0 {
                return Some(Duration::from_secs(duration as u64));
            }
        }
    }

    None
}

/// Calculate delay for rate limit exceeded (429) response.
///
/// Priority order:
/// 1. Retry-After header if present
/// 2. X-RateLimit-Reset header if present
/// 3. Default 60 second delay
///
/// # Arguments
///
/// * `retry_after` - Optional Retry-After header value
/// * `rate_limit_reset` - Optional X-RateLimit-Reset header value (Unix timestamp)
///
/// # Returns
///
/// `Duration` to wait before retrying.
///
/// # Examples
///
/// ```
/// use github_bot_sdk::client::calculate_rate_limit_delay;
/// use std::time::Duration;
///
/// // With Retry-After header
/// let delay = calculate_rate_limit_delay(Some("60"), None);
/// assert_eq!(delay, Duration::from_secs(60));
///
/// // With X-RateLimit-Reset header (Unix timestamp)
/// let future_timestamp = (chrono::Utc::now().timestamp() + 120).to_string();
/// let delay = calculate_rate_limit_delay(None, Some(&future_timestamp));
/// assert!(delay >= Duration::from_secs(119) && delay <= Duration::from_secs(121));
///
/// // No headers, use default
/// let delay = calculate_rate_limit_delay(None, None);
/// assert_eq!(delay, Duration::from_secs(60));
/// ```
///
/// See github-bot-sdk-specs/interfaces/rate-limiting-retry.md
pub fn calculate_rate_limit_delay(
    retry_after: Option<&str>,
    rate_limit_reset: Option<&str>,
) -> Duration {
    // Priority 1: Retry-After header
    if let Some(retry_after_value) = retry_after {
        if let Some(delay) = parse_retry_after(retry_after_value) {
            return delay;
        }
    }

    // Priority 2: X-RateLimit-Reset header
    if let Some(reset_value) = rate_limit_reset {
        if let Ok(reset_timestamp) = reset_value.parse::<i64>() {
            if let Some(reset_time) = DateTime::from_timestamp(reset_timestamp, 0) {
                let now = Utc::now();
                if reset_time > now {
                    let duration = (reset_time - now).num_seconds();
                    if duration > 0 {
                        return Duration::from_secs(duration as u64);
                    }
                }
            }
        }
    }

    // Priority 3: Default delay
    Duration::from_secs(60)
}

/// Detect secondary rate limit (abuse detection) from 403 response.
///
/// GitHub API returns HTTP 403 for both permission denied and secondary
/// rate limits (abuse detection). This function distinguishes between them
/// by checking for rate limit indicators in the response body.
///
/// # Arguments
///
/// * `status` - HTTP status code
/// * `body` - Response body text
///
/// # Returns
///
/// `true` if this is a secondary rate limit (abuse), `false` otherwise.
///
/// # Detection Criteria
///
/// A 403 response is considered a secondary rate limit if the body contains:
/// - "rate limit" or "rate_limit" (case insensitive)
/// - "abuse" (case insensitive)
/// - "too many requests" (case insensitive)
///
/// # Examples
///
/// ```
/// use github_bot_sdk::client::detect_secondary_rate_limit;
///
/// // Secondary rate limit message
/// let is_secondary = detect_secondary_rate_limit(
///     403,
///     r#"{"message":"You have exceeded a secondary rate limit..."}"#
/// );
/// assert!(is_secondary);
///
/// // Permission denied (not rate limit)
/// let is_secondary = detect_secondary_rate_limit(
///     403,
///     r#"{"message":"Resource not accessible by integration"}"#
/// );
/// assert!(!is_secondary);
///
/// // Not a 403 response
/// let is_secondary = detect_secondary_rate_limit(404, "Not found");
/// assert!(!is_secondary);
/// ```
///
/// See github-bot-sdk-specs/interfaces/rate-limiting-retry.md
pub fn detect_secondary_rate_limit(status: u16, body: &str) -> bool {
    if status != 403 {
        return false;
    }

    let body_lower = body.to_lowercase();

    // Check for rate limit indicators in response body
    body_lower.contains("rate limit")
        || body_lower.contains("rate_limit")
        || body_lower.contains("abuse")
        || body_lower.contains("too many requests")
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
