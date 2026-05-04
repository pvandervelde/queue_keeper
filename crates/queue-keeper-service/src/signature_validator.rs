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
//! | [`KeyVaultSignatureValidator`] | Production with Azure Key Vault | Production-safe |

use async_trait::async_trait;
use hmac::{Hmac, KeyInit, Mac};
use queue_keeper_core::key_vault::{KeyVaultError, KeyVaultProvider, SecretName};
use queue_keeper_core::webhook::{SecretError, SignatureValidator};
use queue_keeper_core::ValidationError;
use sha2::Sha256;
use std::sync::Arc;
use tracing::{instrument, warn};

// ============================================================================
// Private helpers
// ============================================================================

/// Validate an HMAC-SHA256 webhook signature in constant time.
///
/// Accepts `sha256=<hex>` format; the `sha256=` prefix is stripped before
/// the hex is decoded. Comparison is performed via `hmac::Mac::verify_slice`
/// which uses constant-time equality to prevent timing attacks.
///
/// Note: `github-bot-sdk` contains equivalent HMAC logic in
/// `webhook::validation::SignatureValidator::compute_hmac`, but that method
/// is private and the SDK's validator is bound to its own `SecretProvider`
/// trait and error types. This helper implements the local
/// `queue_keeper_core::webhook::SignatureValidator` contract and must also
/// serve generic (non-GitHub) providers, so the SDK code cannot be reused.
///
/// # Errors
///
/// - [`ValidationError::InvalidFormat`] — `signature` is not valid hex.
/// - [`ValidationError::InvalidFormat`] — the HMAC digest does not match.
fn validate_hmac_sha256(
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

    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes()).map_err(|_| {
        ValidationError::InvalidFormat {
            field: "secret".to_string(),
            message: "secret cannot be used as HMAC key".to_string(),
        }
    })?;
    mac.update(payload);

    mac.verify_slice(&sig_bytes)
        .map_err(|_| ValidationError::InvalidFormat {
            field: "signature".to_string(),
            message: "HMAC-SHA256 digest does not match".to_string(),
        })
}

/// Map a [`KeyVaultError`] to the [`SecretError`] type used by the webhook layer.
fn map_key_vault_error(e: KeyVaultError, name: &SecretName) -> SecretError {
    match e {
        KeyVaultError::SecretNotFound { .. } => SecretError::NotFound {
            key: name.as_str().to_string(),
        },
        KeyVaultError::AccessDenied { .. } | KeyVaultError::AuthenticationFailed { .. } => {
            SecretError::AccessDenied {
                key: name.as_str().to_string(),
            }
        }
        other => SecretError::ProviderUnavailable(other.to_string()),
    }
}

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
        validate_hmac_sha256(payload, signature, secret_key)
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
// KeyVaultSignatureValidator
// ============================================================================

/// A [`SignatureValidator`] backed by Azure Key Vault.
///
/// Fetches the webhook secret by name from the configured [`KeyVaultProvider`].
/// Secret retrieval is transparently cached according to the provider's TTL
/// (default 300 s = 5 minutes, per spec assertion #16 "Secret Caching").
///
/// HMAC-SHA256 validation uses the same constant-time algorithm as
/// [`LiteralSignatureValidator`].
///
/// # Security
///
/// - The secret value is never stored on this struct; it is fetched from
///   the provider on each call to `get_webhook_secret` (and served from the
///   provider's in-memory cache for up to 5 minutes).
/// - `Debug` output shows only the secret name (the Key Vault lookup key),
///   which is not sensitive.
pub struct KeyVaultSignatureValidator {
    provider: Arc<dyn KeyVaultProvider>,
    secret_name: SecretName,
}

impl KeyVaultSignatureValidator {
    /// Construct a new validator that retrieves `secret_name` from `provider`.
    ///
    /// # Arguments
    ///
    /// * `provider` - Key Vault provider that handles caching and retrieval.
    /// * `secret_name` - Name of the secret inside the Key Vault.
    pub fn new(provider: Arc<dyn KeyVaultProvider>, secret_name: SecretName) -> Self {
        Self {
            provider,
            secret_name,
        }
    }
}

impl std::fmt::Debug for KeyVaultSignatureValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyVaultSignatureValidator")
            .field("secret_name", &self.secret_name.as_str())
            .finish()
    }
}

#[async_trait]
impl SignatureValidator for KeyVaultSignatureValidator {
    /// Validate a HMAC-SHA256 webhook signature.
    ///
    /// Uses the same constant-time HMAC-SHA256 algorithm as
    /// [`LiteralSignatureValidator`]. The `secret_key` is obtained by the
    /// caller via [`get_webhook_secret`](Self::get_webhook_secret) before
    /// this method is invoked.
    #[instrument(skip(self, payload, secret_key), fields(sig_len = signature.len()))]
    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        secret_key: &str,
    ) -> Result<(), ValidationError> {
        validate_hmac_sha256(payload, signature, secret_key)
    }

    /// Retrieve the webhook secret from Azure Key Vault.
    ///
    /// Returns the cached value when the TTL has not expired; otherwise
    /// fetches fresh from Key Vault and updates the cache.
    ///
    /// # Errors
    ///
    /// - [`SecretError::NotFound`] — secret does not exist in Key Vault.
    /// - [`SecretError::AccessDenied`] — insufficient permissions or auth failure.
    /// - [`SecretError::ProviderUnavailable`] — Key Vault is unreachable, timed out,
    ///   or rate-limited.
    #[instrument(skip(self), fields(secret_name = %self.secret_name))]
    async fn get_webhook_secret(&self, _event_type: &str) -> Result<String, SecretError> {
        self.provider
            .get_secret(&self.secret_name)
            .await
            .map(|v| v.expose_secret().to_string())
            .map_err(|e| map_key_vault_error(e, &self.secret_name))
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
