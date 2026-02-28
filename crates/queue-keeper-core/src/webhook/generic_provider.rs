//! Generic webhook provider for non-GitHub webhook sources.
//!
//! This module provides [`GenericWebhookProvider`], a configuration-driven
//! [`WebhookProcessor`] implementation that can handle webhooks from any
//! HTTP-based source (GitLab, Jira, Slack, custom apps, etc.) without
//! writing provider-specific Rust code.
//!
//! # Processing Modes
//!
//! A generic provider operates in one of two modes, selected via
//! [`ProcessingMode`] in [`GenericProviderConfig`]:
//!
//! | Mode       | Behaviour                                              |
//! |------------|--------------------------------------------------------|
//! | **Wrap**   | Parse the payload, extract fields via [`FieldExtractionConfig`], and produce a provider-agnostic [`WrappedEvent`]. |
//! | **Direct** | Forward the raw payload as-is with lightweight [`DirectQueueMetadata`]. |
//!
//! # Configuration
//!
//! New providers are added via YAML configuration — no Rust code changes
//! required. See [`GenericProviderConfig`] for the full schema.
//!
//! # Examples
//!
//! ```rust
//! use queue_keeper_core::webhook::generic_provider::{
//!     GenericProviderConfig, ProcessingMode,
//! };
//!
//! let config = GenericProviderConfig {
//!     provider_id: "jira".to_string(),
//!     processing_mode: ProcessingMode::Direct,
//!     event_type_source: None,
//!     delivery_id_source: None,
//!     signature: None,
//!     field_extraction: None,
//! };
//! assert!(config.validate().is_ok());
//! ```
//!
//! [`WrappedEvent`]: crate::webhook::WrappedEvent
//! [`DirectQueueMetadata`]: crate::webhook::DirectQueueMetadata

