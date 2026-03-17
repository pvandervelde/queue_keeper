//! Tests for [`LiteralSignatureValidator`] and [`KeyVaultSignatureValidator`].
//!
//! Verifies HMAC-SHA256 validation behaviour, secret retrieval, and the
//! constant-time comparison flag.

use super::*;
use hmac::{Hmac, Mac};
use sha2::Sha256;

// ============================================================================
// Helpers
// ============================================================================

/// Compute the HMAC-SHA256 of `payload` keyed by `secret` and return it as a
/// `sha256=<hex>` string — the exact format expected by providers.
fn compute_sha256_signature(secret: &str, payload: &[u8]) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

// ============================================================================
// validate_signature tests
// ============================================================================

mod validate_signature_tests {
    use super::*;

    /// A valid HMAC-SHA256 signature with the `sha256=` prefix must be accepted.
    #[tokio::test]
    async fn test_valid_signature_with_prefix_accepted() {
        let secret = "my-test-secret";
        let payload = b"hello world";
        let signature = compute_sha256_signature(secret, payload);

        let validator = LiteralSignatureValidator::new(secret.to_string());
        let result = validator
            .validate_signature(payload, &signature, secret)
            .await;

        assert!(result.is_ok(), "valid signature should be accepted");
    }

    /// A valid hex digest without the `sha256=` prefix must also be accepted.
    #[tokio::test]
    async fn test_valid_signature_without_prefix_accepted() {
        let secret = "my-test-secret";
        let payload = b"hello world";
        let full_sig = compute_sha256_signature(secret, payload);
        let no_prefix = full_sig.strip_prefix("sha256=").unwrap();

        let validator = LiteralSignatureValidator::new(secret.to_string());
        let result = validator
            .validate_signature(payload, no_prefix, secret)
            .await;

        assert!(
            result.is_ok(),
            "signature without prefix should be accepted"
        );
    }

    /// The wrong secret must cause validation to fail.
    #[tokio::test]
    async fn test_signature_wrong_secret_rejected() {
        let correct_secret = "correct-secret";
        let payload = b"some payload";
        let signature = compute_sha256_signature(correct_secret, payload);

        let validator = LiteralSignatureValidator::new("wrong-secret".to_string());
        let result = validator
            .validate_signature(payload, &signature, "wrong-secret")
            .await;

        assert!(result.is_err(), "signature with wrong secret should fail");
        matches!(result, Err(ValidationError::InvalidFormat { .. }));
    }

    /// A completely wrong hex digest (same length, altered bytes) must fail.
    #[tokio::test]
    async fn test_tampered_signature_rejected() {
        let secret = "my-secret";
        let payload = b"original payload";
        let tampered = format!("sha256={}", "0".repeat(64));

        let validator = LiteralSignatureValidator::new(secret.to_string());
        let result = validator
            .validate_signature(payload, &tampered, secret)
            .await;

        assert!(result.is_err(), "tampered signature should be rejected");
    }

    /// A signature that is not valid hex must return `InvalidFormat`.
    #[tokio::test]
    async fn test_non_hex_signature_returns_invalid_format() {
        let validator = LiteralSignatureValidator::new("secret".to_string());
        let result = validator
            .validate_signature(b"payload", "sha256=not-valid-hex!!", "secret")
            .await;

        assert!(result.is_err());
        assert!(
            matches!(result, Err(ValidationError::InvalidFormat { .. })),
            "expected InvalidFormat, got {:?}",
            result
        );
    }

    /// An empty payload still validates correctly (edge case).
    #[tokio::test]
    async fn test_empty_payload_validates() {
        let secret = "empty-payload-secret";
        let payload = b"";
        let signature = compute_sha256_signature(secret, payload);

        let validator = LiteralSignatureValidator::new(secret.to_string());
        let result = validator
            .validate_signature(payload, &signature, secret)
            .await;

        assert!(result.is_ok(), "empty-payload signature should validate");
    }
}

