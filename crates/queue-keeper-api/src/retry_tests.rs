//! Tests for retry policy module

use super::*;
use std::time::Duration;

// ============================================================================
// RetryPolicy Tests
// ============================================================================

#[test]
fn test_retry_policy_default() {
    let policy = RetryPolicy::default();

    assert_eq!(policy.max_attempts, 5);
    assert_eq!(policy.initial_delay, Duration::from_secs(1));
    assert_eq!(policy.max_delay, Duration::from_secs(16));
    assert_eq!(policy.backoff_multiplier, 2.0);
    assert!(policy.use_jitter);
    assert_eq!(policy.jitter_percent, 0.25);
}

#[test]
fn test_retry_policy_new() {
    let policy = RetryPolicy::new(3, Duration::from_millis(500), Duration::from_secs(10), 1.5);

    assert_eq!(policy.max_attempts, 3);
    assert_eq!(policy.initial_delay, Duration::from_millis(500));
    assert_eq!(policy.max_delay, Duration::from_secs(10));
    assert_eq!(policy.backoff_multiplier, 1.5);
    assert!(policy.use_jitter);
}

#[test]
fn test_retry_policy_without_jitter() {
    let policy = RetryPolicy::default().without_jitter();

    assert!(!policy.use_jitter);
}

#[test]
fn test_retry_policy_with_jitter_percent() {
    let policy = RetryPolicy::default().with_jitter_percent(0.5);

    assert_eq!(policy.jitter_percent, 0.5);
}

#[test]
fn test_retry_policy_jitter_percent_clamping() {
    // Test upper bound
    let policy = RetryPolicy::default().with_jitter_percent(1.5);
    assert_eq!(policy.jitter_percent, 1.0);

    // Test lower bound
    let policy = RetryPolicy::default().with_jitter_percent(-0.5);
    assert_eq!(policy.jitter_percent, 0.0);
}

#[test]
fn test_calculate_delay_exponential_backoff() {
    let policy =
        RetryPolicy::new(5, Duration::from_secs(1), Duration::from_secs(100), 2.0).without_jitter();

    // Attempt 0: 1 * 2^0 = 1s
    assert_eq!(policy.calculate_delay(0), Duration::from_secs(1));

    // Attempt 1: 1 * 2^1 = 2s
    assert_eq!(policy.calculate_delay(1), Duration::from_secs(2));

    // Attempt 2: 1 * 2^2 = 4s
    assert_eq!(policy.calculate_delay(2), Duration::from_secs(4));

    // Attempt 3: 1 * 2^3 = 8s
    assert_eq!(policy.calculate_delay(3), Duration::from_secs(8));
}

#[test]
fn test_calculate_delay_respects_max_delay() {
    let policy =
        RetryPolicy::new(10, Duration::from_secs(1), Duration::from_secs(5), 2.0).without_jitter();

    // Attempt 0: 1s
    assert_eq!(policy.calculate_delay(0), Duration::from_secs(1));

    // Attempt 1: 2s
    assert_eq!(policy.calculate_delay(1), Duration::from_secs(2));

    // Attempt 2: 4s
    assert_eq!(policy.calculate_delay(2), Duration::from_secs(4));

    // Attempt 3: would be 8s, but capped at 5s
    assert_eq!(policy.calculate_delay(3), Duration::from_secs(5));

    // Attempt 4: would be 16s, but capped at 5s
    assert_eq!(policy.calculate_delay(4), Duration::from_secs(5));
}

#[test]
fn test_calculate_delay_with_jitter() {
    let policy = RetryPolicy::new(5, Duration::from_secs(1), Duration::from_secs(100), 2.0)
        .with_jitter_percent(0.25);

    // With jitter, delays should be within expected range
    for attempt in 0..5 {
        let delay = policy.calculate_delay(attempt);
        let base = 2_u64.pow(attempt) as f64;

        // Allow ±25% jitter
        let min = (base * 0.75) as u64;
        let max = (base * 1.25) as u64;

        assert!(
            delay.as_secs() >= min && delay.as_secs() <= max,
            "Attempt {}: delay {:?} not in range {}s-{}s",
            attempt,
            delay,
            min,
            max
        );
    }
}

#[test]
fn test_should_retry() {
    let policy = RetryPolicy::new(3, Duration::from_secs(1), Duration::from_secs(10), 2.0);

    // Should retry for attempts 0, 1, 2 (3 retries)
    assert!(policy.should_retry(0));
    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));

    // Should not retry for attempt 3 (exceeds max_attempts)
    assert!(!policy.should_retry(3));
    assert!(!policy.should_retry(4));
}

