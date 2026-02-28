//! Tests for [`LiteralSignatureValidator`].
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

        assert!(result.is_ok(), "signature without prefix should be accepted");
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
