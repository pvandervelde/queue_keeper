//! JWT (JSON Web Token) generation for GitHub App authentication.
//!
//! This module provides JWT generation capabilities required for GitHub App authentication.
//! JWTs are used to authenticate as a GitHub App and exchange for installation tokens.
//!
//! # GitHub Requirements
//!
//! - JWTs must use RS256 algorithm (RSA Signature with SHA-256)
//! - Maximum expiration time is 10 minutes from issuance
//! - Claims must include `iss` (app ID), `iat` (issued at), and `exp` (expiration)
//!
//! See `github-bot-sdk-specs/modules/auth.md` for complete specification.

use crate::auth::{GitHubAppId, JsonWebToken, JwtClaims, KeyAlgorithm, PrivateKey};
use crate::error::{AuthError, ValidationError};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::RsaPrivateKey;

/// Interface for JWT token generation and signing.
///
/// This trait abstracts JWT generation to allow for different implementations
/// (production RSA signing, mock generators for testing, etc.).
///
/// # Examples
///
/// ```no_run
/// # use github_bot_sdk::auth::jwt::JwtGenerator;
/// # use github_bot_sdk::auth::{GitHubAppId, PrivateKey};
/// # async fn example(generator: impl JwtGenerator) {
/// let app_id = GitHubAppId::new(123456);
/// let token = generator.generate_jwt(app_id).await.unwrap();
/// assert!(!token.is_expired());
/// # }
/// ```
#[async_trait::async_trait]
pub trait JwtGenerator: Send + Sync {
    /// Generate a JWT token for GitHub App authentication.
    ///
    /// Creates a JWT with the following claims:
    /// - `iss`: GitHub App ID
    /// - `iat`: Current timestamp (issued at)
    /// - `exp`: Expiration timestamp (issued at + duration, max 10 minutes)
    ///
    /// # Arguments
    ///
    /// * `app_id` - The GitHub App ID to include in the token
    ///
    /// # Returns
    ///
    /// A `JsonWebToken` containing the signed JWT string and metadata.
    ///
    /// # Errors
    ///
    /// Returns `AuthError` if:
    /// - Private key is invalid or cannot be loaded
    /// - JWT signing fails
    /// - System clock is unreliable
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use github_bot_sdk::auth::jwt::JwtGenerator;
    /// # use github_bot_sdk::auth::GitHubAppId;
    /// # async fn example(generator: impl JwtGenerator) {
    /// let app_id = GitHubAppId::new(123456);
    /// let jwt = generator.generate_jwt(app_id).await.expect("JWT generation failed");
    ///
    /// // Token is valid for up to 10 minutes
    /// assert!(!jwt.is_expired());
    /// assert_eq!(jwt.app_id(), app_id);
    /// # }
    /// ```
    async fn generate_jwt(&self, app_id: GitHubAppId) -> Result<JsonWebToken, AuthError>;

    /// Get the JWT expiration duration configured for this generator.
    ///
    /// Returns the duration from issuance to expiration. This value should not
    /// exceed 10 minutes (GitHub's maximum).
    fn expiration_duration(&self) -> Duration;
}

/// RS256 JWT generator using RSA private keys.
///
/// This is the standard implementation for GitHub App authentication. It uses
/// RSA-SHA256 signing as required by GitHub's API.
///
/// # Examples
///
/// ```no_run
/// # use github_bot_sdk::auth::jwt::RS256JwtGenerator;
/// # use github_bot_sdk::auth::PrivateKey;
/// # use chrono::Duration;
/// # let key_pem = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
/// let private_key = PrivateKey::from_pem(key_pem).unwrap();
/// let generator = RS256JwtGenerator::new(private_key);
///
/// // Generator is ready to produce JWTs
/// ```
pub struct RS256JwtGenerator {
    private_key: PrivateKey,
    expiration_duration: Duration,
}

impl RS256JwtGenerator {
    /// Create a new RS256 JWT generator.
    ///
    /// # Arguments
    ///
    /// * `private_key` - RSA private key for signing JWTs
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use github_bot_sdk::auth::jwt::RS256JwtGenerator;
    /// # use github_bot_sdk::auth::PrivateKey;
    /// # let key_pem = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    /// let private_key = PrivateKey::from_pem(key_pem).unwrap();
    /// let generator = RS256JwtGenerator::new(private_key);
    /// ```
    pub fn new(private_key: PrivateKey) -> Self {
        Self {
            private_key,
            expiration_duration: Duration::minutes(10), // GitHub's maximum
        }
    }

