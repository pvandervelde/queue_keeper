//! Tests for retry policy module

use super::*;
use std::time::Duration;

// ============================================================================
// RetryPolicy Tests
// ============================================================================

#[test]
fn test_retry_policy_default_values() {
    let policy = RetryPolicy::default();

    assert_eq!(policy.max_attempts, 5);
    assert_eq!(policy.initial_delay, Duration::from_secs(1));
    assert_eq!(policy.max_delay, Duration::from_secs(16));
    assert_eq!(policy.backoff_multiplier, 2.0);
    assert!(policy.use_jitter);
    assert_eq!(policy.jitter_percent, 0.25);
}

#[test]
fn test_retry_policy_custom_values() {
    let policy = RetryPolicy::new(3, Duration::from_millis(500), Duration::from_secs(10), 1.5);

    assert_eq!(policy.max_attempts, 3);
    assert_eq!(policy.initial_delay, Duration::from_millis(500));
    assert_eq!(policy.max_delay, Duration::from_secs(10));
    assert_eq!(policy.backoff_multiplier, 1.5);
}

#[test]
fn test_retry_policy_calculate_delay_without_jitter() {
    let policy = RetryPolicy::default().without_jitter();

    // First retry: 1 * 2^0 = 1 second
    assert_eq!(policy.calculate_delay(0), Duration::from_secs(1));

    // Second retry: 1 * 2^1 = 2 seconds
    assert_eq!(policy.calculate_delay(1), Duration::from_secs(2));

    // Third retry: 1 * 2^2 = 4 seconds
    assert_eq!(policy.calculate_delay(2), Duration::from_secs(4));

    // Fourth retry: 1 * 2^3 = 8 seconds
    assert_eq!(policy.calculate_delay(3), Duration::from_secs(8));

    // Fifth retry: 1 * 2^4 = 16 seconds (capped at max_delay)
    assert_eq!(policy.calculate_delay(4), Duration::from_secs(16));

    // Sixth retry: would be 32s but capped at 16s
    assert_eq!(policy.calculate_delay(5), Duration::from_secs(16));
}

#[test]
fn test_retry_policy_calculate_delay_with_jitter() {
    let policy = RetryPolicy::default(); // Jitter enabled by default

    // Test multiple times to ensure jitter is working
    let mut delays = Vec::new();
    for _ in 0..10 {
        let delay = policy.calculate_delay(0);
        delays.push(delay);
    }

    // With 25% jitter, 1s base should be in range [0.75s, 1.25s]
    for delay in &delays {
        let secs = delay.as_secs_f64();
        assert!(secs >= 0.75 && secs <= 1.25, "Delay {} out of range", secs);
    }

    // Check that we got some variation (not all the same)
    let unique_delays: std::collections::HashSet<_> = delays.iter().collect();
    assert!(
        unique_delays.len() > 1,
        "Expected variation in jittered delays"
    );
}

#[test]
fn test_retry_policy_should_retry() {
    let policy = RetryPolicy::default(); // max_attempts = 5

    // Should retry for attempts 0-4
    assert!(policy.should_retry(0));
    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));
    assert!(policy.should_retry(3));
    assert!(policy.should_retry(4));

    // Should not retry for attempt 5 and beyond
    assert!(!policy.should_retry(5));
    assert!(!policy.should_retry(6));
}

#[test]
fn test_retry_policy_total_attempts() {
    let policy = RetryPolicy::default(); // max_attempts = 5
    assert_eq!(policy.total_attempts(), 6); // 1 initial + 5 retries

    let policy = RetryPolicy::new(3, Duration::from_secs(1), Duration::from_secs(10), 2.0);
    assert_eq!(policy.total_attempts(), 4); // 1 initial + 3 retries
}

#[test]
fn test_retry_policy_with_custom_jitter_percent() {
    let policy = RetryPolicy::default().with_jitter_percent(0.5); // 50% jitter

    assert_eq!(policy.jitter_percent, 0.5);

    // Test that delays are within expected range
    for _ in 0..10 {
        let delay = policy.calculate_delay(0);
        let secs = delay.as_secs_f64();
        // With 50% jitter, 1s base should be in range [0.5s, 1.5s]
        assert!(secs >= 0.5 && secs <= 1.5, "Delay {} out of range", secs);
    }
}

