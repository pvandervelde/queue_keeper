//! # Retry Policy Module
//!
//! Implements exponential backoff retry logic for transient queue delivery failures.
//!
//! Provides configurable retry policies with jitter to prevent thundering herd problems.

use rand::Rng;
use std::time::Duration;

/// Retry policy configuration for exponential backoff
///
/// # Examples
///
/// ```rust
/// use queue_keeper_service::retry::RetryPolicy;
/// use std::time::Duration;
///
/// // Default policy: 5 attempts, 1s initial, 16s max, 2.0x multiplier
/// let policy = RetryPolicy::default();
///
/// // Custom policy
/// let policy = RetryPolicy::new(3, Duration::from_millis(500), Duration::from_secs(5), 1.5);
/// ```
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Initial delay before first retry
    pub initial_delay: Duration,

    /// Maximum delay between retries
    pub max_delay: Duration,

    /// Exponential backoff multiplier (typically 2.0)
    pub backoff_multiplier: f64,

    /// Whether to add jitter to delays (recommended)
    pub use_jitter: bool,

    /// Jitter range as percentage (default 25% = ±25%)
    pub jitter_percent: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(16),
            backoff_multiplier: 2.0,
            use_jitter: true,
            jitter_percent: 0.25, // ±25%
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - Maximum retry attempts (typically 3-5)
    /// * `initial_delay` - Initial delay before first retry
    /// * `max_delay` - Maximum delay cap
    /// * `backoff_multiplier` - Exponential growth factor (typically 1.5-2.0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_service::retry::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::new(
    ///     3,
    ///     Duration::from_millis(500),
    ///     Duration::from_secs(10),
    ///     2.0
    /// );
    /// ```
    pub fn new(
        max_attempts: u32,
        initial_delay: Duration,
        max_delay: Duration,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_attempts,
            initial_delay,
            max_delay,
            backoff_multiplier,
            use_jitter: true,
            jitter_percent: 0.25,
        }
    }

    /// Disable jitter (not recommended for production)
    pub fn without_jitter(mut self) -> Self {
        self.use_jitter = false;
        self
    }

    /// Set custom jitter percentage (0.0 to 1.0)
    pub fn with_jitter_percent(mut self, percent: f64) -> Self {
        self.jitter_percent = percent.clamp(0.0, 1.0);
        self
    }

    /// Calculate delay for a specific retry attempt
    ///
    /// Uses exponential backoff formula: delay = initial * multiplier^attempt
    /// Adds jitter if enabled to prevent thundering herd
    ///
    /// # Arguments
    ///
    /// * `attempt` - Retry attempt number (0-based)
    ///
    /// # Returns
    ///
    /// Duration to wait before this retry attempt
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_service::retry::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default();
    ///
    /// // First retry (attempt 0): ~1s
    /// let delay = policy.calculate_delay(0);
    /// assert!(delay.as_secs() >= 0 && delay.as_secs() <= 2);
    ///
    /// // Second retry (attempt 1): ~2s
    /// let delay = policy.calculate_delay(1);
    /// assert!(delay.as_secs() >= 1 && delay.as_secs() <= 3);
    /// ```
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        // Calculate base delay: initial * multiplier^attempt
        let base_delay_secs =
            self.initial_delay.as_secs_f64() * self.backoff_multiplier.powi(attempt as i32);

        // Cap at max_delay
        let capped_delay_secs = base_delay_secs.min(self.max_delay.as_secs_f64());

        // Add jitter if enabled
        let final_delay_secs = if self.use_jitter {
            Self::add_jitter(capped_delay_secs, self.jitter_percent)
        } else {
            capped_delay_secs
        };

        Duration::from_secs_f64(final_delay_secs)
    }

    /// Check if we should retry for this attempt number
    ///
    /// # Arguments
    ///
    /// * `attempt` - Current attempt number (0-based, where 0 is first retry)
    ///
    /// # Returns
    ///
    /// `true` if we haven't exceeded max_attempts
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_service::retry::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default(); // max_attempts = 5
    ///
    /// assert!(policy.should_retry(0));  // First retry
    /// assert!(policy.should_retry(4));  // Fifth retry
    /// assert!(!policy.should_retry(5)); // Sixth would exceed max
    /// ```
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }

    /// Add jitter to a delay value
    ///
    /// Applies random variation in range [delay * (1-jitter), delay * (1+jitter)]
    ///
    /// # Arguments
    ///
    /// * `delay_secs` - Base delay in seconds
    /// * `jitter_percent` - Jitter percentage (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Delay with jitter applied
    fn add_jitter(delay_secs: f64, jitter_percent: f64) -> f64 {
        let mut rng = rand::thread_rng();

        // Calculate jitter range: ±jitter_percent of delay
        let jitter_range = delay_secs * jitter_percent;

        // Generate random value in range [-jitter_range, +jitter_range]
        let jitter = rng.gen_range(-jitter_range..=jitter_range);

        // Apply jitter, ensuring result is positive
        (delay_secs + jitter).max(0.0)
    }

    /// Get total number of delivery attempts (initial + retries)
    ///
    /// # Returns
    ///
    /// Total attempts including initial try
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_service::retry::RetryPolicy;
    ///
    /// let policy = RetryPolicy::default(); // max_attempts = 5
    /// assert_eq!(policy.total_attempts(), 6); // 1 initial + 5 retries
    /// ```
    pub fn total_attempts(&self) -> u32 {
        self.max_attempts + 1 // Initial attempt + retries
    }
}

/// State tracker for retry operations
///
/// Tracks current attempt number and provides helper methods for retry logic.
#[derive(Debug, Clone)]
pub struct RetryState {
    /// Current retry attempt (0-based)
    pub attempt: u32,

    /// Total attempts made so far (including initial)
    pub total_attempts: u32,
}

impl Default for RetryState {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryState {
    /// Create new retry state starting at attempt 0
    pub fn new() -> Self {
        Self {
            attempt: 0,
            total_attempts: 1, // Started with initial attempt
        }
    }

    /// Increment to next retry attempt
    pub fn next_attempt(&mut self) {
        self.attempt += 1;
        self.total_attempts += 1;
    }

    /// Check if this is the first retry (not initial attempt)
    pub fn is_first_retry(&self) -> bool {
        self.attempt == 0
    }

    /// Get next delay from policy
    pub fn get_delay(&self, policy: &RetryPolicy) -> Duration {
        policy.calculate_delay(self.attempt)
    }

    /// Check if we can retry with this policy
    pub fn can_retry(&self, policy: &RetryPolicy) -> bool {
        policy.should_retry(self.attempt)
    }
}

#[cfg(test)]
#[path = "retry_tests.rs"]
mod tests;
