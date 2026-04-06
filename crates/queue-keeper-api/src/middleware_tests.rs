//! Tests for HTTP middleware: IP rate limiting and admin authentication.

use std::time::Duration;

use super::*;

// ============================================================================
// Helpers
// ============================================================================

/// Build a tracker with spec-default settings suitable for most tests.
///
/// rate_restrict_threshold=10, block_threshold=50, window=5 min,
/// rate_restrict_duration=1h, block_duration=24h.
fn spec_tracker() -> IpFailureTracker {
    IpFailureTracker::new(
        10,
        50,
        Duration::from_secs(300),
        Duration::from_secs(3_600),
        Duration::from_secs(86_400),
    )
}

/// Build a tracker with a very short window/durations for time-sensitive tests.
fn fast_tracker(rate_restrict_threshold: usize, block_threshold: usize) -> IpFailureTracker {
    IpFailureTracker::new(
        rate_restrict_threshold,
        block_threshold,
        Duration::from_millis(50),  // window
        Duration::from_millis(80),  // rate_restrict_duration
        Duration::from_millis(120), // block_duration
    )
}

// ============================================================================
// IpFailureTracker — basic counter tests
// ============================================================================

mod ip_failure_tracker_tests {
    use super::*;

    /// Verify that a fresh tracker reports no IP as blocked.
    #[test]
    fn test_new_tracker_reports_no_ip_as_blocked() {
        let tracker = spec_tracker();
        assert!(!tracker.is_blocked("1.2.3.4"));
    }

    /// Verify check_tier returns Normal for an unseen IP.
    #[test]
    fn test_new_tracker_returns_normal_tier_for_unseen_ip() {
        let tracker = spec_tracker();
        assert_eq!(tracker.check_tier("1.2.3.4"), IpTier::Normal);
    }

    /// Verify that failure count starts at zero for an unseen IP.
    #[test]
    fn test_initial_failure_count_is_zero() {
        let tracker = spec_tracker();
        assert_eq!(tracker.failure_count("1.2.3.4"), 0);
    }

    /// Verify that recording failures increments the counter.
    #[test]
    fn test_failure_count_increments_on_record() {
        let tracker = spec_tracker();
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert_eq!(tracker.failure_count("1.2.3.4"), 2);
    }

    /// Verify that an IP stays Normal when below the rate-restrict threshold.
    #[test]
    fn test_ip_stays_normal_below_rate_restrict_threshold() {
        let tracker = IpFailureTracker::new(
            3,
            10,
            Duration::from_secs(300),
            Duration::from_secs(3_600),
            Duration::from_secs(86_400),
        );
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert_eq!(tracker.check_tier("1.2.3.4"), IpTier::Normal);
        assert!(!tracker.is_blocked("1.2.3.4"));
    }

    /// Verify that different IPs have independent failure counters and tiers.
    #[test]
    fn test_different_ips_are_independent() {
        let tracker = IpFailureTracker::new(
            2,
            10,
            Duration::from_secs(300),
            Duration::from_secs(3_600),
            Duration::from_secs(86_400),
        );
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert!(tracker.is_blocked("1.2.3.4"));
        assert!(!tracker.is_blocked("5.6.7.8"));
    }

    /// Verify that failures outside the sliding window no longer contribute to
    /// the failure count and the IP is unblocked once the window expires.
    #[tokio::test]
    async fn test_failures_outside_window_do_not_affect_tier_after_expiry() {
        // rate_restrict_threshold=2, block_threshold=10
        // window=50ms, rate_restrict_duration=200ms
        let tracker = IpFailureTracker::new(
            2,
            10,
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_millis(500),
        );
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        // IP is now RateRestricted for 200ms
        assert!(
            tracker.is_blocked("1.2.3.4"),
            "IP should be rate-restricted immediately after crossing threshold"
        );

        // Wait for rate_restrict_duration to expire
        tokio::time::sleep(Duration::from_millis(210)).await;
        assert!(
            !tracker.is_blocked("1.2.3.4"),
            "IP should be Normal after rate-restrict duration expires"
        );
    }

    /// Verify that checking an unseen IP does not insert an empty entry into
    /// the HashMap, preventing unbounded memory growth.
    #[test]
    fn test_checking_unseen_ip_does_not_grow_map() {
        let tracker = spec_tracker();
        assert!(!tracker.is_blocked("192.0.2.1"));
        assert_eq!(tracker.failure_count("192.0.2.1"), 0);

        let states = tracker.states.lock().unwrap();
        assert_eq!(
            states.len(),
            0,
            "No entries should be inserted for unseen IPs"
        );
    }

