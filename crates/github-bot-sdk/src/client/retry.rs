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

    /// Calculate delay for a specific retry attempt.
    ///
    /// Uses exponential backoff with optional jitter.
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
        // Note: Jitter implementation requires rand crate
        // For now, jitter is a no-op - will be implemented during task execution
        if self.use_jitter {
            // TODO: Implement jitter using rand crate
            // let jitter_factor = rand::thread_rng().gen_range(0.75..=1.25);
            // delay = Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64);
        }

        delay
    }

    /// Check if another retry attempt should be made.
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
