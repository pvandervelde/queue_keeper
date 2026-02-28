//! Production [`SignatureValidator`] implementations for the service binary.
//!
//! This module provides concrete [`SignatureValidator`] implementations that
//! can be constructed from configuration at startup and injected into webhook
//! processor instances.
//!
//! # Implementations
//!
//! | Type | Use | Security |
//! |------|-----|---------|
//! | [`LiteralSignatureValidator`] | Dev / CI with a hard-coded secret | Not for production |
//!
//! Key Vault–backed validators are not included here because the secret is
//! fetched lazily at validation time through the Azure SDK and therefore live
//! in the `key_vault` module.

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use queue_keeper_core::webhook::{SecretError, SignatureValidator};
use queue_keeper_core::ValidationError;
use sha2::Sha256;
use tracing::{instrument, warn};

// ============================================================================
// LiteralSignatureValidator
// ============================================================================

/// A [`SignatureValidator`] backed by a plain-text secret embedded in configuration.
///
/// **Development and testing only.** In production, use a Key Vault–backed
/// implementation so that secrets are never stored in configuration files or
/// environment variables.
///
/// At startup, a `WARN` log line is emitted for every provider that uses a
/// literal secret so that operators are reminded to replace it before going
/// to production.
///
/// # Algorithm
///
/// Validates HMAC-SHA256 signatures in the format `sha256=<hex-digest>`,
/// which is the format used by GitHub and most modern webhook providers.
///
/// # Examples
///
/// ```rust,no_run
/// use queue_keeper_service::signature_validator::LiteralSignatureValidator;
/// use queue_keeper_core::webhook::SignatureValidator;
///
/// let validator = LiteralSignatureValidator::new("my-secret".to_string());
/// ```
pub struct LiteralSignatureValidator {
    secret: String,
}

impl LiteralSignatureValidator {
    /// Construct a new validator with the given literal secret.
    ///
    /// Emits a `WARN` log to remind operators that literal secrets are not
    /// production-safe.
    ///
    /// # Arguments
    ///
    /// * `secret` - The raw secret value (not Base64 or hex-encoded).
    pub fn new(secret: String) -> Self {
        warn!(
            "LiteralSignatureValidator is active — \
             literal secrets in configuration are not safe for production. \
             Migrate to Azure Key Vault before deploying."
        );
        Self { secret }
    }
}

impl std::fmt::Debug for LiteralSignatureValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiteralSignatureValidator")
            .field("secret", &"<REDACTED>")
            .finish()
    }
}

#[async_trait]
impl SignatureValidator for LiteralSignatureValidator {
    /// Validate a HMAC-SHA256 webhook signature.
    ///
    /// Accepts signatures in `sha256=<hex>` format (GitHub style). The
    /// `sha256=` prefix is stripped before comparison if present.
    ///
    /// The comparison is performed in constant time to prevent timing-based
    /// secret recovery attacks.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidFormat`] when the computed digest
    /// does not match the provided signature.
    ///
    /// Returns [`ValidationError::InvalidFormat`] when `signature` cannot be
    /// decoded as a hex string.
    #[instrument(skip(self, payload, secret_key), fields(sig_len = signature.len()))]
    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        secret_key: &str,
    ) -> Result<(), ValidationError> {
        type HmacSha256 = Hmac<Sha256>;

        let sig_bytes = {
            let hex_part = signature.strip_prefix("sha256=").unwrap_or(signature);
            hex::decode(hex_part).map_err(|_| ValidationError::InvalidFormat {
                field: "signature".to_string(),
                message: "signature is not valid hex".to_string(),
            })?
        };

        let mut mac =
            HmacSha256::new_from_slice(secret_key.as_bytes()).map_err(|_| {
                ValidationError::InvalidFormat {
                    field: "secret".to_string(),
                    message: "secret cannot be used as HMAC key".to_string(),
                }
            })?;
        mac.update(payload);

        mac.verify_slice(&sig_bytes).map_err(|_| ValidationError::InvalidFormat {
            field: "signature".to_string(),
            message: "HMAC-SHA256 digest does not match".to_string(),
        })
    }

    /// Return the literal secret for any event type.
    ///
    /// This implementation returns the same secret regardless of `event_type`
    /// because the literal configuration model does not support per-event
    /// secrets.
    #[instrument(skip(self))]
    async fn get_webhook_secret(&self, _event_type: &str) -> Result<String, SecretError> {
        Ok(self.secret.clone())
    }

    /// Returns `true`; the HMAC verification path uses constant-time comparison.
    fn supports_constant_time_comparison(&self) -> bool {
        true
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "signature_validator_tests.rs"]
mod tests;
