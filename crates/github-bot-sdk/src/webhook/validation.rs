//! Webhook signature validation implementation.
//!
//! Provides HMAC-SHA256 signature validation for GitHub webhooks using
//! constant-time comparison to prevent timing attacks.

use crate::auth::SecretProvider;
use crate::error::ValidationError;
use std::sync::Arc;

/// Validates GitHub webhook signatures using HMAC-SHA256.
///
/// This validator ensures webhook payloads are authentic by verifying
/// the `X-Hub-Signature-256` header against the payload using the
/// webhook secret.
///
/// # Security
///
/// - Uses constant-time comparison to prevent timing attacks
/// - Never logs secrets or signature values
/// - Validates signature format before HMAC computation
/// - Completes validation in under 100ms
///
/// # Examples
///
/// ```rust,no_run
/// use github_bot_sdk::webhook::SignatureValidator;
/// use github_bot_sdk::auth::SecretProvider;
/// use std::sync::Arc;
///
/// # async fn example(secret_provider: Arc<dyn SecretProvider>) -> Result<(), Box<dyn std::error::Error>> {
/// let validator = SignatureValidator::new(secret_provider);
///
/// let payload = b"{\"action\":\"opened\",\"number\":1}";
/// let signature = "sha256=a1b2c3d4...";  // From X-Hub-Signature-256 header
///
/// if validator.validate(payload, signature).await? {
///     println!("Valid webhook");
/// } else {
///     println!("Invalid signature - rejecting webhook");
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SignatureValidator {
    secrets: Arc<dyn SecretProvider>,
}

impl SignatureValidator {
    /// Create a new signature validator.
    ///
    /// # Arguments
    ///
    /// * `secrets` - Provider for retrieving webhook secrets
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use github_bot_sdk::webhook::SignatureValidator;
    /// # use github_bot_sdk::auth::SecretProvider;
    /// # use std::sync::Arc;
    /// # fn example(secret_provider: Arc<dyn SecretProvider>) {
    /// let validator = SignatureValidator::new(secret_provider);
    /// # }
    /// ```
    pub fn new(secrets: Arc<dyn SecretProvider>) -> Self {
        Self { secrets }
    }

    /// Validate a webhook signature.
    ///
    /// Verifies that the signature matches the HMAC-SHA256 of the payload
    /// using the webhook secret. Uses constant-time comparison to prevent
    /// timing attacks.
    ///
    /// # Arguments
    ///
    /// * `payload` - The raw webhook payload bytes
    /// * `signature` - The signature from X-Hub-Signature-256 header (format: "sha256=<hex>")
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Signature is valid
    /// * `Ok(false)` - Signature is invalid (tampered payload or wrong secret)
    /// * `Err` - Validation error (malformed signature, secret retrieval failure)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use github_bot_sdk::webhook::SignatureValidator;
    /// # use github_bot_sdk::auth::SecretProvider;
    /// # use std::sync::Arc;
    /// # async fn example(validator: SignatureValidator) -> Result<(), Box<dyn std::error::Error>> {
    /// let payload = b"{\"action\":\"opened\"}";
    /// let signature = "sha256=5c4a...";
    ///
    /// match validator.validate(payload, signature).await {
    ///     Ok(true) => println!("Valid webhook"),
    ///     Ok(false) => println!("Invalid signature"),
    ///     Err(e) => println!("Validation error: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn validate(&self, payload: &[u8], signature: &str) -> Result<bool, ValidationError> {
        // Parse the signature header
        let signature_bytes = self.parse_signature(signature)?;

        // Get webhook secret from provider
        let secret = self.secrets.get_webhook_secret().await.map_err(|e| {
            ValidationError::InvalidSignatureFormat {
                message: format!("Failed to retrieve webhook secret: {}", e),
            }
        })?;

        // Compute expected HMAC
        let expected_hmac = self.compute_hmac(payload, &secret)?;

        // Constant-time comparison
        let is_valid = self.constant_time_compare(&signature_bytes, &expected_hmac);

        Ok(is_valid)
    }

    /// Parse GitHub signature format.
    ///
    /// Extracts hex-encoded signature bytes from GitHub's "sha256=<hex>" format.
    ///
    /// # Arguments
    ///
    /// * `signature` - The signature header value
    ///
    /// # Returns
    ///
    /// The decoded signature bytes
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::InvalidSignatureFormat` if:
    /// - Signature doesn't start with "sha256="
    /// - Hex encoding is invalid
    fn parse_signature(&self, signature: &str) -> Result<Vec<u8>, ValidationError> {
        // Check for "sha256=" prefix
        const PREFIX: &str = "sha256=";
        if !signature.starts_with(PREFIX) {
            return Err(ValidationError::InvalidSignatureFormat {
                message: format!(
                    "Signature must start with '{}', got: '{}'",
                    PREFIX,
                    signature.chars().take(10).collect::<String>()
                ),
            });
        }

        // Extract hex portion
        let hex_signature = &signature[PREFIX.len()..];

        // Decode hex to bytes
        hex::decode(hex_signature).map_err(|e| ValidationError::InvalidSignatureFormat {
            message: format!("Invalid hex encoding in signature: {}", e),
        })
    }

    /// Compute HMAC-SHA256 signature.
    ///
    /// Generates the expected HMAC-SHA256 signature for the payload
    /// using the webhook secret.
    ///
    /// # Arguments
    ///
    /// * `payload` - The webhook payload bytes
    /// * `secret` - The webhook secret
    ///
    /// # Returns
    ///
    /// The computed HMAC signature bytes
    fn compute_hmac(&self, payload: &[u8], secret: &str) -> Result<Vec<u8>, ValidationError> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        // Create HMAC instance with secret
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| {
            ValidationError::HmacError {
                message: format!("Failed to create HMAC instance: {}", e),
            }
        })?;

        // Update with payload
        mac.update(payload);

        // Finalize and return bytes
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// Constant-time comparison of signatures.
    ///
    /// Compares two byte slices in constant time to prevent timing attacks.
    /// Uses the `subtle` crate for cryptographically secure comparison.
    ///
    /// # Arguments
    ///
    /// * `a` - First signature
    /// * `b` - Second signature
    ///
    /// # Returns
    ///
    /// `true` if signatures match, `false` otherwise
    fn constant_time_compare(&self, a: &[u8], b: &[u8]) -> bool {
        use subtle::ConstantTimeEq;

        // Check length first (this is safe to do in non-constant time)
        if a.len() != b.len() {
            return false;
        }

        // Perform constant-time comparison
        a.ct_eq(b).into()
    }
}

// Security: Don't expose secrets in debug output
impl std::fmt::Debug for SignatureValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignatureValidator")
            .field("secrets", &"<REDACTED>")
            .finish()
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
