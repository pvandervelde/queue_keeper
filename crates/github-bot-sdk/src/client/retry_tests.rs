//! Tests for retry policy and rate limiting.

use super::*;

mod rate_limit_info {
    use super::*;

    #[test]
    #[ignore = "TODO: Verify RateLimitInfo::from_headers with valid headers"]
    fn test_from_headers_valid() {
        todo!("Verify RateLimitInfo::from_headers with valid headers")
    }

    #[test]
    #[ignore = "TODO: Verify from_headers returns None when headers missing"]
    fn test_from_headers_missing() {
        todo!("Verify from_headers returns None when headers missing")
    }

    #[test]
    #[ignore = "TODO: Verify from_headers returns None when headers invalid"]
    fn test_from_headers_invalid() {
        todo!("Verify from_headers returns None when headers invalid")
    }

    #[test]
    #[ignore = "TODO: Verify is_limited is true when remaining=0"]
    fn test_is_limited() {
        todo!("Verify is_limited is true when remaining=0")
    }

    #[test]
    #[ignore = "TODO: Verify is_limited is false when remaining>0"]
    fn test_is_not_limited() {
        todo!("Verify is_limited is false when remaining>0")
    }

    #[test]
    #[ignore = "TODO: Verify is_near_limit when below threshold"]
    fn test_is_near_limit_true() {
        todo!("Verify is_near_limit when below threshold")
    }

    #[test]
    #[ignore = "TODO: Verify is_near_limit when above threshold"]
    fn test_is_near_limit_false() {
        todo!("Verify is_near_limit when above threshold")
    }

    #[test]
    #[ignore = "TODO: Verify time_until_reset when reset is in future"]
    fn test_time_until_reset_future() {
        todo!("Verify time_until_reset when reset is in future")
    }

    #[test]
    #[ignore = "TODO: Verify time_until_reset returns 0 when reset is in past"]
    fn test_time_until_reset_past() {
        todo!("Verify time_until_reset returns 0 when reset is in past")
    }
}

mod retry_policy {
    use super::*;