use crate::{
    audit_logging::AuditLogger,
    webhook::{
        NormalizationError, ProcessingOutput, StorageError, StorageReference, ValidationStatus,
        WebhookError, WebhookProcessor, WebhookRequest, WrappedEvent,
    },
    ValidationError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;

// ============================================================================
// Configuration types
// ============================================================================

/// Complete configuration for a single generic webhook provider.
///
/// This struct is loaded from YAML at startup and drives all runtime
/// behaviour of the corresponding [`GenericWebhookProvider`]. Adding a
/// new provider is entirely configuration-driven — no Rust code changes
/// are needed.
///
/// # Validation
///
/// Call [`GenericProviderConfig::validate`] at startup to catch
/// configuration errors early. The method checks:
///
/// - `provider_id` is non-empty and URL-safe (`[a-z0-9\-_]`)
/// - Wrap mode requires a [`FieldExtractionConfig`] to be present
/// - Any [`SignatureConfig`] is internally consistent
/// - Individual [`FieldSource`] values are non-empty
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::generic_provider::{
///     GenericProviderConfig, ProcessingMode, FieldExtractionConfig, FieldSource,
/// };
///
/// let config = GenericProviderConfig {
///     provider_id: "gitlab".to_string(),
///     processing_mode: ProcessingMode::Wrap,
///     event_type_source: Some(FieldSource::Header {
///         name: "X-Gitlab-Event".to_string(),
///     }),
///     delivery_id_source: Some(FieldSource::Header {
///         name: "X-Gitlab-Token".to_string(),
///     }),
///     signature: None,
///     field_extraction: Some(FieldExtractionConfig {
///         repository_path: "project.path_with_namespace".to_string(),
///         entity_path: Some("object_attributes.iid".to_string()),
///         action_path: Some("object_attributes.action".to_string()),
///     }),
/// };
/// assert!(config.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericProviderConfig {
    /// URL-safe provider identifier (`[a-z0-9\-_]+`).
    ///
    /// This value appears verbatim in the URL path: `/webhook/{provider_id}`.
    pub provider_id: String,

    /// Whether to normalise the payload into a [`WrappedEvent`] or forward
    /// it as-is.
    pub processing_mode: ProcessingMode,

    /// Where to read the "event type" value (e.g. a header or JSON field).
    ///
    /// In **wrap** mode this value becomes `WrappedEvent::event_type`.
    /// In **direct** mode it is used for logging and metrics only.
    /// When `None`, the event type defaults to `"webhook"`.
    #[serde(default)]
    pub event_type_source: Option<FieldSource>,

    /// Where to read a delivery / deduplication ID.
    ///
    /// When `None`, an auto-generated ULID is used instead.
    #[serde(default)]
    pub delivery_id_source: Option<FieldSource>,

    /// Optional HMAC signature validation configuration.
    ///
    /// When `None`, signature validation is skipped.
    #[serde(default)]
    pub signature: Option<SignatureConfig>,

    /// Field extraction rules for **wrap** mode.
    ///
    /// Required when `processing_mode` is [`ProcessingMode::Wrap`].
    /// Ignored in direct mode.
    #[serde(default)]
    pub field_extraction: Option<FieldExtractionConfig>,
}

impl GenericProviderConfig {
    /// Validate this configuration for internal consistency.
    ///
    /// # Errors
    ///
    /// Returns [`GenericProviderConfigError`] describing the first
    /// validation failure encountered.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_core::webhook::generic_provider::{
    ///     GenericProviderConfig, ProcessingMode,
    /// };
    ///
    /// let config = GenericProviderConfig {
    ///     provider_id: "my-app".to_string(),
    ///     processing_mode: ProcessingMode::Direct,
    ///     event_type_source: None,
    ///     delivery_id_source: None,
    ///     signature: None,
    ///     field_extraction: None,
    /// };
    /// assert!(config.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), GenericProviderConfigError> {
        // Validate provider ID: non-empty and URL-safe
        if self.provider_id.is_empty() {
            return Err(GenericProviderConfigError::InvalidProviderId {
                message: "provider_id must not be empty".to_string(),
            });
        }
        if !self
            .provider_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(GenericProviderConfigError::InvalidProviderId {
                message: format!(
                    "provider_id '{}' contains invalid characters; \
                     use lowercase alphanumeric, hyphens, or underscores",
                    self.provider_id
                ),
            });
        }

        // Wrap mode requires field extraction config
        if self.processing_mode == ProcessingMode::Wrap && self.field_extraction.is_none() {
            return Err(GenericProviderConfigError::MissingFieldExtraction {
                provider_id: self.provider_id.clone(),
            });
        }

        // Validate field sources if present
        if let Some(ref source) = self.event_type_source {
            source.validate("event_type_source")?;
        }
        if let Some(ref source) = self.delivery_id_source {
            source.validate("delivery_id_source")?;
        }

        // Validate signature config if present
        if let Some(ref sig) = self.signature {
            sig.validate(&self.provider_id)?;
        }

        // Validate field extraction config if present
        if let Some(ref extraction) = self.field_extraction {
            extraction.validate(&self.provider_id)?;
        }

        Ok(())
    }
}

// ============================================================================
// ProcessingMode
// ============================================================================

/// How the provider processes incoming webhooks.
///
/// # Variants
///
/// - **Wrap**: Parse the payload and produce a [`WrappedEvent`] using field
///   extraction rules from [`FieldExtractionConfig`].
/// - **Direct**: Forward the raw payload to the queue without transformation.
///
/// [`WrappedEvent`]: crate::webhook::WrappedEvent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingMode {
    /// Normalise into [`WrappedEvent`].
    Wrap,
    /// Forward raw payload.
    Direct,
}

// ============================================================================
// FieldSource
// ============================================================================

/// Where to read a particular field value from the incoming request.
///
/// Used to extract event type, delivery ID, and other metadata from
/// provider-specific HTTP headers or JSON body fields.
///
/// # Variants
///
/// | Variant       | Example                                 |
/// |---------------|-----------------------------------------|
/// | `Header`      | Read from `X-Gitlab-Event` header       |
/// | `JsonPath`    | Read from `object_attributes.action`    |
/// | `Static`      | Always use the configured literal value  |
/// | `AutoGenerate`| Auto-generate a unique value (ULID)     |
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::generic_provider::FieldSource;
///
/// let source = FieldSource::Header { name: "X-Gitlab-Event".to_string() };
/// assert!(source.validate("event_type_source").is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FieldSource {
    /// Read the value from an HTTP request header.
    Header {
        /// The case-insensitive header name (e.g. `"X-Gitlab-Event"`).
        name: String,
    },

    /// Read the value from a dot-separated JSON path in the request body.
    ///
    /// Example paths: `"project.id"`, `"object_attributes.action"`.
    /// Nested arrays are not supported; all segments must be object keys.
    JsonPath {
        /// Dot-separated path into the JSON body (e.g. `"project.id"`).
        path: String,
    },

    /// Always use a static literal value regardless of request content.
    Static {
        /// The literal value to use.
        value: String,
    },

    /// Auto-generate a unique value (ULID).
    ///
    /// Useful for delivery IDs when the provider does not supply one.
    AutoGenerate,
}