    /// Verify the public accessors return the configured values.
    #[test]
    fn test_accessors_return_configured_values() {
        let tracker = IpFailureTracker::new(
            7,
            30,
            Duration::from_secs(120),
            Duration::from_secs(1_800),
            Duration::from_secs(43_200),
        );
        assert_eq!(tracker.rate_restrict_threshold(), 7);
        assert_eq!(tracker.block_threshold(), 30);
        assert_eq!(tracker.window(), Duration::from_secs(120));
        assert_eq!(tracker.rate_restrict_duration(), Duration::from_secs(1_800));
        assert_eq!(tracker.block_duration(), Duration::from_secs(43_200));
    }
}

// ============================================================================
// IpFailureTracker — three-tier escalation tests
// ============================================================================

mod ip_tier_escalation_tests {
    use super::*;

    /// Verify that reaching the rate-restrict threshold transitions the IP to
    /// the RateRestricted tier with a 1-hour retry-after.
    #[test]
    fn test_tier_becomes_rate_restricted_at_lower_threshold() {
        let tracker = IpFailureTracker::new(
            3,
            10,
            Duration::from_secs(300),
            Duration::from_secs(3_600),
            Duration::from_secs(86_400),
        );
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4"); // crosses threshold of 3
        let tier = tracker.check_tier("1.2.3.4");
        assert!(
            matches!(tier, IpTier::RateRestricted { .. }),
            "Expected RateRestricted, got {:?}",
            tier
        );
        assert_eq!(tier.retry_after_secs(), 3_600);
        assert!(tier.is_restricted());
    }

    /// Verify that exceeding the block threshold transitions the IP to
    /// the Blocked tier with a 24-hour retry-after.
    #[test]
    fn test_tier_becomes_blocked_above_upper_threshold() {
        let tracker = IpFailureTracker::new(
            3,
            5,
            Duration::from_secs(300),
            Duration::from_secs(3_600),
            Duration::from_secs(86_400),
        );
        for _ in 0..6 {
            tracker.record_failure("1.2.3.4");
        }
        let tier = tracker.check_tier("1.2.3.4");
        assert!(
            matches!(tier, IpTier::Blocked { .. }),
            "Expected Blocked, got {:?}",
            tier
        );
        assert_eq!(tier.retry_after_secs(), 86_400);
        assert!(tier.is_restricted());
    }

    /// Verify that once in RateRestricted, additional failures escalate to Blocked.
    #[test]
    fn test_rate_restricted_escalates_to_blocked_on_more_failures() {
        let tracker = IpFailureTracker::new(
            3,
            5,
            Duration::from_secs(300),
            Duration::from_secs(3_600),
            Duration::from_secs(86_400),
        );
        // Cross rate-restrict threshold
        for _ in 0..3 {
            tracker.record_failure("1.2.3.4");
        }
        assert!(
            matches!(tracker.check_tier("1.2.3.4"), IpTier::RateRestricted { .. }),
            "Should be RateRestricted after 3 failures"
        );
        // Cross block threshold
        for _ in 0..3 {
            tracker.record_failure("1.2.3.4");
        }
        assert!(
            matches!(tracker.check_tier("1.2.3.4"), IpTier::Blocked { .. }),
            "Should escalate to Blocked after 6 failures"
        );
    }

    /// Verify that the Blocked tier is not downgraded by additional failures.
    #[test]
    fn test_blocked_tier_is_not_downgraded_by_more_failures() {
        let tracker = IpFailureTracker::new(
            3,
            5,
            Duration::from_secs(300),
            Duration::from_secs(3_600),
            Duration::from_secs(86_400),
        );
        for _ in 0..6 {
            tracker.record_failure("1.2.3.4");
        }
        assert!(matches!(
            tracker.check_tier("1.2.3.4"),
            IpTier::Blocked { .. }
        ));

        // More failures should not change the tier
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert!(
            matches!(tracker.check_tier("1.2.3.4"), IpTier::Blocked { .. }),
            "Blocked tier should persist after further failures"
        );
    }

    /// Verify that the RateRestricted tier expires after its duration and
    /// the IP returns to Normal.
    #[tokio::test]
    async fn test_rate_restricted_tier_expires_after_duration() {
        let tracker = fast_tracker(2, 20);
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4"); // cross rate_restrict_threshold
        assert!(
            matches!(tracker.check_tier("1.2.3.4"), IpTier::RateRestricted { .. }),
            "Should be RateRestricted immediately"
        );

        tokio::time::sleep(Duration::from_millis(90)).await; // > rate_restrict_duration (80ms)
        assert_eq!(
            tracker.check_tier("1.2.3.4"),
            IpTier::Normal,
            "Should return to Normal after rate_restrict_duration expires"
        );
        assert!(!tracker.is_blocked("1.2.3.4"));
    }