    /// Create a new RS256 JWT generator with custom expiration duration.
    ///
    /// # Arguments
    ///
    /// * `private_key` - RSA private key for signing JWTs
    /// * `expiration_duration` - How long JWTs should be valid (max 10 minutes)
    ///
    /// # Panics
    ///
    /// Panics if `expiration_duration` exceeds 10 minutes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use github_bot_sdk::auth::jwt::RS256JwtGenerator;
    /// # use github_bot_sdk::auth::PrivateKey;
    /// # use chrono::Duration;
    /// # let key_pem = "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----";
    /// let private_key = PrivateKey::from_pem(key_pem).unwrap();
    ///
    /// // Use 8-minute expiration for extra safety margin
    /// let generator = RS256JwtGenerator::with_expiration(
    ///     private_key,
    ///     Duration::minutes(8)
    /// );
    /// ```
    pub fn with_expiration(private_key: PrivateKey, expiration_duration: Duration) -> Self {
        assert!(
            expiration_duration <= Duration::minutes(10),
            "JWT expiration cannot exceed 10 minutes (GitHub requirement)"
        );

        Self {
            private_key,
            expiration_duration,
        }
    }

    /// Build JWT claims for the given app ID.
    fn build_claims(&self, app_id: GitHubAppId) -> JwtClaims {
        let now = Utc::now();
        let iat = now.timestamp();
        let exp = (now + self.expiration_duration).timestamp();

        JwtClaims {
            iss: app_id,
            iat,
            exp,
        }
    }
}

#[async_trait::async_trait]
impl JwtGenerator for RS256JwtGenerator {
    async fn generate_jwt(&self, app_id: GitHubAppId) -> Result<JsonWebToken, AuthError> {
        let claims = self.build_claims(app_id);
        let expires_at = Utc::now() + self.expiration_duration;

        // Create encoding key from private key
        let encoding_key = EncodingKey::from_rsa_pem(self.private_key.key_data())
            .map_err(|e| AuthError::InvalidPrivateKey {
                message: format!("Failed to create encoding key: {}", e),
            })?;

        // Set up JWT header for RS256
        let header = Header::new(Algorithm::RS256);

        // Encode the JWT
        let token_string = encode(&header, &claims, &encoding_key)
            .map_err(|e| AuthError::JwtGenerationFailed {
                message: format!("Failed to encode JWT: {}", e),
            })?;

        Ok(JsonWebToken::new(token_string, app_id, expires_at))
    }

    fn expiration_duration(&self) -> Duration {
        self.expiration_duration
    }
}

impl PrivateKey {
    /// Create a private key from PEM-encoded string.
    ///
    /// # Arguments
    ///
    /// * `pem` - PEM-encoded RSA private key
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if:
    /// - PEM format is invalid
    /// - Key type is not RSA
    /// - Key data is corrupted
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use github_bot_sdk::auth::PrivateKey;
    /// let pem = r#"-----BEGIN RSA PRIVATE KEY-----
    /// MIIEpAIBAAKCAQEA...
    /// -----END RSA PRIVATE KEY-----"#;
    ///
    /// let key = PrivateKey::from_pem(pem).expect("Invalid PEM");
    /// ```
    pub fn from_pem(pem: &str) -> Result<Self, ValidationError> {
        // Trim whitespace
        let pem = pem.trim();

        // Validate PEM format
        if pem.is_empty() {
            return Err(ValidationError::InvalidFormat {
                field: "private_key".to_string(),
                message: "PEM string cannot be empty".to_string(),
            });
        }

        if !pem.contains("-----BEGIN") || !pem.contains("-----END") {
            return Err(ValidationError::InvalidFormat {
                field: "private_key".to_string(),
                message: "Invalid PEM format: missing BEGIN/END markers".to_string(),
            });
        }

        // Attempt to parse the RSA private key to validate it
        RsaPrivateKey::from_pkcs1_pem(pem)
            .map_err(|e| ValidationError::InvalidFormat {
                field: "private_key".to_string(),
                message: format!("Failed to parse RSA private key: {}", e),
            })?;

        // Store the PEM bytes
        Ok(Self {
            key_data: pem.as_bytes().to_vec(),
            algorithm: KeyAlgorithm::RS256,
        })
    }

    /// Create a private key from PKCS#8 DER-encoded bytes.
    ///
    /// # Arguments
    ///
    /// * `der` - DER-encoded PKCS#8 private key bytes
    ///
    /// # Errors
    ///
    /// Returns `ValidationError` if:
    /// - DER format is invalid
    /// - Key type is not RSA
    /// - Key data is corrupted
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self, ValidationError> {
        // Validate by attempting to parse
        use rsa::pkcs8::DecodePrivateKey;
        RsaPrivateKey::from_pkcs8_der(der)
            .map_err(|e| ValidationError::InvalidFormat {
                field: "private_key".to_string(),
                message: format!("Failed to parse PKCS#8 DER private key: {}", e),
            })?;

        Ok(Self {
            key_data: der.to_vec(),
            algorithm: KeyAlgorithm::RS256,
        })
    }
}

#[cfg(test)]
#[path = "jwt_tests.rs"]
mod tests;