impl FieldSource {
    /// Validate this field source.
    ///
    /// # Errors
    ///
    /// Returns [`GenericProviderConfigError::InvalidFieldSource`] when:
    /// - `Header.name` is empty
    /// - `JsonPath.path` is empty
    /// - `Static.value` is empty
    pub fn validate(&self, context: &str) -> Result<(), GenericProviderConfigError> {
        match self {
            Self::Header { name } if name.is_empty() => {
                Err(GenericProviderConfigError::InvalidFieldSource {
                    context: context.to_string(),
                    message: "header name must not be empty".to_string(),
                })
            }
            Self::JsonPath { path } if path.is_empty() => {
                Err(GenericProviderConfigError::InvalidFieldSource {
                    context: context.to_string(),
                    message: "JSON path must not be empty".to_string(),
                })
            }
            Self::Static { value } if value.is_empty() => {
                Err(GenericProviderConfigError::InvalidFieldSource {
                    context: context.to_string(),
                    message: "static value must not be empty".to_string(),
                })
            }
            _ => Ok(()),
        }
    }
}

// ============================================================================
// SignatureConfig
// ============================================================================

/// Signature validation configuration for a generic provider.
///
/// When present, incoming requests must carry a valid signature in the
/// specified header, computed with the configured algorithm and the
/// provider's webhook secret.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::generic_provider::{
///     SignatureConfig, SignatureAlgorithm,
/// };
///
/// let sig = SignatureConfig {
///     header_name: "X-Hub-Signature-256".to_string(),
///     algorithm: SignatureAlgorithm::HmacSha256,
/// };
/// assert!(sig.validate("my-provider").is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureConfig {
    /// The HTTP header containing the signature (case-insensitive).
    pub header_name: String,

    /// The algorithm used to compute the signature.
    pub algorithm: SignatureAlgorithm,
}

impl SignatureConfig {
    /// Validate this signature configuration.
    ///
    /// # Errors
    ///
    /// Returns [`GenericProviderConfigError::InvalidSignatureConfig`] when
    /// `header_name` is empty.
    pub fn validate(&self, provider_id: &str) -> Result<(), GenericProviderConfigError> {
        if self.header_name.is_empty() {
            return Err(GenericProviderConfigError::InvalidSignatureConfig {
                provider_id: provider_id.to_string(),
                message: "signature header_name must not be empty".to_string(),
            });
        }
        Ok(())
    }
}

// ============================================================================
// SignatureAlgorithm
// ============================================================================

/// Algorithm used for webhook signature validation.
///
/// # Variants
///
/// | Variant       | Algorithm         | Common providers       |
/// |---------------|-------------------|------------------------|
/// | `HmacSha256`  | HMAC-SHA256       | GitHub, GitLab, Stripe |
/// | `HmacSha1`    | HMAC-SHA1         | Legacy providers       |
/// | `BearerToken`  | Bearer token match | Slack, Jira            |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureAlgorithm {
    /// HMAC-SHA256 — the recommended algorithm.
    HmacSha256,

    /// HMAC-SHA1 — supported for legacy providers, not recommended.
    HmacSha1,

    /// Simple bearer token comparison (the header value must exactly
    /// match the configured secret).
    BearerToken,
}

// ============================================================================
// FieldExtractionConfig
// ============================================================================

