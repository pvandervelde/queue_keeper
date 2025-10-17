//! Tests for error types.

use super::*;
use crate::auth::InstallationId;

/// Verify that AuthError variants correctly classify transient vs non-transient conditions.
///
/// Tests the `is_transient()` method across all AuthError variants to ensure:
/// - Non-retryable errors (invalid credentials, missing installations, permissions) return false
/// - Retryable errors (expired tokens, network issues, server errors, rate limits) return true
/// - Client errors (4xx except 429) are classified as non-transient
#[test]
fn test_auth_error_transience() {
    // Non-transient errors
    assert!(!AuthError::InvalidCredentials.is_transient());
    assert!(!AuthError::InstallationNotFound {
        installation_id: InstallationId::new(123)
    }
    .is_transient());
    assert!(!AuthError::InsufficientPermissions {
        permission: "write".to_string()
    }
    .is_transient());

    // Transient errors
    assert!(AuthError::TokenExpired.is_transient());
    assert!(AuthError::NetworkError("timeout".to_string()).is_transient());
    assert!(AuthError::GitHubApiError {
        status: 500,
        message: "server error".to_string()
    }
    .is_transient());
    assert!(AuthError::GitHubApiError {
        status: 429,
        message: "rate limited".to_string()
    }
    .is_transient());

    // Non-transient API errors
    assert!(!AuthError::GitHubApiError {
        status: 400,
        message: "bad request".to_string()
    }
    .is_transient());
    assert!(!AuthError::GitHubApiError {
        status: 404,
        message: "not found".to_string()
    }
    .is_transient());
}

/// Verify that AuthError provides appropriate retry delay recommendations.
///
/// Tests the `retry_after()` method to ensure:
/// - Rate limit errors (429) recommend a 1-minute delay
/// - Network errors recommend a 5-second delay
/// - Non-retryable errors return None (no delay recommended)
#[test]
fn test_auth_error_retry_after() {
    let rate_limit_error = AuthError::GitHubApiError {
        status: 429,
        message: "rate limited".to_string(),
    };
    assert_eq!(
        rate_limit_error.retry_after(),
        Some(chrono::Duration::minutes(1))
    );

    let network_error = AuthError::NetworkError("connection failed".to_string());
    assert_eq!(
        network_error.retry_after(),
        Some(chrono::Duration::seconds(5))
    );

    let invalid_creds = AuthError::InvalidCredentials;
    assert_eq!(invalid_creds.retry_after(), None);
}

/// Verify that SecretError variants correctly classify transient vs non-transient conditions.
///
/// Tests the `is_transient()` method across all SecretError variants to ensure:
/// - NotFound, AccessDenied, and InvalidFormat errors are non-transient
/// - ProviderUnavailable is the only transient error (infrastructure issue)
#[test]
fn test_secret_error_transience() {
    assert!(!SecretError::NotFound {
        key: "test".to_string()
    }
    .is_transient());
    assert!(!SecretError::AccessDenied {
        key: "test".to_string()
    }
    .is_transient());
    assert!(!SecretError::InvalidFormat {
        key: "test".to_string()
    }
    .is_transient());
    assert!(SecretError::ProviderUnavailable("vault down".to_string()).is_transient());
}

/// Verify that ApiError variants correctly classify transient vs non-transient conditions.
///
/// Tests the `is_transient()` method across all ApiError variants to ensure:
/// - Server errors (5xx), rate limits (429), and timeouts are transient
/// - Client errors (4xx except 429) and authentication/authorization failures are non-transient
/// - Network/transport errors are considered transient
#[test]
fn test_api_error_transience() {
    assert!(ApiError::HttpError {
        status: 500,
        message: "server error".to_string()
    }
    .is_transient());
    assert!(ApiError::HttpError {
        status: 503,
        message: "service unavailable".to_string()
    }
    .is_transient());
    assert!(ApiError::HttpError {
        status: 429,
        message: "rate limited".to_string()
    }
    .is_transient());
    assert!(ApiError::RateLimitExceeded {
        reset_at: chrono::Utc::now()
    }
    .is_transient());
    assert!(ApiError::Timeout.is_transient());

    assert!(!ApiError::HttpError {
        status: 400,
        message: "bad request".to_string()
    }
    .is_transient());
    assert!(!ApiError::HttpError {
        status: 404,
        message: "not found".to_string()
    }
    .is_transient());
    assert!(!ApiError::AuthenticationFailed.is_transient());
    assert!(!ApiError::AuthorizationFailed.is_transient());
    assert!(!ApiError::NotFound.is_transient());
}

/// Verify that ValidationError produces correct error messages for each variant.
///
/// Tests the Display implementation to ensure:
/// - Required field errors format as "Required field missing: {field}"
/// - Invalid format errors include both field name and reason
/// - Out of range errors include both field name and constraint details
#[test]
fn test_validation_error_messages() {
    let required = ValidationError::Required {
        field: "app_id".to_string(),
    };
    assert_eq!(required.to_string(), "Required field missing: app_id");

    let invalid_format = ValidationError::InvalidFormat {
        field: "private_key".to_string(),
        message: "not PEM format".to_string(),
    };
    assert_eq!(
        invalid_format.to_string(),
        "Invalid format for private_key: not PEM format"
    );

    let out_of_range = ValidationError::OutOfRange {
        field: "expiry".to_string(),
        message: "exceeds 10 minutes".to_string(),
    };
    assert_eq!(
        out_of_range.to_string(),
        "Value out of range for expiry: exceeds 10 minutes"
    );
}

/// Verify that `should_retry()` is a correct alias for `is_transient()`.
///
/// Tests that both methods return the same result for both transient
/// and non-transient errors, ensuring consistency in retry logic.
#[test]
fn test_auth_error_should_retry_alias() {
    // Verify should_retry() is an alias for is_transient()
    let transient = AuthError::TokenExpired;
    assert_eq!(transient.should_retry(), transient.is_transient());

    let non_transient = AuthError::InvalidCredentials;
    assert_eq!(non_transient.should_retry(), non_transient.is_transient());
}

/// Verify that CacheError correctly converts from serde_json::Error.
///
/// Tests the From trait implementation to ensure JSON parsing errors
/// are properly wrapped as CacheError::Serialization variants.
#[test]
fn test_cache_error_serialization() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid json");
    assert!(json_err.is_err());

    let cache_err = CacheError::from(json_err.unwrap_err());
    match cache_err {
        CacheError::Serialization(_) => (),
        _ => panic!("Expected Serialization error"),
    }
}