#[test]
fn test_total_attempts() {
    let policy = RetryPolicy::new(5, Duration::from_secs(1), Duration::from_secs(10), 2.0);

    // 1 initial + 5 retries = 6 total
    assert_eq!(policy.total_attempts(), 6);
}

#[test]
fn test_different_backoff_multipliers() {
    // Test with 1.5x multiplier
    let policy =
        RetryPolicy::new(5, Duration::from_secs(1), Duration::from_secs(100), 1.5).without_jitter();

    // Attempt 0: 1 * 1.5^0 = 1s
    assert_eq!(policy.calculate_delay(0), Duration::from_secs(1));

    // Attempt 1: 1 * 1.5^1 = 1.5s
    assert_eq!(policy.calculate_delay(1), Duration::from_millis(1500));

    // Attempt 2: 1 * 1.5^2 = 2.25s
    assert_eq!(policy.calculate_delay(2), Duration::from_millis(2250));
}

// ============================================================================
// RetryState Tests
// ============================================================================

#[test]
fn test_retry_state_new() {
    let state = RetryState::new();

    assert_eq!(state.attempt, 0);
    assert_eq!(state.total_attempts, 1); // Started with initial attempt
}

#[test]
fn test_retry_state_default() {
    let state = RetryState::default();

    assert_eq!(state.attempt, 0);
    assert_eq!(state.total_attempts, 1);
}

#[test]
fn test_retry_state_next_attempt() {
    let mut state = RetryState::new();

    assert_eq!(state.attempt, 0);
    assert_eq!(state.total_attempts, 1);

    state.next_attempt();
    assert_eq!(state.attempt, 1);
    assert_eq!(state.total_attempts, 2);

    state.next_attempt();
    assert_eq!(state.attempt, 2);
    assert_eq!(state.total_attempts, 3);
}

#[test]
fn test_retry_state_is_first_retry() {
    let mut state = RetryState::new();

    assert!(state.is_first_retry());

    state.next_attempt();
    assert!(!state.is_first_retry());
}

#[test]
fn test_retry_state_get_delay() {
    let policy =
        RetryPolicy::new(5, Duration::from_secs(1), Duration::from_secs(100), 2.0).without_jitter();

    let mut state = RetryState::new();

    // First retry: 1s
    assert_eq!(state.get_delay(&policy), Duration::from_secs(1));

    state.next_attempt();

    // Second retry: 2s
    assert_eq!(state.get_delay(&policy), Duration::from_secs(2));
}

#[test]
fn test_retry_state_can_retry() {
    let policy = RetryPolicy::new(3, Duration::from_secs(1), Duration::from_secs(10), 2.0);

    let mut state = RetryState::new();

    // Can retry for first 3 attempts
    assert!(state.can_retry(&policy));

    state.next_attempt();
    assert!(state.can_retry(&policy));

    state.next_attempt();
    assert!(state.can_retry(&policy));

    state.next_attempt();
    // Now at attempt 3, which exceeds max_attempts of 3
    assert!(!state.can_retry(&policy));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_retry_loop_simulation() {
    let policy = RetryPolicy::new(3, Duration::from_millis(100), Duration::from_secs(1), 2.0)
        .without_jitter();

    let mut state = RetryState::new();
    let mut attempts = vec![];

    // Simulate retry loop
    loop {
        if !state.can_retry(&policy) {
            break;
        }

        let delay = state.get_delay(&policy);
        attempts.push((state.attempt, delay));

        state.next_attempt();
    }

    // Should have 3 retry attempts
    assert_eq!(attempts.len(), 3);

    // Verify delays
    assert_eq!(attempts[0], (0, Duration::from_millis(100)));
    assert_eq!(attempts[1], (1, Duration::from_millis(200)));
    assert_eq!(attempts[2], (2, Duration::from_millis(400)));

    // Total attempts should be 4 (1 initial + 3 retries)
    assert_eq!(state.total_attempts, 4);
}

#[test]
fn test_zero_max_attempts() {
    let policy = RetryPolicy::new(0, Duration::from_secs(1), Duration::from_secs(10), 2.0);

    let state = RetryState::new();

    // Should not retry with 0 max_attempts
    assert!(!state.can_retry(&policy));

    // But total_attempts should still be 1 (initial attempt)
    assert_eq!(policy.total_attempts(), 1);
}

#[test]
fn test_very_large_attempt_number() {
    let policy = RetryPolicy::new(100, Duration::from_secs(1), Duration::from_secs(60), 2.0)
        .without_jitter();

    // Even with very large attempt numbers, should cap at max_delay
    let delay = policy.calculate_delay(50);
    assert_eq!(delay, Duration::from_secs(60));
}