/// Rules for extracting structured fields from the JSON payload in **wrap** mode.
///
/// These paths are dot-separated keys into the JSON body. For example,
/// `"project.path_with_namespace"` accesses `{"project": {"path_with_namespace": "..."}}`.
///
/// # Required for Wrap Mode
///
/// `repository_path` is always required as the primary resource identifier
/// used for session-key derivation and event routing. The remaining paths are
/// optional and default to generic/unknown values when absent.
///
/// [`WrappedEvent`]: crate::webhook::WrappedEvent
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::generic_provider::FieldExtractionConfig;
///
/// let extraction = FieldExtractionConfig {
///     repository_path: "project.path_with_namespace".to_string(),
///     entity_path: Some("object_attributes.iid".to_string()),
///     action_path: Some("object_attributes.action".to_string()),
/// };
/// assert!(extraction.validate("gitlab").is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldExtractionConfig {
    /// Dot-separated path to the repository identifier in the payload
    /// (e.g. `"project.path_with_namespace"` for GitLab).
    ///
    /// The extracted value is used as the `Repository.full_name`.
    pub repository_path: String,

    /// Dot-separated path to the entity identifier (e.g. PR number, issue
    /// ID). When `None`, the entity defaults to [`EventEntity::Unknown`].
    ///
    /// [`EventEntity::Unknown`]: crate::webhook::EventEntity::Unknown
    #[serde(default)]
    pub entity_path: Option<String>,

    /// Dot-separated path to the action field (e.g. `"opened"`,
    /// `"closed"`). When `None`, the action is omitted from the envelope.
    #[serde(default)]
    pub action_path: Option<String>,
}

impl FieldExtractionConfig {
    /// Validate this extraction configuration.
    ///
    /// # Errors
    ///
    /// Returns [`GenericProviderConfigError::InvalidFieldExtraction`] when
    /// `repository_path` is empty.
    pub fn validate(&self, provider_id: &str) -> Result<(), GenericProviderConfigError> {
        if self.repository_path.is_empty() {
            return Err(GenericProviderConfigError::InvalidFieldExtraction {
                provider_id: provider_id.to_string(),
                message: "repository_path must not be empty".to_string(),
            });
        }
        Ok(())
    }
}

// ============================================================================
// Error types
// ============================================================================

/// Validation errors for [`GenericProviderConfig`].
#[derive(Debug, thiserror::Error)]
pub enum GenericProviderConfigError {
    /// The provider ID is empty or contains invalid characters.
    #[error("invalid provider ID: {message}")]
    InvalidProviderId { message: String },

    /// Wrap mode requires a field extraction configuration.
    #[error(
        "provider '{provider_id}': processing_mode is 'wrap' but no \
         field_extraction configuration was provided"
    )]
    MissingFieldExtraction { provider_id: String },

    /// A field source value is invalid (e.g. empty header name).
    #[error("{context}: {message}")]
    InvalidFieldSource { context: String, message: String },

    /// The signature configuration is invalid.
    #[error("provider '{provider_id}': {message}")]
    InvalidSignatureConfig {
        provider_id: String,
        message: String,
    },

    /// The field extraction configuration is invalid.
    #[error("provider '{provider_id}': {message}")]
    InvalidFieldExtraction {
        provider_id: String,
        message: String,
    },
}

// ============================================================================
// GenericWebhookProvider
// ============================================================================

/// A configuration-driven webhook provider for non-GitHub sources.
///
/// Implements [`WebhookProcessor`] based on rules defined in
/// [`GenericProviderConfig`]. No provider-specific Rust code is needed;
/// behaviour is entirely driven by configuration.
///
/// In **wrap** mode the provider parses the JSON payload, extracts fields
/// using the configured paths, and produces a provider-agnostic [`WrappedEvent`].
///
/// In **direct** mode the raw payload is forwarded with lightweight
/// [`DirectQueueMetadata`] (see [`crate::webhook::ProcessingOutput`]).
///
/// # Construction
///
/// Use [`GenericWebhookProvider::new`] which validates the config before
/// constructing the provider.
///
/// # Examples
///
/// ```rust
/// use queue_keeper_core::webhook::generic_provider::{
///     GenericWebhookProvider, GenericProviderConfig, ProcessingMode,
/// };
///
/// let config = GenericProviderConfig {
///     provider_id: "slack".to_string(),
///     processing_mode: ProcessingMode::Direct,
///     event_type_source: None,
///     delivery_id_source: None,
///     signature: None,
///     field_extraction: None,
/// };
/// let provider = GenericWebhookProvider::new(config, None).unwrap();
/// ```
///
/// [`DirectQueueMetadata`]: crate::webhook::DirectQueueMetadata
pub struct GenericWebhookProvider {
    config: GenericProviderConfig,
    #[allow(dead_code)]
    audit_logger: Option<Arc<dyn AuditLogger>>,
}

