//! Tests for rate limit tracking functionality.

use super::*;
use crate::auth::InstallationId;
use chrono::{Duration, Utc};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

mod rate_limit_tests {
    use super::*;

    /// Verify that RateLimit correctly stores and retrieves basic information.
    ///
    /// Creates a rate limit and verifies all fields are accessible.
    #[test]
    fn test_rate_limit_creation_and_accessors() {
        let reset_time = Utc::now() + Duration::hours(1);
        let rate_limit = RateLimit::new(5000, 4500, reset_time, "core");

        assert_eq!(rate_limit.limit(), 5000);
        assert_eq!(rate_limit.remaining(), 4500);
        assert_eq!(rate_limit.reset_at(), reset_time);
        assert_eq!(rate_limit.resource(), "core");
    }

    /// Verify that is_exhausted returns true when remaining is 0.
    #[test]
    fn test_rate_limit_is_exhausted_when_zero_remaining() {
        let rate_limit = RateLimit::new(5000, 0, Utc::now(), "core");
        assert!(rate_limit.is_exhausted());
    }

    /// Verify that is_exhausted returns false when requests remain.
    #[test]
    fn test_rate_limit_not_exhausted_when_remaining() {
        let rate_limit = RateLimit::new(5000, 100, Utc::now(), "core");
        assert!(!rate_limit.is_exhausted());
    }

    /// Verify that is_near_exhaustion correctly detects when below margin threshold.
    ///
    /// With 10% margin on 5000 limit (500 threshold), 400 remaining should trigger.
    #[test]
    fn test_rate_limit_near_exhaustion_below_margin() {
        let rate_limit = RateLimit::new(5000, 400, Utc::now(), "core");

        // 400 is below 10% of 5000 (500)
        assert!(rate_limit.is_near_exhaustion(0.1));
    }

    /// Verify that is_near_exhaustion returns false when above margin threshold.
    ///
    /// With 10% margin on 5000 limit (500 threshold), 600 remaining should be safe.
    #[test]
    fn test_rate_limit_not_near_exhaustion_above_margin() {
        let rate_limit = RateLimit::new(5000, 600, Utc::now(), "core");

        // 600 is above 10% of 5000 (500)
        assert!(!rate_limit.is_near_exhaustion(0.1));
    }

    /// Verify that has_reset returns false when reset time is in the future.
    #[test]
    fn test_rate_limit_has_not_reset_before_time() {
        let reset_time = Utc::now() + Duration::hours(1);
        let rate_limit = RateLimit::new(5000, 4500, reset_time, "core");

        assert!(!rate_limit.has_reset());
    }

    /// Verify that has_reset returns true when reset time is in the past.
    #[test]
    fn test_rate_limit_has_reset_after_time() {
        let reset_time = Utc::now() - Duration::minutes(1);
        let rate_limit = RateLimit::new(5000, 0, reset_time, "core");

        assert!(rate_limit.has_reset());
    }
}

mod parse_rate_limit_tests {
    use super::*;

    /// Verify that parse_rate_limit_from_headers successfully parses valid headers.
    ///
    /// GitHub API returns X-RateLimit-* headers with rate limit information.
    #[test]
    fn test_parse_valid_rate_limit_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4999"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-resource"),
            HeaderValue::from_static("core"),
        );

        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_some());
        let rate_limit = rate_limit.unwrap();
        assert_eq!(rate_limit.limit(), 5000);
        assert_eq!(rate_limit.remaining(), 4999);
        assert_eq!(rate_limit.resource(), "core");
    }

    /// Verify that parse_rate_limit_from_headers defaults to "core" resource when header missing.
    #[test]
    fn test_parse_rate_limit_defaults_to_core_resource() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4999"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );
        // No x-ratelimit-resource header

        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_some());
        let rate_limit = rate_limit.unwrap();
        assert_eq!(rate_limit.resource(), "core");
    }

    /// Verify that parse_rate_limit_from_headers returns None when required headers are missing.
    #[test]
    fn test_parse_rate_limit_returns_none_when_headers_missing() {
        let mut headers = HeaderMap::new();
        // Only include limit, missing remaining and reset
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );

        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_none());
    }

    /// Verify that parse_rate_limit_from_headers returns None when headers have invalid format.
    #[test]
    fn test_parse_rate_limit_returns_none_when_headers_invalid() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("not-a-number"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4999"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_static("1234567890"),
        );

        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_none());
    }
}

