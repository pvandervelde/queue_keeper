//! Error types for the HTTP service

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use queue_keeper_core::{ValidationError, WebhookError};
use tracing::{error, warn};

/// Webhook handler errors with HTTP status code mapping
///
/// This error type represents all possible webhook processing failures
/// and maps them to appropriate HTTP status codes following REST conventions:
///
/// - `400 Bad Request`: Client errors that are permanent and not retryable
///   (invalid headers, malformed payloads, validation failures)
/// - `500 Internal Server Error`: Unexpected server failures
/// - `503 Service Unavailable`: Transient failures that should be retried
///   (temporary storage unavailability, network issues)
///
/// # Error Classification
///
/// Errors are classified as either:
/// - **Permanent**: Client should not retry (4xx status codes)
/// - **Transient**: Client should retry with backoff (503 status code)
///
/// # Security Considerations
///
/// Error messages returned to clients are sanitized to prevent information
/// disclosure. Detailed error information is logged server-side with
/// correlation IDs for debugging.
#[derive(Debug, thiserror::Error)]
pub enum WebhookHandlerError {
    /// Invalid or missing required HTTP headers
    ///
    /// Maps to: `400 Bad Request` (permanent error, do not retry)
    ///
    /// Common causes:
    /// - Missing `X-GitHub-Event` header
    /// - Missing `X-GitHub-Delivery` header
    /// - Invalid header format or encoding
    #[error("Invalid headers: {0}")]
    InvalidHeaders(#[from] ValidationError),

    /// Webhook processing pipeline failure
    ///
    /// Maps to:
    /// - `400 Bad Request` if error is permanent (invalid signature, malformed payload)
    /// - `503 Service Unavailable` if error is transient (storage temporarily down)
    ///
    /// The underlying `WebhookError` determines if the failure is transient
    /// via the `is_transient()` method.
    #[error("Processing failed: {0}")]
    ProcessingFailed(#[from] WebhookError),

    /// Unexpected internal server error
    ///
    /// Maps to: `500 Internal Server Error` (server-side bug or unexpected failure)
    ///
    /// These errors indicate bugs or unexpected system states that should
    /// be investigated. Details are logged but a generic message is returned
    /// to the client.
    #[error("Internal server error: {message}")]
    InternalError { message: String },

    /// Request timeout
    ///
    /// Maps to: `408 Request Timeout` (client should retry)
    ///
    /// Occurs when webhook processing exceeds the configured timeout.
    /// GitHub expects responses within 10 seconds.
    #[error("Request timeout after {seconds}s")]
    Timeout { seconds: u64 },

    /// Payload too large
    ///
    /// Maps to: `413 Payload Too Large` (permanent error, do not retry)
    ///
    /// Occurs when webhook payload exceeds the configured maximum size.
    #[error("Payload too large: {size} bytes (max: {max_size} bytes)")]
    PayloadTooLarge { size: usize, max_size: usize },

    /// Rate limit exceeded
    ///
    /// Maps to: `429 Too Many Requests` (client should retry after delay)
    ///
    /// Occurs when too many requests are received from a single source.
    /// Includes retry-after duration in response headers.
    #[error("Rate limit exceeded. Retry after {retry_after_seconds}s")]
    RateLimitExceeded { retry_after_seconds: u64 },

    /// Webhook provider not found in the registry
    ///
    /// Maps to: `404 Not Found` (permanent error, the provider is not configured)
    ///
    /// Occurs when the `{provider}` URL segment does not match any entry
    /// in the [`ProviderRegistry`](crate::provider_registry::ProviderRegistry).
    #[error("Webhook provider not found: {provider}")]
    ProviderNotFound { provider: String },
}

impl IntoResponse for WebhookHandlerError {
    fn into_response(self) -> Response {
        // Determine HTTP status code and error message based on error type
        let (status, message, retry_after) = match self {
            Self::InvalidHeaders(_) => (StatusCode::BAD_REQUEST, self.to_string(), None),
            Self::ProcessingFailed(ref e) => {
                if e.is_transient() {
                    // Transient errors should be retried
                    (StatusCode::SERVICE_UNAVAILABLE, self.to_string(), Some(60))
                } else {
                    // Permanent errors should not be retried
                    (StatusCode::BAD_REQUEST, self.to_string(), None)
                }
            }
            Self::InternalError { ref message } => {
                // Log detailed error server-side but return generic message to client
                error!(error = %message, "Internal server error occurred");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error occurred. Please try again later.".to_string(),
                    None,
                )
            }
            Self::Timeout { seconds } => {
                warn!(timeout_seconds = seconds, "Request timeout");
                (StatusCode::REQUEST_TIMEOUT, self.to_string(), Some(5))
            }
            Self::PayloadTooLarge { size, max_size } => {
                warn!(
                    payload_size = size,
                    max_size = max_size,
                    "Payload too large"
                );
                (StatusCode::PAYLOAD_TOO_LARGE, self.to_string(), None)
            }
            Self::RateLimitExceeded {
                retry_after_seconds,
            } => {
                warn!(retry_after = retry_after_seconds, "Rate limit exceeded");
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    self.to_string(),
                    Some(retry_after_seconds),
                )
            }
            Self::ProviderNotFound { ref provider } => {
                warn!(provider = %provider, "Webhook provider not found");
                (StatusCode::NOT_FOUND, self.to_string(), None)
            }
        };

        // Build JSON error response
        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // Build response with appropriate headers
        let mut response = (status, Json(body)).into_response();

        // Add Retry-After header for retryable errors
        if let Some(retry_seconds) = retry_after {
            if let Ok(header_value) = retry_seconds.to_string().parse() {
                response.headers_mut().insert("Retry-After", header_value);
            }
        }

        response
    }
}

/// Service-level errors
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Failed to bind to address {address}: {message}")]
    BindFailed { address: String, message: String },

    #[error("Server failed: {message}")]
    ServerFailed { message: String },

    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigError),

    #[error("Health check failed: {message}")]
    HealthCheckFailed { message: String },
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {message}")]
    Invalid { message: String },

    #[error("Missing required configuration: {key}")]
    Missing { key: String },

    #[error("Configuration parsing failed: {0}")]
    Parsing(#[from] toml::de::Error),
}
