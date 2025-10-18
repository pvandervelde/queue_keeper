//! Error types for GitHub Bot SDK operations.
//!
//! This module defines all error types used throughout the SDK, with proper
//! classification for retry logic and comprehensive context for debugging.

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::auth::InstallationId;

/// Authentication-related errors with retry classification.
///
/// This error type covers all authentication failures including credential issues,
/// token expiration, and GitHub API errors. Each variant includes metadata to
/// support intelligent retry logic and detailed error reporting.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Invalid GitHub App credentials (non-retryable).
    #[error("Invalid GitHub App credentials")]
    InvalidCredentials,

    /// Installation not found or access denied (non-retryable).
    #[error("Installation {installation_id} not found or access denied")]
    InstallationNotFound { installation_id: InstallationId },

    /// Installation token has expired (retryable via refresh).
    #[error("Installation token expired")]
    TokenExpired,

    /// Insufficient permissions for the requested operation (non-retryable).
    #[error("Insufficient permissions for operation: {permission}")]
    InsufficientPermissions { permission: String },

    /// Invalid private key format or data (non-retryable).
    #[error("Invalid private key: {message}")]
    InvalidPrivateKey { message: String },

    /// JWT generation failed (non-retryable).
    #[error("JWT generation failed: {message}")]
    JwtGenerationFailed { message: String },

    /// GitHub API returned an error response.
    #[error("GitHub API error: {status} - {message}")]
    GitHubApiError { status: u16, message: String },

    /// JWT signing operation failed.
    #[error("JWT signing failed: {0}")]
    SigningError(#[from] SigningError),

    /// Secret retrieval from secure storage failed.
    #[error("Secret retrieval failed: {0}")]
    SecretError(#[from] SecretError),

    /// Token cache operation failed.
    #[error("Token cache error: {0}")]
    CacheError(#[from] CacheError),

    /// Network connectivity or transport error.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// GitHub API client error.
    #[error("API error: {0}")]
    ApiError(#[from] ApiError),
}

impl AuthError {
    /// Check if this error represents a transient condition that may succeed if retried.
    ///
    /// Transient errors include:
    /// - Network failures
    /// - Server errors (5xx)
    /// - Rate limiting (429)
    /// - Token expiration (can refresh)
    /// - Cache failures (can regenerate)
    ///
    /// Non-transient errors include:
    /// - Invalid credentials
    /// - Missing installations
    /// - Insufficient permissions
    /// - Client errors (4xx except 429)
    pub fn is_transient(&self) -> bool {
        match self {
            Self::InvalidCredentials => false,
            Self::InstallationNotFound { .. } => false,
            Self::TokenExpired => true, // Can refresh token
            Self::InsufficientPermissions { .. } => false,
            Self::InvalidPrivateKey { .. } => false,
            Self::JwtGenerationFailed { .. } => false,
            Self::GitHubApiError { status, .. } => *status >= 500 || *status == 429,
            Self::SigningError(_) => false,
            Self::SecretError(e) => e.is_transient(),
            Self::CacheError(_) => true, // Can fallback to fresh generation
            Self::NetworkError(_) => true,
            Self::ApiError(e) => e.is_transient(),
        }
    }

    /// Determine if this error should trigger a retry attempt.
    ///
    /// Alias for `is_transient()` to support different retry policy conventions.
    pub fn should_retry(&self) -> bool {
        self.is_transient()
    }

    /// Get the recommended retry delay for this error.
    ///
    /// Returns `Some(Duration)` if a specific delay is recommended (e.g., rate limiting),
    /// or `None` to use the default exponential backoff policy.
    pub fn retry_after(&self) -> Option<chrono::Duration> {
        match self {
            Self::GitHubApiError { status, .. } if *status == 429 => {
                Some(chrono::Duration::minutes(1))
            }
            Self::NetworkError(_) => Some(chrono::Duration::seconds(5)),
            _ => None,
        }
    }
}

/// Errors during secret retrieval from secure storage.
///
/// These errors occur when accessing secrets from Key Vault, environment variables,
/// or other secure storage mechanisms.
#[derive(Debug, Error)]
pub enum SecretError {
    /// The requested secret was not found in the storage provider.
    #[error("Secret not found: {key}")]
    NotFound { key: String },

    /// Access to the secret was denied due to permissions.
    #[error("Access denied to secret: {key}")]
    AccessDenied { key: String },

    /// The secret storage provider is unavailable (retryable).
    #[error("Secret provider unavailable: {0}")]
    ProviderUnavailable(String),

    /// The secret exists but has an invalid format.
    #[error("Invalid secret format: {key}")]
    InvalidFormat { key: String },
}

impl SecretError {
    /// Check if this error represents a transient condition.
    ///
    /// Only `ProviderUnavailable` is considered transient.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::ProviderUnavailable(_))
    }
}

/// Errors during token caching operations.
///
/// Cache errors are generally non-fatal and allow fallback to regenerating tokens.
#[derive(Debug, Error)]
pub enum CacheError {
    /// A cache operation failed for a specific reason.
    #[error("Cache operation failed: {message}")]
    OperationFailed { message: String },

    /// The cache is unavailable or unreachable.
    #[error("Cache unavailable: {message}")]
    Unavailable { message: String },

    /// Failed to serialize or deserialize cached data.
    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Errors during JWT signing operations.
///
/// These errors occur during cryptographic operations for JWT generation.
#[derive(Debug, Error)]
pub enum SigningError {
    /// The private key is invalid or malformed.
    #[error("Invalid private key: {message}")]
    InvalidKey { message: String },

    /// The signing operation failed.
    #[error("Signing operation failed: {message}")]
    SigningFailed { message: String },

    /// Failed to encode the JWT token.
    #[error("Token encoding failed: {message}")]
    EncodingFailed { message: String },
}

/// Errors during GitHub API operations.
///
/// These errors represent failures when communicating with the GitHub API,
/// including HTTP errors, rate limiting, and parsing failures.
#[derive(Debug, Error)]
pub enum ApiError {
    /// HTTP error response from GitHub API.
    #[error("HTTP error: {status} - {message}")]
    HttpError { status: u16, message: String },

    /// Rate limit exceeded. Operations should wait until reset time.
    #[error("Rate limit exceeded. Reset at: {reset_at}")]
    RateLimitExceeded { reset_at: DateTime<Utc> },

    /// Request to GitHub API timed out.
    #[error("Request timeout")]
    Timeout,

    /// The request was invalid (client error).
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// Authentication to GitHub API failed.
    #[error("Authentication failed")]
    AuthenticationFailed,

    /// Authorization check failed (insufficient permissions).
    #[error("Authorization failed")]
    AuthorizationFailed,

    /// The requested resource was not found.
    #[error("Resource not found")]
    NotFound,

    /// Failed to parse JSON response from GitHub API.
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// HTTP client error (network, TLS, etc.).
    #[error("HTTP client error: {0}")]
    HttpClientError(#[from] reqwest::Error),
}

impl ApiError {
    /// Check if this error represents a transient condition that may succeed if retried.
    ///
    /// Transient conditions include:
    /// - Server errors (5xx)
    /// - Rate limiting (429)
    /// - Request timeouts
    /// - Network/transport errors
    pub fn is_transient(&self) -> bool {
        match self {
            Self::HttpError { status, .. } => *status >= 500 || *status == 429,
            Self::RateLimitExceeded { .. } => true,
            Self::Timeout => true,
            Self::InvalidRequest { .. } => false,
            Self::AuthenticationFailed => false,
            Self::AuthorizationFailed => false,
            Self::NotFound => false,
            Self::JsonError(_) => false,
            Self::HttpClientError(_) => true, // Network issues are transient
        }
    }
}

/// Input validation errors.
///
/// These errors occur when validating user input or configuration data.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// A required field is missing.
    #[error("Required field missing: {field}")]
    Required { field: String },

    /// A field has an invalid format.
    #[error("Invalid format for {field}: {message}")]
    InvalidFormat { field: String, message: String },

    /// A field value is out of the acceptable range.
    #[error("Value out of range for {field}: {message}")]
    OutOfRange { field: String, message: String },
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