// ============================================================================
// get_webhook_secret tests
// ============================================================================

mod get_webhook_secret_tests {
    use super::*;

    /// The configured secret is returned for any event type.
    #[tokio::test]
    async fn test_returns_configured_secret_for_any_event() {
        let validator = LiteralSignatureValidator::new("super-secret".to_string());

        for event_type in ["push", "pull_request", "ping", "release", "unknown"] {
            let result = validator.get_webhook_secret(event_type).await;
            assert!(result.is_ok());
            assert_eq!(
                result.unwrap(),
                "super-secret",
                "expected configured secret for event type '{}'",
                event_type
            );
        }
    }
}

// ============================================================================
// supports_constant_time_comparison tests
// ============================================================================

mod supports_constant_time_comparison_tests {
    use super::*;

    /// The implementation must advertise constant-time comparison support
    /// because it delegates to `hmac::Mac::verify_slice` which is designed
    /// for constant-time execution.
    #[test]
    fn test_constant_time_comparison_is_supported() {
        let validator = LiteralSignatureValidator::new("any-secret".to_string());
        assert!(
            validator.supports_constant_time_comparison(),
            "LiteralSignatureValidator must support constant-time comparison"
        );
    }
}

// ============================================================================
// Debug formatting tests
// ============================================================================

mod debug_formatting_tests {
    use super::*;

    /// The `Debug` output must not reveal the secret.
    #[test]
    fn test_debug_redacts_secret() {
        let validator = LiteralSignatureValidator::new("top-secret-value".to_string());
        let debug_str = format!("{:?}", validator);

        assert!(
            !debug_str.contains("top-secret-value"),
            "secret must not appear in debug output; got: {}",
            debug_str
        );
        assert!(
            debug_str.contains("<REDACTED>"),
            "debug output should contain <REDACTED>; got: {}",
            debug_str
        );
    }
}

// ============================================================================
// KeyVaultSignatureValidator tests
// ============================================================================

mod key_vault_signature_validator_tests {
    use super::*;
    use queue_keeper_core::adapters::memory_key_vault::InMemoryKeyVaultProvider;
    use queue_keeper_core::key_vault::{KeyVaultError, SecretName, SecretValue};
    use std::collections::HashMap;

    fn secret_name(s: &str) -> SecretName {
        SecretName::new(s).expect("test secret name must be valid")
    }

    fn provider_with_secret(name: &str, value: &str) -> Arc<dyn KeyVaultProvider> {
        let mut secrets = HashMap::new();
        secrets.insert(
            secret_name(name),
            SecretValue::from_string(value.to_string()),
        );
        Arc::new(InMemoryKeyVaultProvider::with_secrets(secrets))
    }

    fn empty_provider() -> Arc<dyn KeyVaultProvider> {
        Arc::new(InMemoryKeyVaultProvider::new())
    }