mod rate_limiter_tests {
    use super::*;

    /// Verify that RateLimiter correctly creates with specified margin.
    #[test]
    fn test_rate_limiter_creation_with_margin() {
        let limiter = RateLimiter::new(0.15);
        assert_eq!(limiter.margin, 0.15);
    }

    /// Verify that RateLimiter clamps margin to valid range.
    #[test]
    fn test_rate_limiter_clamps_margin_to_valid_range() {
        let limiter_too_low = RateLimiter::new(-0.5);
        assert_eq!(limiter_too_low.margin, 0.0);

        let limiter_too_high = RateLimiter::new(1.5);
        assert_eq!(limiter_too_high.margin, 1.0);
    }

    /// Verify that update_from_headers stores rate limit information for app context.
    #[test]
    fn test_rate_limiter_updates_from_headers_app_context() {
        let limiter = RateLimiter::new(0.1);

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4500"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-resource"),
            HeaderValue::from_static("core"),
        );

        limiter.update_from_headers(&RateLimitContext::App, &headers);

        let rate_limit = limiter.get_limit(&RateLimitContext::App, "core");
        assert!(rate_limit.is_some());
        let rate_limit = rate_limit.unwrap();
        assert_eq!(rate_limit.limit(), 5000);
        assert_eq!(rate_limit.remaining(), 4500);
    }

    /// Verify that update_from_headers stores rate limit information for installation context.
    #[test]
    fn test_rate_limiter_updates_from_headers_installation_context() {
        let limiter = RateLimiter::new(0.1);
        let install_id = InstallationId::new(12345);

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("3500"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-resource"),
            HeaderValue::from_static("core"),
        );

        limiter.update_from_headers(&RateLimitContext::Installation(install_id), &headers);

        let rate_limit = limiter.get_limit(&RateLimitContext::Installation(install_id), "core");
        assert!(rate_limit.is_some());
        let rate_limit = rate_limit.unwrap();
        assert_eq!(rate_limit.limit(), 5000);
        assert_eq!(rate_limit.remaining(), 3500);
    }

    /// Verify that rate limits are tracked separately for different installations.
    ///
    /// Multiple installations should have independent rate limit tracking.
    #[test]
    fn test_rate_limiter_tracks_installations_separately() {
        let limiter = RateLimiter::new(0.1);
        let install_1 = InstallationId::new(111);
        let install_2 = InstallationId::new(222);

        // Setup rate limit for installation 1
        let mut headers_1 = HeaderMap::new();
        headers_1.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers_1.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4000"),
        );
        headers_1.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::Installation(install_1), &headers_1);

        // Setup different rate limit for installation 2
        let mut headers_2 = HeaderMap::new();
        headers_2.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers_2.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("2000"),
        );
        headers_2.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::Installation(install_2), &headers_2);

        // Verify both installations have their own limits
        let limit_1 = limiter
            .get_limit(&RateLimitContext::Installation(install_1), "core")
            .unwrap();
        let limit_2 = limiter
            .get_limit(&RateLimitContext::Installation(install_2), "core")
            .unwrap();

        assert_eq!(limit_1.remaining(), 4000);
        assert_eq!(limit_2.remaining(), 2000);
    }

    /// Verify that app-level and installation-level rate limits are tracked separately.
    ///
    /// App-level operations use JWT, installation-level use installation tokens.
    /// They should have independent rate limits.
    #[test]
    fn test_rate_limiter_tracks_app_and_installation_separately() {
        let limiter = RateLimiter::new(0.1);
        let install_id = InstallationId::new(12345);

        // Setup app-level rate limit
        let mut app_headers = HeaderMap::new();
        app_headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        app_headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4500"),
        );
        app_headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::App, &app_headers);

        // Setup installation-level rate limit
        let mut install_headers = HeaderMap::new();
        install_headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        install_headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("3000"),
        );
        install_headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(
            &RateLimitContext::Installation(install_id),
            &install_headers,
        );

        // Verify both contexts have their own limits
        let app_limit = limiter.get_limit(&RateLimitContext::App, "core").unwrap();
        let install_limit = limiter
            .get_limit(&RateLimitContext::Installation(install_id), "core")
            .unwrap();

        assert_eq!(app_limit.remaining(), 4500);
        assert_eq!(install_limit.remaining(), 3000);
    }

    /// Verify that can_proceed returns true when rate limit is healthy for app context.
    #[test]
    fn test_rate_limiter_can_proceed_when_healthy_app() {
        let limiter = RateLimiter::new(0.1);

        // Setup rate limit with plenty of requests remaining
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::App, &headers);

        assert!(limiter.can_proceed(&RateLimitContext::App, "core"));
    }

    /// Verify that can_proceed returns true when rate limit is healthy for installation context.
    #[test]
    fn test_rate_limiter_can_proceed_when_healthy_installation() {
        let limiter = RateLimiter::new(0.1);
        let install_id = InstallationId::new(12345);

        // Setup rate limit with plenty of requests remaining
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::Installation(install_id), &headers);

        assert!(limiter.can_proceed(&RateLimitContext::Installation(install_id), "core"));
    }

    /// Verify that can_proceed returns false when near rate limit exhaustion.
    ///
    /// With 10% margin on 5000 limit, 400 remaining should block requests.
    #[test]
    fn test_rate_limiter_blocks_when_near_exhaustion() {
        let limiter = RateLimiter::new(0.1);

        // Setup rate limit near exhaustion (below margin)
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("400"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::App, &headers);

        assert!(!limiter.can_proceed(&RateLimitContext::App, "core"));
    }

    /// Verify that can_proceed returns false when rate limit is exhausted.
    #[test]
    fn test_rate_limiter_blocks_when_exhausted() {
        let limiter = RateLimiter::new(0.1);

        // Setup exhausted rate limit
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("0"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::App, &headers);

        assert!(!limiter.can_proceed(&RateLimitContext::App, "core"));
    }

    /// Verify that can_proceed returns true when no rate limit data exists yet.
    ///
    /// Before first API call, we don't know the rate limit, so allow the request.
    #[test]
    fn test_rate_limiter_allows_when_no_data() {
        let limiter = RateLimiter::new(0.1);

        // No update_from_headers called yet
        assert!(limiter.can_proceed(&RateLimitContext::App, "core"));
    }

    /// Verify that can_proceed returns true when rate limit has reset.
    #[test]
    fn test_rate_limiter_allows_after_reset() {
        let limiter = RateLimiter::new(0.1);

        // Setup rate limit that has already reset (past reset time)
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("0"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() - 60).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::App, &headers);

        // Should allow because rate limit has reset
        assert!(limiter.can_proceed(&RateLimitContext::App, "core"));
    }

    /// Verify that can_proceed checks only the specified context.
    ///
    /// One installation being rate limited shouldn't affect another installation.
    #[test]
    fn test_rate_limiter_contexts_independent() {
        let limiter = RateLimiter::new(0.1);
        let install_1 = InstallationId::new(111);
        let install_2 = InstallationId::new(222);

        // Setup installation 1 as exhausted
        let mut headers_1 = HeaderMap::new();
        headers_1.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers_1.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("0"),
        );
        headers_1.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::Installation(install_1), &headers_1);

        // Setup installation 2 as healthy
        let mut headers_2 = HeaderMap::new();
        headers_2.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers_2.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4000"),
        );
        headers_2.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::Installation(install_2), &headers_2);

        // Installation 1 should be blocked
        assert!(!limiter.can_proceed(&RateLimitContext::Installation(install_1), "core"));

        // Installation 2 should be allowed
        assert!(limiter.can_proceed(&RateLimitContext::Installation(install_2), "core"));
    }

    /// Verify that get_limit returns None when no data exists for context.
    #[test]
    fn test_rate_limiter_get_limit_returns_none_when_no_data() {
        let limiter = RateLimiter::new(0.1);

        assert!(limiter.get_limit(&RateLimitContext::App, "core").is_none());
    }

    /// Verify that get_limit returns stored rate limit information.
    #[test]
    fn test_rate_limiter_get_limit_returns_stored_data() {
        let limiter = RateLimiter::new(0.1);

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4500"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&(Utc::now().timestamp() + 3600).to_string()).unwrap(),
        );

        limiter.update_from_headers(&RateLimitContext::App, &headers);

        let rate_limit = limiter.get_limit(&RateLimitContext::App, "core");
        assert!(rate_limit.is_some());
        let rate_limit = rate_limit.unwrap();
        assert_eq!(rate_limit.limit(), 5000);
        assert_eq!(rate_limit.remaining(), 4500);
    }
}