    /// Verify that RetryPolicy::default() has expected values.
    ///
    /// Default policy should have 3 max retries, 100ms initial delay,
    /// 60s max delay, 2.0 multiplier, and jitter enabled.
    #[test]
    fn test_default() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.initial_delay, Duration::from_millis(100));
        assert_eq!(policy.max_delay, Duration::from_secs(60));
        assert_eq!(policy.backoff_multiplier, 2.0);
        assert!(policy.use_jitter);
    }

    /// Verify that RetryPolicy::new creates a policy with custom values.
    #[test]
    fn test_new() {
        let policy = RetryPolicy::new(5, Duration::from_millis(500), Duration::from_secs(30));

        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_delay, Duration::from_millis(500));
        assert_eq!(policy.max_delay, Duration::from_secs(30));
        assert_eq!(policy.backoff_multiplier, 2.0);
        assert!(policy.use_jitter); // Default is enabled
    }

    /// Verify that with_jitter() enables jitter.
    #[test]
    fn test_with_jitter() {
        let policy = RetryPolicy::default().with_jitter();
        assert!(policy.use_jitter);
    }

    /// Verify that without_jitter() disables jitter.
    #[test]
    fn test_without_jitter() {
        let policy = RetryPolicy::default().without_jitter();
        assert!(!policy.use_jitter);
    }

    /// Verify that attempt 0 returns zero delay.
    ///
    /// The first request (attempt 0) should proceed immediately.
    #[test]
    fn test_calculate_delay_attempt_zero() {
        let policy = RetryPolicy::default();
        let delay = policy.calculate_delay(0);
        assert_eq!(delay, Duration::from_secs(0));
    }

    /// Verify that delays grow exponentially without jitter.
    ///
    /// With 100ms initial delay and 2.0 multiplier:
    /// - Attempt 1: 100ms
    /// - Attempt 2: 200ms
    /// - Attempt 3: 400ms
    /// - Attempt 4: 800ms
    #[test]
    fn test_calculate_delay_exponential_backoff() {
        let policy = RetryPolicy::default().without_jitter();

        assert_eq!(policy.calculate_delay(1), Duration::from_millis(100));
        assert_eq!(policy.calculate_delay(2), Duration::from_millis(200));
        assert_eq!(policy.calculate_delay(3), Duration::from_millis(400));
        assert_eq!(policy.calculate_delay(4), Duration::from_millis(800));
    }

    /// Verify that delay is capped at max_delay.
    ///
    /// Even with exponential growth, delays should never exceed max_delay.
    #[test]
    fn test_calculate_delay_max_cap() {
        let policy = RetryPolicy {
            max_retries: 100,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            use_jitter: false,
        };

        // Attempt 20 would be 100ms * 2^19 = ~52 seconds
        // Should be capped at 5 seconds
        let delay = policy.calculate_delay(20);
        assert_eq!(delay, Duration::from_secs(5));
    }

    /// Verify that jitter adds randomization within ±25%.
    ///
    /// With jitter enabled, delays should vary but stay within bounds.
    /// We test by running multiple calculations and verifying the range.
    #[test]
    fn test_calculate_delay_with_jitter() {
        let policy = RetryPolicy::default().with_jitter();

        // For attempt 1, base delay is 100ms
        // With ±25% jitter, range is 75-125ms
        let mut delays = Vec::new();
        for _ in 0..100 {
            let delay = policy.calculate_delay(1);
            delays.push(delay.as_millis());
        }

        // All delays should be within the jitter range
        for delay_ms in &delays {
            assert!(*delay_ms >= 75, "Delay {}ms below minimum 75ms", delay_ms);
            assert!(*delay_ms <= 125, "Delay {}ms above maximum 125ms", delay_ms);
        }

        // With 100 samples, we should see variation (not all the same)
        let min = *delays.iter().min().unwrap();
        let max = *delays.iter().max().unwrap();
        assert!(max > min, "Jitter should produce variation in delays");
    }

    /// Verify that delay is deterministic when use_jitter=false.
    ///
    /// Without jitter, the same attempt should always produce the same delay.
    #[test]
    fn test_calculate_delay_without_jitter() {
        let policy = RetryPolicy::default().without_jitter();

        let delay1 = policy.calculate_delay(1);
        let delay2 = policy.calculate_delay(1);
        let delay3 = policy.calculate_delay(1);

        assert_eq!(delay1, delay2);
        assert_eq!(delay2, delay3);
        assert_eq!(delay1, Duration::from_millis(100));
    }

    /// Verify that jitter stays within bounds for large delays.
    ///
    /// Even for delays near max_delay, jitter should respect bounds.
    #[test]
    fn test_calculate_delay_jitter_respects_bounds() {
        let policy = RetryPolicy {
            max_retries: 10,
            initial_delay: Duration::from_millis(5000),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            use_jitter: true,
        };

        // This should be capped at 10 seconds, then jitter applied
        // Jitter of ±25% means 7.5-12.5 seconds
        // But max_delay is 10s, so actual range is 7.5-10s
        let mut delays = Vec::new();
        for _ in 0..50 {
            let delay = policy.calculate_delay(5);
            delays.push(delay.as_millis());
        }

        for delay_ms in &delays {
            // With jitter on 10s max, minimum is 7.5s
            assert!(*delay_ms >= 7500, "Delay {}ms below minimum", delay_ms);
            // Jitter can push above max_delay
            assert!(*delay_ms <= 12500, "Delay {}ms above maximum", delay_ms);
        }
    }

    /// Verify that should_retry returns true when attempts < max.
    #[test]
    fn test_should_retry_true() {
        let policy = RetryPolicy::default(); // max_retries = 3

        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
    }

    /// Verify that should_retry returns false when attempts >= max.
    #[test]
    fn test_should_retry_false() {
        let policy = RetryPolicy::default(); // max_retries = 3

        assert!(!policy.should_retry(3));
        assert!(!policy.should_retry(4));
        assert!(!policy.should_retry(100));
    }

    /// Verify that builder pattern works correctly.
    #[test]
    fn test_builder_pattern() {
        let policy = RetryPolicy::new(5, Duration::from_millis(200), Duration::from_secs(30))
            .without_jitter();

        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.initial_delay, Duration::from_millis(200));
        assert_eq!(policy.max_delay, Duration::from_secs(30));
        assert!(!policy.use_jitter);

        // Chaining should work
        let policy2 = policy.clone().with_jitter();
        assert!(policy2.use_jitter);
    }
}

mod serialization {
    use super::*;

    #[test]
    #[ignore = "TODO: Verify RateLimitInfo can be serialized"]
    fn test_rate_limit_info_serialize() {
        todo!("Verify RateLimitInfo can be serialized")
    }

    #[test]
    #[ignore = "TODO: Verify RateLimitInfo can be deserialized"]
    fn test_rate_limit_info_deserialize() {
        todo!("Verify RateLimitInfo can be deserialized")
    }

    #[test]
    #[ignore = "TODO: Verify RetryPolicy can be serialized"]
    fn test_retry_policy_serialize() {
        todo!("Verify RetryPolicy can be serialized")
    }

    #[test]
    #[ignore = "TODO: Verify RetryPolicy can be deserialized"]
    fn test_retry_policy_deserialize() {
        todo!("Verify RetryPolicy can be deserialized")
    }
}

mod retry_after_parsing {
    use super::*;

