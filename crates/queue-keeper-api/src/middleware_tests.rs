//! Tests for HTTP middleware: IP rate limiting and admin authentication.

use std::time::Duration;

use super::*;

// ============================================================================
// IpFailureTracker tests
// ============================================================================

mod ip_failure_tracker_tests {
    use super::*;

    /// Verify that a fresh tracker reports no IP as blocked.
    #[test]
    fn test_new_tracker_reports_no_ip_as_blocked() {
        let tracker = IpFailureTracker::new(10, Duration::from_secs(300));
        assert!(!tracker.is_blocked("1.2.3.4"));
    }

    /// Verify that failure count starts at zero for an unseen IP.
    #[test]
    fn test_initial_failure_count_is_zero() {
        let tracker = IpFailureTracker::new(10, Duration::from_secs(300));
        assert_eq!(tracker.failure_count("1.2.3.4"), 0);
    }

    /// Verify that recording failures increments the counter.
    #[test]
    fn test_failure_count_increments_on_record() {
        let tracker = IpFailureTracker::new(10, Duration::from_secs(300));
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert_eq!(tracker.failure_count("1.2.3.4"), 2);
    }

    /// Verify that an IP is NOT blocked when below the threshold.
    #[test]
    fn test_ip_not_blocked_below_threshold() {
        let tracker = IpFailureTracker::new(3, Duration::from_secs(300));
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert!(!tracker.is_blocked("1.2.3.4"));
    }

    /// Verify that an IP is blocked once the failure threshold is reached.
    #[test]
    fn test_ip_blocked_at_threshold() {
        let tracker = IpFailureTracker::new(3, Duration::from_secs(300));
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert!(tracker.is_blocked("1.2.3.4"));
    }

    /// Verify that different IPs have independent failure counters.
    #[test]
    fn test_different_ips_are_independent() {
        let tracker = IpFailureTracker::new(2, Duration::from_secs(300));
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert!(tracker.is_blocked("1.2.3.4"));
        assert!(!tracker.is_blocked("5.6.7.8"));
    }

    /// Verify that failures outside the sliding window no longer contribute to
    /// the failure count and the IP is unblocked once the window expires.
    #[tokio::test]
    async fn test_failures_outside_window_are_ignored() {
        let tracker = IpFailureTracker::new(2, Duration::from_millis(50));
        tracker.record_failure("1.2.3.4");
        tracker.record_failure("1.2.3.4");
        assert!(
            tracker.is_blocked("1.2.3.4"),
            "IP should be blocked before window expires"
        );

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(
            !tracker.is_blocked("1.2.3.4"),
            "IP should be unblocked after failures expire"
        );
    }

    /// Verify that checking an unseen IP does not insert an empty entry into
    /// the HashMap, preventing unbounded memory growth.
    #[test]
    fn test_checking_unseen_ip_does_not_grow_map() {
        let tracker = IpFailureTracker::new(10, Duration::from_secs(300));
        // Call all three read paths with a fresh IP
        assert!(!tracker.is_blocked("192.0.2.1"));
        assert_eq!(tracker.failure_count("192.0.2.1"), 0);

        // The internal map must remain empty — no phantom entries created.
        let map = tracker.failures.lock().unwrap();
        assert_eq!(map.len(), 0, "No entries should be inserted for unseen IPs");
    }

    /// Verify that after the window expires, the entry is cleaned up so the map
    /// does not hold stale empty vecs.
    #[tokio::test]
    async fn test_expired_entries_are_removed_from_map() {
        let tracker = IpFailureTracker::new(5, Duration::from_millis(50));
        tracker.record_failure("192.0.2.2");

        {
            let map = tracker.failures.lock().unwrap();
            assert_eq!(map.len(), 1, "Entry should exist before expiry");
        }

        tokio::time::sleep(Duration::from_millis(60)).await;

        // Calling is_blocked prunes and removes the now-empty entry.
        tracker.is_blocked("192.0.2.2");

        let map = tracker.failures.lock().unwrap();
        assert_eq!(
            map.len(),
            0,
            "Expired empty entry should be removed from the map"
        );
    }

    /// Verify the public accessors return the configured values.
    #[test]
    fn test_accessors_return_configured_values() {
        let tracker = IpFailureTracker::new(7, Duration::from_secs(120));
        assert_eq!(tracker.max_failures(), 7);
        assert_eq!(tracker.window(), Duration::from_secs(120));
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
