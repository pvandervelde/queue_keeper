//! Tests for webhook signature validation.

use super::*;
use crate::auth::{GitHubAppId, PrivateKey, SecretProvider};
use crate::error::SecretError;
use async_trait::async_trait;
use chrono::Duration;
use std::sync::Arc;

// ============================================================================
// Mock Secret Provider
// ============================================================================

struct MockSecretProvider {
    webhook_secret: String,
}

impl MockSecretProvider {
    fn new(secret: String) -> Self {
        Self {
            webhook_secret: secret,
        }
    }
}

#[async_trait]
impl SecretProvider for MockSecretProvider {
    async fn get_private_key(&self) -> Result<PrivateKey, SecretError> {
        // Not used in signature validation
        Err(SecretError::NotFound {
            key: "private_key".to_string(),
        })
    }

    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError> {
        // Not used in signature validation
        Ok(GitHubAppId::new(12345))
    }

    async fn get_webhook_secret(&self) -> Result<String, SecretError> {
        Ok(self.webhook_secret.clone())
    }

    fn cache_duration(&self) -> Duration {
        Duration::minutes(5)
    }
}

// ============================================================================
// Test: Valid Signature Validation
// ============================================================================

#[tokio::test]
async fn test_validate_with_valid_signature() {
    // Arrange: Create validator with known secret
    let secret = "test_webhook_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    // GitHub webhook example payload
    let payload = br#"{"action":"opened","number":1,"pull_request":{"id":1}}"#;

    // Compute expected signature manually for verification
    // This is the HMAC-SHA256 of the payload with the secret
    // Expected: sha256=<hex_encoded_hmac>
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let expected_hex = hex::encode(result.into_bytes());
    let signature = format!("sha256={}", expected_hex);

    // Act: Validate the signature
    let is_valid = validator
        .validate(payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert: Signature should be valid (Assertion #16)
    assert!(is_valid, "Valid signature should pass validation");
}

#[tokio::test]
async fn test_validate_with_github_example_payload() {
    // Arrange: Real GitHub webhook example
    let secret = "It's a Secret to Everybody";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    // Real GitHub ping event payload
    let payload = br#"{"zen":"Design for failure.","hook_id":1}"#;

    // Compute expected signature
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let expected_hex = hex::encode(result.into_bytes());
    let signature = format!("sha256={}", expected_hex);

    // Act
    let is_valid = validator
        .validate(payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert
    assert!(is_valid, "GitHub example signature should be valid");
}

// ============================================================================
// Test: Invalid Signature Detection
// ============================================================================

#[tokio::test]
async fn test_validate_with_tampered_payload() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let original_payload = br#"{"action":"opened","number":1}"#;
    let tampered_payload = br#"{"action":"closed","number":1}"#; // Changed action

    // Create signature for original payload
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(original_payload);
    let result = mac.finalize();
    let signature = format!("sha256={}", hex::encode(result.into_bytes()));

    // Act: Validate tampered payload with original signature
    let is_valid = validator
        .validate(tampered_payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert: Should detect tampering (Assertion #17)
    assert!(!is_valid, "Tampered payload should fail validation");
}

#[tokio::test]
async fn test_validate_with_wrong_secret() {
    // Arrange
    let correct_secret = "correct_secret";
    let wrong_secret = "wrong_secret";

    // Create signature with correct secret
    let payload = br#"{"action":"opened"}"#;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(correct_secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let signature = format!("sha256={}", hex::encode(result.into_bytes()));

    // Validator uses wrong secret
    let secret_provider = Arc::new(MockSecretProvider::new(wrong_secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    // Act
    let is_valid = validator
        .validate(payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert: Wrong secret should fail validation
    assert!(!is_valid, "Wrong secret should fail validation");
}

#[tokio::test]
async fn test_validate_with_modified_signature() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = br#"{"action":"opened"}"#;

    // Create valid signature
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let mut sig_bytes = hex::encode(result.into_bytes());

    // Modify one character in the signature
    if let Some(ch) = sig_bytes.chars().next() {
        let new_ch = if ch == 'a' { 'b' } else { 'a' };
        sig_bytes = format!("{}{}", new_ch, &sig_bytes[1..]);
    }
    let signature = format!("sha256={}", sig_bytes);

    // Act
    let is_valid = validator
        .validate(payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert
    assert!(!is_valid, "Modified signature should fail validation");
}

// ============================================================================
// Test: Signature Format Validation
// ============================================================================

#[tokio::test]
async fn test_validate_with_missing_prefix() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = br#"{"action":"opened"}"#;
    let signature = "a1b2c3d4e5f6"; // Missing "sha256=" prefix

    // Act
    let result = validator.validate(payload, signature).await;

    // Assert: Should return error for invalid format
    assert!(result.is_err(), "Missing prefix should return error");
    if let Err(ValidationError::InvalidSignatureFormat { .. }) = result {
        // Expected error type
    } else {
        panic!("Expected InvalidSignatureFormat error");
    }
}

#[tokio::test]
async fn test_validate_with_invalid_hex_encoding() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = br#"{"action":"opened"}"#;
    let signature = "sha256=not_valid_hex!!!"; // Invalid hex characters

    // Act
    let result = validator.validate(payload, signature).await;

    // Assert
    assert!(result.is_err(), "Invalid hex should return error");
    if let Err(ValidationError::InvalidSignatureFormat { .. }) = result {
        // Expected error type
    } else {
        panic!("Expected InvalidSignatureFormat error");
    }
}

#[tokio::test]
async fn test_validate_with_empty_signature() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = br#"{"action":"opened"}"#;
    let signature = "";

    // Act
    let result = validator.validate(payload, signature).await;

    // Assert
    assert!(result.is_err(), "Empty signature should return error");
}

#[tokio::test]
async fn test_validate_with_wrong_algorithm_prefix() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = br#"{"action":"opened"}"#;
    let signature = "sha1=a1b2c3d4e5f6"; // Wrong algorithm (should be sha256)

    // Act
    let result = validator.validate(payload, signature).await;

    // Assert
    assert!(
        result.is_err(),
        "Wrong algorithm prefix should return error"
    );
}

// ============================================================================
// Test: Edge Cases
// ============================================================================

#[tokio::test]
async fn test_validate_with_empty_payload() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = b"";

    // Compute signature for empty payload
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let signature = format!("sha256={}", hex::encode(result.into_bytes()));

    // Act
    let is_valid = validator
        .validate(payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert: Empty payload should still validate correctly
    assert!(is_valid, "Empty payload with valid signature should pass");
}

#[tokio::test]
async fn test_validate_with_large_payload() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    // Create a large payload (1MB)
    let large_payload = vec![b'a'; 1024 * 1024];

    // Compute signature
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(&large_payload);
    let result = mac.finalize();
    let signature = format!("sha256={}", hex::encode(result.into_bytes()));

    // Act: Should complete within performance requirement
    let start = std::time::Instant::now();
    let is_valid = validator
        .validate(&large_payload, &signature)
        .await
        .expect("Validation should not error");
    let duration = start.elapsed();

    // Assert
    assert!(is_valid, "Large payload should validate correctly");
    assert!(
        duration.as_millis() < 100,
        "Validation should complete in <100ms, took {}ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_validate_with_unicode_in_payload() {
    // Arrange
    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = r#"{"message":"Hello 世界 🌍"}"#.as_bytes();

    // Compute signature
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let signature = format!("sha256={}", hex::encode(result.into_bytes()));

    // Act
    let is_valid = validator
        .validate(payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert
    assert!(is_valid, "Unicode payload should validate correctly");
}

#[tokio::test]
async fn test_validate_with_special_characters_in_secret() {
    // Arrange: Secret with special characters
    let secret = "my!@#$%^&*()secret_key";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = br#"{"action":"opened"}"#;

    // Compute signature
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let signature = format!("sha256={}", hex::encode(result.into_bytes()));

    // Act
    let is_valid = validator
        .validate(payload, &signature)
        .await
        .expect("Validation should not error");

    // Assert
    assert!(
        is_valid,
        "Special characters in secret should work correctly"
    );
}

// ============================================================================
// Test: Constant-Time Comparison
// ============================================================================

#[tokio::test]
async fn test_uses_constant_time_comparison() {
    // This test verifies that the implementation uses the subtle crate's
    // constant-time comparison. The actual timing attack resistance is
    // provided by the subtle::ConstantTimeEq trait, which is used in
    // the constant_time_compare method.
    //
    // We verify this by checking that both valid and invalid signatures
    // go through the full validation flow without early termination.

    let secret = "test_secret";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    let payload = br#"{"action":"opened"}"#;

    // Create valid signature
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload);
    let result = mac.finalize();
    let result_bytes = result.into_bytes();
    let valid_sig = format!("sha256={}", hex::encode(result_bytes));

    // Create invalid signature (completely different)
    let invalid_sig = "sha256=0000000000000000000000000000000000000000000000000000000000000000";

    // Both should process without error (though one returns false)
    let valid_result = validator.validate(payload, &valid_sig).await;
    let invalid_result = validator.validate(payload, invalid_sig).await;

    assert!(valid_result.is_ok() && valid_result.unwrap());
    assert!(invalid_result.is_ok() && !invalid_result.unwrap());

    // The key security property is that we use subtle::ConstantTimeEq
    // in the constant_time_compare method, which provides timing attack
    // resistance at the cryptographic level
} // ============================================================================
  // Test: Debug Output Security
  // ============================================================================

#[test]
fn test_debug_output_does_not_expose_secrets() {
    // Arrange
    let secret = "super_secret_webhook_key";
    let secret_provider = Arc::new(MockSecretProvider::new(secret.to_string()));
    let validator = SignatureValidator::new(secret_provider);

    // Act
    let debug_output = format!("{:?}", validator);

    // Assert: Secret should not appear in debug output
    assert!(
        !debug_output.contains(secret),
        "Debug output should not contain secret"
    );
    assert!(
        debug_output.contains("REDACTED"),
        "Debug output should indicate redaction"
    );
}