#[test]
fn test_retry_policy_jitter_percent_clamped() {
    // Test that jitter percent is clamped to [0.0, 1.0]
    let policy1 = RetryPolicy::default().with_jitter_percent(-0.5);
    assert_eq!(policy1.jitter_percent, 0.0);

    let policy2 = RetryPolicy::default().with_jitter_percent(1.5);
    assert_eq!(policy2.jitter_percent, 1.0);
}

#[test]
fn test_retry_policy_exponential_backoff_sequence() {
    let policy = RetryPolicy::new(10, Duration::from_millis(100), Duration::from_secs(60), 2.0)
        .without_jitter();

    // Verify exponential growth: 100ms, 200ms, 400ms, 800ms, 1.6s, 3.2s, 6.4s, 12.8s, 25.6s, 51.2s
    assert_eq!(policy.calculate_delay(0), Duration::from_millis(100));
    assert_eq!(policy.calculate_delay(1), Duration::from_millis(200));
    assert_eq!(policy.calculate_delay(2), Duration::from_millis(400));
    assert_eq!(policy.calculate_delay(3), Duration::from_millis(800));
    assert_eq!(policy.calculate_delay(4), Duration::from_millis(1600));
    assert_eq!(policy.calculate_delay(5), Duration::from_millis(3200));
    assert_eq!(policy.calculate_delay(6), Duration::from_millis(6400));
    assert_eq!(policy.calculate_delay(7), Duration::from_millis(12800));
    assert_eq!(policy.calculate_delay(8), Duration::from_millis(25600));
    assert_eq!(policy.calculate_delay(9), Duration::from_millis(51200));

    // Further attempts capped at 60s
    assert_eq!(policy.calculate_delay(10), Duration::from_secs(60));
}

// ============================================================================
// RetryState Tests
// ============================================================================

#[test]
fn test_retry_state_initial_values() {
    let state = RetryState::new();

    assert_eq!(state.attempt, 0);
    assert_eq!(state.total_attempts, 1); // Initial attempt counts
    assert!(state.is_first_retry());
}

#[test]
fn test_retry_state_next_attempt() {
    let mut state = RetryState::new();

    assert_eq!(state.attempt, 0);
    assert_eq!(state.total_attempts, 1);

    state.next_attempt();
    assert_eq!(state.attempt, 1);
    assert_eq!(state.total_attempts, 2);
    assert!(!state.is_first_retry());

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
    let policy = RetryPolicy::default().without_jitter();
    let mut state = RetryState::new();

    // First retry
    assert_eq!(state.get_delay(&policy), Duration::from_secs(1));

    // Second retry
    state.next_attempt();
    assert_eq!(state.get_delay(&policy), Duration::from_secs(2));

    // Third retry
    state.next_attempt();
    assert_eq!(state.get_delay(&policy), Duration::from_secs(4));
}

#[test]
fn test_retry_state_can_retry() {
    let policy = RetryPolicy::default(); // max_attempts = 5
    let mut state = RetryState::new();

    // Can retry for attempts 0-4
    assert!(state.can_retry(&policy));

    for _ in 0..5 {
        state.next_attempt();
    }

    // Cannot retry after 5 attempts
    assert!(!state.can_retry(&policy));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_retry_workflow() {
    let policy = RetryPolicy::default().without_jitter();
    let mut state = RetryState::new();

    let mut delays = Vec::new();

    // Simulate retry loop
    while state.can_retry(&policy) {
        let delay = state.get_delay(&policy);
        delays.push(delay);
        state.next_attempt();
    }

    // Should have collected 5 delays (max_attempts)
    assert_eq!(delays.len(), 5);

    // Verify exponential backoff
    assert_eq!(delays[0], Duration::from_secs(1));
    assert_eq!(delays[1], Duration::from_secs(2));
    assert_eq!(delays[2], Duration::from_secs(4));
    assert_eq!(delays[3], Duration::from_secs(8));
    assert_eq!(delays[4], Duration::from_secs(16));
}

#[test]
fn test_retry_with_early_success() {
    let policy = RetryPolicy::default();
    let mut state = RetryState::new();

    // Simulate success on third attempt
    let mut attempts = 0;
    while state.can_retry(&policy) {
        attempts += 1;

        if attempts == 3 {
            // Success! Break early
            break;
        }

        state.next_attempt();
    }

    assert_eq!(state.total_attempts, 3);
    assert_eq!(state.attempt, 2); // 0-based, so attempt 2 is third try
}
