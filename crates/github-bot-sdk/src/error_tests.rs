//! Tests for error types.

use super::*;

#[test]
fn test_auth_error_transience() {
    // Non-transient errors
    assert!(!AuthError::InvalidCredentials.is_transient());
    assert!(!AuthError::InstallationNotFound {
        installation_id: 123
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

#[test]
fn test_auth_error_should_retry_alias() {
    // Verify should_retry() is an alias for is_transient()
    let transient = AuthError::TokenExpired;
    assert_eq!(transient.should_retry(), transient.is_transient());

    let non_transient = AuthError::InvalidCredentials;
    assert_eq!(non_transient.should_retry(), non_transient.is_transient());
}

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