    /// Verify that parse_retry_after correctly parses delta-seconds format.
    ///
    /// The Retry-After header often uses simple integer seconds.
    #[test]
    fn test_parse_retry_after_delta_seconds() {
        assert_eq!(parse_retry_after("60"), Some(Duration::from_secs(60)));
        assert_eq!(parse_retry_after("120"), Some(Duration::from_secs(120)));
        assert_eq!(parse_retry_after("0"), Some(Duration::from_secs(0)));
    }

    /// Verify that parse_retry_after handles HTTP-date format.
    ///
    /// GitHub may use RFC 2822 date format in Retry-After headers.
    #[test]
    fn test_parse_retry_after_http_date() {
        // Create a future date (60 seconds from now)
        let future = Utc::now() + chrono::Duration::seconds(60);
        let http_date = future.to_rfc2822();

        let delay = parse_retry_after(&http_date);
        assert!(delay.is_some());

        let delay_secs = delay.unwrap().as_secs();
        // Should be approximately 60 seconds (allow 1 second variance for test execution)
        assert!(
            delay_secs >= 59 && delay_secs <= 61,
            "Delay was {}s",
            delay_secs
        );
    }

    /// Verify that parse_retry_after returns None for invalid formats.
    #[test]
    fn test_parse_retry_after_invalid() {
        assert_eq!(parse_retry_after("invalid"), None);
        assert_eq!(parse_retry_after("not a number"), None);
        assert_eq!(parse_retry_after(""), None);
        assert_eq!(parse_retry_after("-10"), None); // Negative numbers invalid
    }

    /// Verify that parse_retry_after returns None for past HTTP dates.
    #[test]
    fn test_parse_retry_after_past_date() {
        // Create a past date
        let past = Utc::now() - chrono::Duration::seconds(60);
        let http_date = past.to_rfc2822();

        let delay = parse_retry_after(&http_date);
        assert_eq!(delay, None);
    }

    /// Verify that very large delay values are handled correctly.
    #[test]
    fn test_parse_retry_after_large_values() {
        // 1 hour in seconds
        assert_eq!(parse_retry_after("3600"), Some(Duration::from_secs(3600)));

        // 24 hours in seconds
        assert_eq!(parse_retry_after("86400"), Some(Duration::from_secs(86400)));
    }
}

mod rate_limit_delay_calculation {
    use super::*;

    /// Verify that Retry-After header takes priority over rate limit reset.
    #[test]
    fn test_calculate_rate_limit_delay_retry_after_priority() {
        let future_timestamp = (Utc::now().timestamp() + 300).to_string(); // 5 minutes

        let delay = calculate_rate_limit_delay(
            Some("60"),              // 1 minute
            Some(&future_timestamp), // 5 minutes
        );

        // Should use Retry-After (60s), not reset time (300s)
        assert_eq!(delay, Duration::from_secs(60));
    }

    /// Verify that rate limit reset is used when Retry-After is absent.
    #[test]
    fn test_calculate_rate_limit_delay_uses_reset() {
        let future_timestamp = (Utc::now().timestamp() + 120).to_string(); // 2 minutes

        let delay = calculate_rate_limit_delay(None, Some(&future_timestamp));

        // Should be approximately 120 seconds
        let delay_secs = delay.as_secs();
        assert!(
            delay_secs >= 119 && delay_secs <= 121,
            "Delay was {}s",
            delay_secs
        );
    }

    /// Verify that default delay is used when no headers present.
    #[test]
    fn test_calculate_rate_limit_delay_default() {
        let delay = calculate_rate_limit_delay(None, None);
        assert_eq!(delay, Duration::from_secs(60));
    }

    /// Verify that invalid Retry-After falls back to reset time.
    #[test]
    fn test_calculate_rate_limit_delay_invalid_retry_after() {
        let future_timestamp = (Utc::now().timestamp() + 90).to_string();

        let delay = calculate_rate_limit_delay(Some("invalid"), Some(&future_timestamp));

        // Should fall back to reset time (90s)
        let delay_secs = delay.as_secs();
        assert!(
            delay_secs >= 89 && delay_secs <= 91,
            "Delay was {}s",
            delay_secs
        );
    }

    /// Verify that invalid reset time falls back to default.
    #[test]
    fn test_calculate_rate_limit_delay_invalid_reset() {
        let delay = calculate_rate_limit_delay(None, Some("not-a-timestamp"));

        // Should use default 60s
        assert_eq!(delay, Duration::from_secs(60));
    }

    /// Verify that past reset time falls back to default.
    #[test]
    fn test_calculate_rate_limit_delay_past_reset() {
        let past_timestamp = (Utc::now().timestamp() - 60).to_string(); // 1 minute ago

        let delay = calculate_rate_limit_delay(None, Some(&past_timestamp));

        // Should use default 60s since reset is in the past
        assert_eq!(delay, Duration::from_secs(60));
    }