    /// `get_webhook_secret` returns the value stored in Key Vault.
    #[tokio::test]
    async fn test_get_webhook_secret_returns_value_from_vault() {
        let provider = provider_with_secret("my-secret", "super-secret-value");
        let validator = KeyVaultSignatureValidator::new(provider, secret_name("my-secret"));

        let result = validator.get_webhook_secret("push").await;

        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), "super-secret-value");
    }

    /// `get_webhook_secret` maps `SecretNotFound` to `SecretError::NotFound`.
    #[tokio::test]
    async fn test_get_webhook_secret_maps_not_found() {
        let provider = empty_provider();
        let validator =
            KeyVaultSignatureValidator::new(provider, secret_name("nonexistent-secret"));

        let result = validator.get_webhook_secret("push").await;

        assert!(result.is_err());
        assert!(
            matches!(result, Err(SecretError::NotFound { .. })),
            "expected NotFound, got {:?}",
            result
        );
    }

    /// `get_webhook_secret` maps `AccessDenied` to `SecretError::AccessDenied`.
    #[tokio::test]
    async fn test_get_webhook_secret_maps_access_denied() {
        // Use the private helper directly to test the mapping function
        let name = secret_name("my-secret");
        let kv_error = KeyVaultError::AccessDenied {
            name: name.clone(),
            reason: "forbidden".to_string(),
        };
        let result = map_key_vault_error(kv_error, &name);
        assert!(
            matches!(result, SecretError::AccessDenied { .. }),
            "expected AccessDenied, got {:?}",
            result
        );
    }

    /// `get_webhook_secret` maps `AuthenticationFailed` to `SecretError::AccessDenied`.
    #[tokio::test]
    async fn test_get_webhook_secret_maps_auth_failed_to_access_denied() {
        let name = secret_name("my-secret");
        let kv_error = KeyVaultError::AuthenticationFailed {
            message: "token expired".to_string(),
        };
        let result = map_key_vault_error(kv_error, &name);
        assert!(
            matches!(result, SecretError::AccessDenied { .. }),
            "expected AccessDenied, got {:?}",
            result
        );
    }

    /// `get_webhook_secret` maps `ServiceUnavailable` to `SecretError::ProviderUnavailable`.
    #[tokio::test]
    async fn test_get_webhook_secret_maps_service_unavailable() {
        let name = secret_name("my-secret");
        let kv_error = KeyVaultError::ServiceUnavailable {
            message: "vault down".to_string(),
        };
        let result = map_key_vault_error(kv_error, &name);
        assert!(
            matches!(result, SecretError::ProviderUnavailable(_)),
            "expected ProviderUnavailable, got {:?}",
            result
        );
    }

    /// `validate_signature` with a correct HMAC-SHA256 signature returns `Ok`.
    #[tokio::test]
    async fn test_validate_signature_with_correct_hmac_passes() {
        let secret = "webhook-secret";
        let payload = b"hello from webhook";
        let signature = compute_sha256_signature(secret, payload);

        let provider = provider_with_secret("my-secret", secret);
        let validator = KeyVaultSignatureValidator::new(provider, secret_name("my-secret"));

        let result = validator.validate_signature(payload, &signature, secret).await;
        assert!(result.is_ok(), "valid signature should be accepted");
    }

    /// `validate_signature` with a wrong secret key returns an error.
    #[tokio::test]
    async fn test_validate_signature_with_wrong_key_fails() {
        let correct_secret = "correct-secret";
        let payload = b"some payload";
        let signature = compute_sha256_signature(correct_secret, payload);

        let provider = provider_with_secret("my-secret", correct_secret);
        let validator = KeyVaultSignatureValidator::new(provider, secret_name("my-secret"));

        // Validate with wrong key — same as Literal behaviour
        let result = validator
            .validate_signature(payload, &signature, "wrong-secret")
            .await;
        assert!(result.is_err(), "wrong key should fail validation");
    }

    /// `supports_constant_time_comparison` is always `true`.
    #[test]
    fn test_supports_constant_time_comparison_is_true() {
        let provider = empty_provider();
        let validator =
            KeyVaultSignatureValidator::new(provider, secret_name("any-secret"));
        assert!(validator.supports_constant_time_comparison());
    }

    /// `Debug` output shows the secret_name but not the secret value.
    #[test]
    fn test_debug_shows_secret_name_not_value() {
        let provider = provider_with_secret("my-kv-secret", "top-secret");
        let validator =
            KeyVaultSignatureValidator::new(provider, secret_name("my-kv-secret"));
        let debug_str = format!("{:?}", validator);

        assert!(
            debug_str.contains("my-kv-secret"),
            "debug should show the KV lookup key (not sensitive): {debug_str}"
        );
        assert!(
            !debug_str.contains("top-secret"),
            "debug must never include the actual secret value: {debug_str}"
        );
    }
}
