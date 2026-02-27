//! GitHub-specific webhook provider.
//!
//! This module provides [`GithubWebhookProvider`], the concrete [`WebhookProcessor`]
//! implementation for GitHub webhooks. It encapsulates all GitHub-specific processing
//! semantics including:
//!
//! - Parsing GitHub-specific HTTP headers (`X-GitHub-Event`, `X-GitHub-Delivery`,
//!   `X-Hub-Signature-256`)
//! - HMAC-SHA256 signature validation via a pluggable [`SignatureValidator`]
//! - Raw payload archival via a pluggable [`PayloadStorer`]
//! - Normalisation of GitHub payloads into the standard [`EventEnvelope`] format
//!
//! # Registration
//!
//! The provider must be registered under the canonical provider ID `"github"`:
//!
//! ```rust,no_run
//! use queue_keeper_core::webhook::{GithubWebhookProvider, WebhookProcessor};
//! use std::sync::Arc;
//!
//! let provider: Arc<dyn WebhookProcessor> =
//!     Arc::new(GithubWebhookProvider::new(None, None, None));
//! // registry.register(ProviderId::new("github").unwrap(), provider);
//! ```
//!
//! # Provider ID
//!
//! [`GithubWebhookProvider::PROVIDER_ID`] is `"github"` and must be used when
//! registering this provider with the [`ProviderRegistry`].

use crate::{
    audit_logging::AuditLogger,
    webhook::{
        EventEnvelope, NormalizationError, PayloadStorer, SignatureValidator, StorageError,
        StorageReference, ValidationStatus, WebhookError, WebhookProcessor, WebhookProcessorImpl,
        WebhookRequest,
    },
    ValidationError,
};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::instrument;

// ============================================================================
// GithubWebhookProvider
// ============================================================================

/// Webhook provider for GitHub events.
///
/// Implements the full GitHub webhook processing pipeline including header
/// parsing, HMAC-SHA256 signature validation, payload storage, and event
/// normalisation. All processing is delegated to the inner
/// [`WebhookProcessorImpl`], with this type serving as the named GitHub
/// integration point in the provider registry.
///
/// All dependencies are optional to support testing scenarios where not all
/// infrastructure is available.
///
/// # Examples
///
/// ```rust,no_run
/// use queue_keeper_core::webhook::GithubWebhookProvider;
///
/// // Minimal provider for testing — no signature validation or storage
/// let provider = GithubWebhookProvider::new(None, None, None);
/// assert_eq!(GithubWebhookProvider::PROVIDER_ID, "github");
/// ```
///
/// # Errors
///
/// All processing errors are returned as [`WebhookError`] variants. See
/// [`WebhookProcessor::process_webhook`] for the full error contract.
pub struct GithubWebhookProvider {
    inner: WebhookProcessorImpl,
}

impl GithubWebhookProvider {
    /// The canonical provider ID used when registering this provider.
    ///
    /// Must be passed to `ProviderId::new()` when building the provider registry.
    pub const PROVIDER_ID: &'static str = "github";

    /// Create a new `GithubWebhookProvider` with optional dependencies.
    ///
    /// All three dependencies may be `None`; omitting them is useful for
    /// testing or when a particular feature (e.g. payload archival) is not
    /// required in the current deployment.
    ///
    /// # Arguments
    ///
    /// * `signature_validator` - Optional HMAC-SHA256 validator. When `None`,
    ///   signature checking is skipped (not recommended for production).
    /// * `payload_storer` - Optional blob storage for raw payload archival.
    ///   When `None`, payloads are not persisted.
    /// * `audit_logger` - Optional audit logger for compliance and security
    ///   monitoring. When `None`, audit events are not emitted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_core::webhook::GithubWebhookProvider;
    ///
    /// let provider = GithubWebhookProvider::new(None, None, None);
    /// ```
    pub fn new(
        signature_validator: Option<Arc<dyn SignatureValidator>>,
        payload_storer: Option<Arc<dyn PayloadStorer>>,
        audit_logger: Option<Arc<dyn AuditLogger>>,
    ) -> Self {
        Self {
            inner: WebhookProcessorImpl::new(signature_validator, payload_storer, audit_logger),
        }
    }
}

// ============================================================================
// WebhookProcessor implementation
// ============================================================================

#[async_trait]
impl WebhookProcessor for GithubWebhookProvider {
    /// Process a GitHub webhook request through the full pipeline.
    ///
    /// Delegates to the inner [`WebhookProcessorImpl`].
    ///
    /// # Errors
    ///
    /// Returns [`WebhookError`] if:
    /// - Header validation fails (missing or malformed GitHub headers)
    /// - Signature validation fails (when a validator is configured)
    /// - Payload storage fails (when a storer is configured)
    /// - Event normalization fails (malformed or missing payload fields)
    #[instrument(skip(self, request), fields(
        provider = Self::PROVIDER_ID,
        event_type = %request.event_type(),
        delivery_id = %request.delivery_id(),
    ))]
    async fn process_webhook(
        &self,
        request: WebhookRequest,
    ) -> Result<EventEnvelope, WebhookError> {
        self.inner.process_webhook(request).await
    }

    /// Validate the GitHub HMAC-SHA256 webhook signature.
    ///
    /// Delegates to the inner [`WebhookProcessorImpl`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError`] if the signature is invalid or if the
    /// secret cannot be retrieved.
    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        event_type: &str,
    ) -> Result<(), ValidationError> {
        self.inner
            .validate_signature(payload, signature, event_type)
            .await
    }

    /// Store the raw webhook payload for audit and replay purposes.
    ///
    /// Delegates to the inner [`WebhookProcessorImpl`].
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if storage is configured and the operation
    /// fails. Returns a placeholder reference when no storer is configured.
    async fn store_raw_payload(
        &self,
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        self.inner
            .store_raw_payload(request, validation_status)
            .await
    }

    /// Normalise a GitHub webhook payload into an [`EventEnvelope`].
    ///
    /// Delegates to the inner [`WebhookProcessorImpl`].
    ///
    /// # Errors
    ///
    /// Returns [`NormalizationError`] if required fields are missing from
    /// the payload or the JSON structure is unexpected.
    async fn normalize_event(
        &self,
        request: &WebhookRequest,
    ) -> Result<EventEnvelope, NormalizationError> {
        self.inner.normalize_event(request).await
    }
}

#[cfg(test)]
#[path = "github_provider_tests.rs"]
mod tests;