    /// Verify handling of edge case: zero delay.
    #[test]
    fn test_calculate_rate_limit_delay_zero() {
        let delay = calculate_rate_limit_delay(Some("0"), None);

        assert_eq!(delay, Duration::from_secs(0));
    }

    /// Verify that Retry-After with HTTP date format works.
    #[test]
    fn test_calculate_rate_limit_delay_http_date() {
        let future = Utc::now() + chrono::Duration::seconds(180); // 3 minutes
        let http_date = future.to_rfc2822();

        let delay = calculate_rate_limit_delay(Some(&http_date), None);

        // Should be approximately 180 seconds
        let delay_secs = delay.as_secs();
        assert!(
            delay_secs >= 179 && delay_secs <= 181,
            "Delay was {}s",
            delay_secs
        );
    }

    /// Verify handling of very large delays.
    #[test]
    fn test_calculate_rate_limit_delay_large_value() {
        // 1 hour
        let delay = calculate_rate_limit_delay(Some("3600"), None);

        assert_eq!(delay, Duration::from_secs(3600));
    }
}

mod secondary_rate_limit_detection {
    use super::*;

    /// Verify that detect_secondary_rate_limit returns true for 403 with "rate limit" message.
    #[test]
    fn test_detect_secondary_rate_limit_rate_limit_message() {
        let body = r#"{"message":"You have exceeded a secondary rate limit. Please wait a few minutes before you try again."}"#;

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit returns true for 403 with "rate_limit" underscore format.
    #[test]
    fn test_detect_secondary_rate_limit_rate_limit_underscore() {
        let body = r#"{"message":"API rate_limit exceeded for user"}"#;

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit returns true for 403 with "abuse" message.
    #[test]
    fn test_detect_secondary_rate_limit_abuse_message() {
        let body = r#"{"message":"You have triggered an abuse detection mechanism. Please retry your request later."}"#;

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit returns true for 403 with "too many requests" message.
    #[test]
    fn test_detect_secondary_rate_limit_too_many_requests() {
        let body = r#"{"message":"Too many requests. Please slow down."}"#;

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit is case insensitive.
    #[test]
    fn test_detect_secondary_rate_limit_case_insensitive() {
        let body = r#"{"message":"RATE LIMIT exceeded"}"#;

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit returns false for 403 permission denied.
    #[test]
    fn test_detect_secondary_rate_limit_permission_denied() {
        let body = r#"{"message":"Resource not accessible by integration"}"#;

        assert!(!detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit returns false for 403 with unrelated message.
    #[test]
    fn test_detect_secondary_rate_limit_unrelated_403() {
        let body = r#"{"message":"This repository has been archived"}"#;

        assert!(!detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit returns false for non-403 status codes.
    #[test]
    fn test_detect_secondary_rate_limit_not_403() {
        let body = r#"{"message":"You have exceeded a secondary rate limit"}"#;

        // Should only detect on 403
        assert!(!detect_secondary_rate_limit(429, body));
        assert!(!detect_secondary_rate_limit(404, body));
        assert!(!detect_secondary_rate_limit(500, body));
        assert!(!detect_secondary_rate_limit(200, body));
    }

    /// Verify that detect_secondary_rate_limit returns false for empty body.
    #[test]
    fn test_detect_secondary_rate_limit_empty_body() {
        assert!(!detect_secondary_rate_limit(403, ""));
    }

    /// Verify that detect_secondary_rate_limit handles non-JSON body.
    #[test]
    fn test_detect_secondary_rate_limit_non_json_body() {
        let body = "Plain text error: rate limit exceeded";

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit matches substring in larger text.
    #[test]
    fn test_detect_secondary_rate_limit_substring_match() {
        let body = r#"
        {
            "message": "Request forbidden by administrative rules. The request has triggered GitHub's abuse detection mechanisms. Please wait before retrying.",
            "documentation_url": "https://docs.github.com"
        }
        "#;

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit with real GitHub secondary rate limit response.
    #[test]
    fn test_detect_secondary_rate_limit_real_github_response() {
        // Real GitHub secondary rate limit response format
        let body = r#"{
            "message": "You have exceeded a secondary rate limit. Please wait a few minutes before you try again.",
            "documentation_url": "https://docs.github.com/rest/overview/resources-in-the-rest-api#secondary-rate-limits"
        }"#;

        assert!(detect_secondary_rate_limit(403, body));
    }

    /// Verify that detect_secondary_rate_limit does not match partial words.
    #[test]
    fn test_detect_secondary_rate_limit_no_partial_match() {
        // "prelimit" should not match "rate limit"
        let body = r#"{"message":"Preliminary check failed"}"#;

        assert!(!detect_secondary_rate_limit(403, body));
    }
}