impl GenericWebhookProvider {
    /// Create a new generic provider, validating the configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Provider-specific configuration loaded from YAML.
    /// * `audit_logger` - Optional audit logger for compliance monitoring.
    ///
    /// # Errors
    ///
    /// Returns [`GenericProviderConfigError`] if the configuration is invalid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use queue_keeper_core::webhook::generic_provider::{
    ///     GenericWebhookProvider, GenericProviderConfig, ProcessingMode,
    /// };
    ///
    /// let config = GenericProviderConfig {
    ///     provider_id: "jira".to_string(),
    ///     processing_mode: ProcessingMode::Direct,
    ///     event_type_source: None,
    ///     delivery_id_source: None,
    ///     signature: None,
    ///     field_extraction: None,
    /// };
    /// let provider = GenericWebhookProvider::new(config, None).unwrap();
    /// ```
    pub fn new(
        config: GenericProviderConfig,
        audit_logger: Option<Arc<dyn AuditLogger>>,
    ) -> Result<Self, GenericProviderConfigError> {
        config.validate()?;
        Ok(Self {
            config,
            audit_logger,
        })
    }

    /// The provider ID for this instance.
    pub fn provider_id(&self) -> &str {
        &self.config.provider_id
    }

    /// The processing mode for this instance.
    pub fn processing_mode(&self) -> ProcessingMode {
        self.config.processing_mode
    }
}

// ============================================================================
// WebhookProcessor implementation (stubs for future tasks 2.2 – 2.4)
// ============================================================================

#[async_trait]
impl WebhookProcessor for GenericWebhookProvider {
    /// Process a generic webhook through the configured pipeline.
    ///
    /// **Not yet implemented** — full processing logic will be added in
    /// tasks 2.2 (wrap mode) and 2.3 (direct mode).
    #[instrument(skip(self, _request), fields(
        provider = %self.config.provider_id,
    ))]
    async fn process_webhook(
        &self,
        _request: WebhookRequest,
    ) -> Result<ProcessingOutput, WebhookError> {
        // Processing logic will be implemented in tasks 2.2 and 2.3.
        // For now, return an error indicating the feature is not yet available.
        Err(WebhookError::MalformedPayload {
            message: format!(
                "generic provider '{}' processing not yet implemented",
                self.config.provider_id
            ),
        })
    }

    /// Validate the webhook signature using the configured algorithm.
    ///
    /// **Not yet implemented** — full signature validation will be added
    /// in task 2.4.
    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _event_type: &str,
    ) -> Result<(), ValidationError> {
        // Signature validation will be implemented in task 2.4.
        // For now, succeed unconditionally (no validation).
        Ok(())
    }

    /// Store the raw payload for audit and replay.
    ///
    /// **Not yet implemented** — delegates to stub that returns a
    /// placeholder reference.
    async fn store_raw_payload(
        &self,
        request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        // Return a placeholder reference.
        Ok(StorageReference {
            blob_path: format!("not-stored/{}", request.delivery_id()),
            stored_at: crate::Timestamp::now(),
            size_bytes: request.body.len() as u64,
        })
    }

    /// Normalise the payload into a [`WrappedEvent`].
    ///
    /// **Not yet implemented** — full normalisation will be added in
    /// task 2.2.
    async fn normalize_event(
        &self,
        _request: &WebhookRequest,
    ) -> Result<WrappedEvent, NormalizationError> {
        Err(NormalizationError::MissingRequiredField {
            field: format!(
                "generic provider '{}' normalisation not yet implemented",
                self.config.provider_id
            ),
        })
    }
}

#[cfg(test)]
#[path = "generic_provider_tests.rs"]
mod tests;
