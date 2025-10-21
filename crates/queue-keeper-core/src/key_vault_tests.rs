//! Tests for key vault module.

use super::*;

#[test]
fn test_secret_name_validation() {
    // Valid names
    assert!(SecretName::new("queue-keeper-prod-github-webhook").is_ok());
    assert!(SecretName::new("app-dev-database").is_ok());

    // Invalid names
    assert!(SecretName::new("").is_err()); // Empty
    assert!(SecretName::new("invalid_chars!").is_err()); // Invalid characters
    assert!(SecretName::new("a".repeat(128)).is_err()); // Too long
}

#[test]
fn test_secret_name_components() {
    // Use service name without dashes to avoid ambiguity in component parsing
    let name = SecretName::from_components("queuekeeper", "prod", "github-webhook").unwrap();
    assert_eq!(name.as_str(), "queuekeeper-prod-github-webhook");

    let (service, env, purpose) = name.get_components().unwrap();
    assert_eq!(service, "queuekeeper");
    assert_eq!(env, "prod");
    assert_eq!(purpose, "github-webhook");
}

#[test]
fn test_secret_value_security() {
    let secret = SecretValue::from_string("sensitive-data".to_string());

    // Debug should not expose value
    let debug_output = format!("{:?}", secret);
    assert!(!debug_output.contains("sensitive-data"));
    assert!(debug_output.contains("[REDACTED]"));

    // Length should be available
    assert_eq!(secret.len(), 14);
}

#[test]
fn test_cached_secret_expiration() {
    let name = SecretName::new("test-secret").unwrap();
    let value = SecretValue::from_string("test-value".to_string());
    let now = Timestamp::now();

    let cached = CachedSecret {
        name,
        value,
        cached_at: now,
        expires_at: now.add_seconds(300),           // 5 minutes
        extended_expires_at: now.add_seconds(3600), // 1 hour
        version: Some("v1".to_string()),
    };

    assert!(!cached.is_expired()); // Should not be expired immediately
    assert!(!cached.is_extended_expired());
}

#[test]
fn test_standard_secrets() {
    let webhook_secret = StandardSecrets::github_webhook_secret("prod").unwrap();
    assert_eq!(webhook_secret.as_str(), "queue-keeper-prod-github-webhook");

    let db_conn = StandardSecrets::database_connection("dev").unwrap();
    assert_eq!(db_conn.as_str(), "queue-keeper-dev-database-conn");
}

#[test]
fn test_keyvault_error_transient() {
    assert!(KeyVaultError::ServiceUnavailable {
        message: "test".to_string()
    }
    .is_transient());

    assert!(!KeyVaultError::SecretNotFound {
        name: SecretName::new("test").unwrap()
    }
    .is_transient());
}

#[test]
fn test_keyvault_error_retry_delay() {
    let rate_limit_error = KeyVaultError::RateLimitExceeded {
        retry_after_seconds: 60,
    };
    assert_eq!(
        rate_limit_error.get_retry_delay(),
        Some(Duration::from_secs(60))
    );

    let not_found_error = KeyVaultError::SecretNotFound {
        name: SecretName::new("test").unwrap(),
    };
    assert_eq!(not_found_error.get_retry_delay(), None);
}