    /// Verify that the Blocked tier expires after its duration and the IP
    /// returns to Normal.
    #[tokio::test]
    async fn test_blocked_tier_expires_after_duration() {
        let tracker = fast_tracker(2, 3);
        for _ in 0..4 {
            tracker.record_failure("1.2.3.4"); // cross block_threshold (3)
        }
        assert!(
            matches!(tracker.check_tier("1.2.3.4"), IpTier::Blocked { .. }),
            "Should be Blocked immediately"
        );

        tokio::time::sleep(Duration::from_millis(130)).await; // > block_duration (120ms)
        assert_eq!(
            tracker.check_tier("1.2.3.4"),
            IpTier::Normal,
            "Should return to Normal after block_duration expires"
        );
        assert!(!tracker.is_blocked("1.2.3.4"));
    }

    /// Verify that record_failure also expires the old tier before escalating,
    /// so a new failure after a Blocked tier expires can escalate again cleanly.
    #[tokio::test]
    async fn test_failure_after_block_expiry_starts_fresh_escalation() {
        let tracker = fast_tracker(2, 3);
        for _ in 0..4 {
            tracker.record_failure("1.2.3.4");
        }
        assert!(matches!(
            tracker.check_tier("1.2.3.4"),
            IpTier::Blocked { .. }
        ));

        // Wait for: window (50ms) + block_duration (120ms) to both expire
        tokio::time::sleep(Duration::from_millis(180)).await;

        // One fresh failure should not re-block (count = 1, below threshold)
        tracker.record_failure("1.2.3.4");
        assert_eq!(
            tracker.check_tier("1.2.3.4"),
            IpTier::Normal,
            "Single failure after full expiry should stay Normal"
        );
    }

    /// Verify IpTier::Normal retry_after_secs is 0.
    #[test]
    fn test_normal_tier_retry_after_is_zero() {
        assert_eq!(IpTier::Normal.retry_after_secs(), 0);
        assert!(!IpTier::Normal.is_restricted());
    }
}

// ============================================================================
// extract_client_ip tests
// ============================================================================

mod extract_client_ip_tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderName, HeaderValue};

    fn headers_with(key: &str, value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            HeaderName::from_bytes(key.as_bytes()).unwrap(),
            HeaderValue::from_str(value).unwrap(),
        );
        h
    }

    /// Verify that the leftmost (original client) IP in X-Forwarded-For is used.
    #[test]
    fn test_uses_first_ip_from_x_forwarded_for() {
        let headers = headers_with("x-forwarded-for", "203.0.113.1, 10.0.0.1, 192.168.1.1");
        assert_eq!(extract_client_ip(&headers), "203.0.113.1");
    }

    /// Verify that X-Real-IP is used when X-Forwarded-For is absent.
    #[test]
    fn test_falls_back_to_x_real_ip() {
        let headers = headers_with("x-real-ip", "203.0.113.5");
        assert_eq!(extract_client_ip(&headers), "203.0.113.5");
    }

    /// Verify that "unknown" is returned when no IP header is present.
    #[test]
    fn test_returns_unknown_when_no_ip_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_client_ip(&headers), "unknown");
    }

    /// Verify that X-Forwarded-For takes priority over X-Real-IP when both
    /// are present.
    #[test]
    fn test_x_forwarded_for_takes_priority_over_x_real_ip() {
        let mut h = HeaderMap::new();
        h.insert(
            HeaderName::from_bytes(b"x-forwarded-for").unwrap(),
            HeaderValue::from_static("203.0.113.1"),
        );
        h.insert(
            HeaderName::from_bytes(b"x-real-ip").unwrap(),
            HeaderValue::from_static("10.0.0.1"),
        );
        assert_eq!(extract_client_ip(&h), "203.0.113.1");
    }

    /// Verify that surrounding whitespace in X-Forwarded-For IPs is trimmed.
    #[test]
    fn test_x_forwarded_for_ip_is_trimmed() {
        let headers = headers_with("x-forwarded-for", "  203.0.113.2  , 10.0.0.1");
        assert_eq!(extract_client_ip(&headers), "203.0.113.2");
    }
}

// ============================================================================
// constant_time_eq tests
// ============================================================================

mod constant_time_eq_tests {
    use super::*;

    /// Verify that equal byte slices compare as equal.
    #[test]
    fn test_equal_byte_slices_are_equal() {
        assert!(constant_time_eq(b"secret-key", b"secret-key"));
    }

    /// Verify that byte slices with different content compare as unequal.
    #[test]
    fn test_different_byte_content_is_not_equal() {
        assert!(!constant_time_eq(b"secret", b"SECRET"));
    }

    /// Verify that byte slices with different lengths compare as unequal.
    #[test]
    fn test_different_length_slices_are_not_equal() {
        assert!(!constant_time_eq(b"short", b"longer"));
    }

    /// Verify that empty slices compare as equal.
    #[test]
    fn test_empty_slices_are_equal() {
        assert!(constant_time_eq(b"", b""));
    }

    /// Verify that a single differing byte makes the comparison unequal.
    #[test]
    fn test_single_bit_difference_is_not_equal() {
        assert!(!constant_time_eq(b"abcdef", b"abcdeF"));
    }
}
