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
//!     target_queue: Some("queue-keeper-jira".to_string()),
//!     event_type_source: None,
//!     delivery_id_source: None,
//!     signature: None,
//!     webhook_secret: None,
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
        DirectQueueMetadata, NormalizationError, ProcessingOutput, SignatureValidator, StorageError,
        StorageReference, ValidationStatus, WebhookError, WebhookProcessor, WebhookRequest,
        WrappedEvent,
    },
    ValidationError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{instrument, warn};

/// Source for a generic provider's webhook validation secret.
///
/// Used in [`GenericProviderConfig::webhook_secret`] to supply the HMAC key
/// or bearer token that the provider uses to sign requests.
///
/// In production deployments, always use [`WebhookSecretConfig::KeyVault`]
/// to avoid embedding secrets in configuration files or environment variables.
///
/// # Security
///
/// [`WebhookSecretConfig::Literal`] is provided for development and testing
/// only. A startup `WARN` is emitted when a literal secret is active.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum WebhookSecretConfig {
    /// Secret fetched from Azure Key Vault at validation time.
    KeyVault {
        /// Name of the secret inside the vault.
        secret_name: String,
    },

    /// Literal secret embedded in the configuration.
    ///
    /// **Development / testing only.** Never commit to source control.
    Literal {
        /// Raw secret value.  Excluded from `Debug` output.
        #[serde(rename = "value")]
        value: String,
    },
}

impl std::fmt::Debug for WebhookSecretConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeyVault { secret_name } => f
                .debug_struct("WebhookSecretConfig::KeyVault")
                .field("secret_name", secret_name)
                .finish(),
            Self::Literal { .. } => f
                .debug_struct("WebhookSecretConfig::Literal")
                .field("value", &"<REDACTED>")
                .finish(),
        }
    }
}

// ============================================================================
// GenericProviderConfig
// ============================================================================

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
/// - Direct mode requires a non-empty `target_queue` to be configured
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
///     target_queue: None,
///     event_type_source: Some(FieldSource::Header {
///         name: "X-Gitlab-Event".to_string(),
///     }),
///     delivery_id_source: Some(FieldSource::Header {
///         name: "X-Gitlab-Token".to_string(),
///     }),
///     signature: None,
///     webhook_secret: None,
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

    /// Target Azure Service Bus queue for **direct** mode delivery.
    ///
    /// Required when `processing_mode` is [`ProcessingMode::Direct`].
    /// The queue must follow Azure Service Bus naming conventions
    /// (`queue-keeper-{provider}`).
    /// Ignored in wrap mode (routing is determined by [`BotConfiguration`]
    /// in that case).
    ///
    /// [`BotConfiguration`]: crate::bot_config::BotConfiguration
    #[serde(default)]
    pub target_queue: Option<String>,

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

    /// Source for the webhook validation secret (HMAC key or bearer token).
    ///
    /// Required when [`signature`](Self::signature) is set; the secret
    /// supplies the HMAC key or bearer token used to verify incoming requests.
    /// When `None` and a `signature` config is present, validation is skipped
    /// with a startup `WARN`.
    ///
    /// In production, always use [`WebhookSecretConfig::KeyVault`].
    #[serde(default)]
    pub webhook_secret: Option<WebhookSecretConfig>,

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
    ///     target_queue: Some("queue-keeper-my-app".to_string()),
    ///     event_type_source: None,
    ///     delivery_id_source: None,
    ///     signature: None,
    ///     webhook_secret: None,
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

        // Direct mode requires a target queue for delivery
        if self.processing_mode == ProcessingMode::Direct && self.target_queue.is_none() {
            return Err(GenericProviderConfigError::MissingTargetQueue {
                provider_id: self.provider_id.clone(),
            });
        }

        // Validate target queue name format if present
        if let Some(ref queue) = self.target_queue {
            if queue.is_empty() {
                return Err(GenericProviderConfigError::InvalidTargetQueue {
                    provider_id: self.provider_id.clone(),
                    message: "target_queue must not be empty".to_string(),
                });
            }
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

    /// Direct mode requires a target queue for message delivery.
    #[error(
        "provider '{provider_id}': processing_mode is 'direct' but no \
         target_queue was configured"
    )]
    MissingTargetQueue { provider_id: String },

    /// The target queue name is invalid.
    #[error("provider '{provider_id}': invalid target_queue: {message}")]
    InvalidTargetQueue {
        provider_id: String,
        message: String,
    },

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
///     target_queue: Some("queue-keeper-slack".to_string()),
///     event_type_source: None,
///     delivery_id_source: None,
///     signature: None,
///     webhook_secret: None,
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
    /// Optional signature validator for HMAC validation.
    ///
    /// When [`GenericProviderConfig::signature`] is `Some` and a validator is
    /// provided, incoming requests are validated before processing. When the
    /// validator is `None` and signature config is present, a warning is logged
    /// and the check is skipped.
    signature_validator: Option<Arc<dyn SignatureValidator>>,
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
    ///     target_queue: Some("queue-keeper-jira".to_string()),
    ///     event_type_source: None,
    ///     delivery_id_source: None,
    ///     signature: None,
    ///     webhook_secret: None,
    ///     field_extraction: None,
    /// };
    /// let provider = GenericWebhookProvider::new(config, None).unwrap();
    /// ```
    pub fn new(
        config: GenericProviderConfig,
        audit_logger: Option<Arc<dyn AuditLogger>>,
    ) -> Result<Self, GenericProviderConfigError> {
        Self::with_signature_validator(config, audit_logger, None)
    }

    /// Create a new generic provider with a signature validator.
    ///
    /// Use this constructor when the provider requires HMAC or bearer-token
    /// signature validation. The validator is responsible for retrieving the
    /// shared secret from Key Vault or a literal config value.
    ///
    /// # Errors
    ///
    /// Returns [`GenericProviderConfigError`] if the configuration is invalid.
    pub fn with_signature_validator(
        config: GenericProviderConfig,
        audit_logger: Option<Arc<dyn AuditLogger>>,
        signature_validator: Option<Arc<dyn SignatureValidator>>,
    ) -> Result<Self, GenericProviderConfigError> {
        config.validate()?;
        Ok(Self {
            config,
            audit_logger,
            signature_validator,
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
// Private helpers
// ============================================================================

/// Traverse a dot-separated JSON path and return a reference to the value.
///
/// Example: `resolve_json_path(&json, "project.id")` returns `&json["project"]["id"]`.
fn resolve_json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    path.split('.').fold(Some(value), |current, key| {
        current.and_then(|v| v.get(key))
    })
}

/// Resolve a [`FieldSource`] against the current request's raw headers and parsed body.
///
/// Returns `None` when the header or JSON path is absent from the request.
fn resolve_field_source(
    source: &FieldSource,
    raw_headers: &HashMap<String, String>,
    payload: &serde_json::Value,
) -> Option<String> {
    match source {
        FieldSource::Header { name } => {
            // HTTP headers are case-insensitive; the header_map is lowercased.
            raw_headers.get(&name.to_lowercase()).cloned()
        }
        FieldSource::JsonPath { path } => {
            let node = resolve_json_path(payload, path)?;
            if let Some(s) = node.as_str() {
                Some(s.to_string())
            } else if let Some(n) = node.as_i64() {
                Some(n.to_string())
            } else if let Some(n) = node.as_u64() {
                Some(n.to_string())
            } else {
                None
            }
        }
        FieldSource::Static { value } => Some(value.clone()),
        // AutoGenerate: each call produces a fresh ULID.
        FieldSource::AutoGenerate => Some(crate::EventId::new().as_str().to_string()),
    }
}

/// Constant-time byte comparison to prevent timing attacks.
///
/// Both slices must have the same length; returns `false` immediately if they
/// differ in length (this is not length-constant but length disclosure is
/// acceptable for bearer token validation where token length is known).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ============================================================================
// WebhookProcessor implementation
// ============================================================================

#[async_trait]
impl WebhookProcessor for GenericWebhookProvider {
    /// Process a generic webhook through the configured pipeline.
    ///
    /// # Behaviour
    ///
    /// 1. Build a lowercase header map from `request.raw_headers`.
    /// 2. Validate the HMAC/bearer signature when [`GenericProviderConfig::signature`] is configured.
    /// 3. Resolve the event type using [`GenericProviderConfig::event_type_source`] (falls back to `"webhook"`).
    /// 4. Dispatch to wrap-mode or direct-mode based on [`GenericProviderConfig::processing_mode`].
    ///
    /// # Errors
    ///
    /// Returns [`WebhookError::InvalidSignature`] when signature validation fails.
    /// Returns [`WebhookError::MalformedPayload`] when the JSON body cannot be parsed
    /// (wrap mode only).
    #[instrument(skip(self, request), fields(
        provider = %self.config.provider_id,
        mode = ?self.config.processing_mode,
    ))]
    async fn process_webhook(
        &self,
        request: WebhookRequest,
    ) -> Result<ProcessingOutput, WebhookError> {
        // 1. Validate signature when configured.
        if let Some(ref sig_config) = self.config.signature {
            let sig_header = sig_config.header_name.to_lowercase();
            match request.raw_headers.get(&sig_header) {
                Some(signature) => {
                    // event_type is available from headers (default "webhook" for generic providers)
                    self.validate_signature(&request.body, signature, &request.headers.event_type)
                        .await
                        .map_err(|e| WebhookError::InvalidSignature(e.to_string()))?;
                }
                None => {
                    return Err(WebhookError::InvalidSignature(format!(
                        "missing required signature header '{}' for provider '{}'",
                        sig_config.header_name, self.config.provider_id
                    )));
                }
            }
        }

        // 2. Parse the body as JSON (needed for event_type extraction in both modes and
        //    for field extraction in wrap mode). In direct mode we retain the original bytes.
        let payload: serde_json::Value = if self.config.processing_mode == ProcessingMode::Wrap
            || self.config.event_type_source.as_ref().map_or(false, |s| {
                matches!(s, FieldSource::JsonPath { .. })
            }) {
            serde_json::from_slice(&request.body).unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        // 3. Resolve the event type.
        let event_type = self
            .config
            .event_type_source
            .as_ref()
            .and_then(|src| resolve_field_source(src, &request.raw_headers, &payload))
            .unwrap_or_else(|| "webhook".to_string());

        // 4. Dispatch by processing mode.
        match self.config.processing_mode {
            ProcessingMode::Wrap => {
                self.process_wrap_mode(request, payload, &event_type).await
            }
            ProcessingMode::Direct => self.process_direct_mode(request, &event_type).await,
        }
    }

    /// Validate the webhook signature using the algorithm specified in [`SignatureConfig`].
    ///
    /// Supported algorithms:
    /// - [`SignatureAlgorithm::HmacSha256`]: Full HMAC-SHA256 with constant-time comparison.
    ///   The signature may carry a `sha256=` prefix (GitHub / Stripe style).
    /// - [`SignatureAlgorithm::HmacSha1`]: HMAC-SHA1 with `sha1=` prefix support (legacy).
    /// - [`SignatureAlgorithm::BearerToken`]: Constant-time equality between the header value
    ///   and the secret (Jira/Slack style).
    ///
    /// When no [`GenericProviderConfig::signature`] is configured, or no validator is wired,
    /// the method returns `Ok(())` immediately.
    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        event_type: &str,
    ) -> Result<(), ValidationError> {
        let sig_config = match &self.config.signature {
            Some(c) => c,
            None => return Ok(()), // No signature validation configured
        };

        let validator = match &self.signature_validator {
            Some(v) => v,
            None => {
                warn!(
                    provider = %self.config.provider_id,
                    "Signature validation configured but no validator provided; skipping"
                );
                return Ok(());
            }
        };

        // Retrieve the secret from the validator (Key Vault or literal).
        let secret = validator
            .get_webhook_secret(event_type)
            .await
            .map_err(|e| ValidationError::InvalidFormat {
                field: "signature".to_string(),
                message: format!("failed to retrieve webhook secret: {}", e),
            })?;

        match sig_config.algorithm {
            SignatureAlgorithm::HmacSha256 => {
                use hmac::{Hmac, Mac};
                use sha2::Sha256;
                type HmacSha256 = Hmac<Sha256>;

                let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| {
                    ValidationError::InvalidFormat {
                        field: "signature".to_string(),
                        message: "failed to initialize HMAC-SHA256".to_string(),
                    }
                })?;
                mac.update(payload);

                let hex_sig = signature.strip_prefix("sha256=").unwrap_or(signature);
                let sig_bytes = hex::decode(hex_sig).map_err(|_| ValidationError::InvalidFormat {
                    field: "signature".to_string(),
                    message: "invalid hex encoding in HMAC-SHA256 signature".to_string(),
                })?;

                mac.verify_slice(&sig_bytes).map_err(|_| ValidationError::InvalidFormat {
                    field: "signature".to_string(),
                    message: "HMAC-SHA256 signature mismatch".to_string(),
                })?;
            }

            SignatureAlgorithm::HmacSha1 => {
                use hmac::{Hmac, Mac};
                use sha1::Sha1;
                type HmacSha1 = Hmac<Sha1>;

                let mut mac = HmacSha1::new_from_slice(secret.as_bytes()).map_err(|_| {
                    ValidationError::InvalidFormat {
                        field: "signature".to_string(),
                        message: "failed to initialize HMAC-SHA1".to_string(),
                    }
                })?;
                mac.update(payload);

                let hex_sig = signature.strip_prefix("sha1=").unwrap_or(signature);
                let sig_bytes = hex::decode(hex_sig).map_err(|_| ValidationError::InvalidFormat {
                    field: "signature".to_string(),
                    message: "invalid hex encoding in HMAC-SHA1 signature".to_string(),
                })?;

                mac.verify_slice(&sig_bytes).map_err(|_| ValidationError::InvalidFormat {
                    field: "signature".to_string(),
                    message: "HMAC-SHA1 signature mismatch".to_string(),
                })?;
            }

            SignatureAlgorithm::BearerToken => {
                // Constant-time comparison between header value and the secret.
                if !constant_time_eq(signature.as_bytes(), secret.as_bytes()) {
                    return Err(ValidationError::InvalidFormat {
                        field: "signature".to_string(),
                        message: "bearer token mismatch".to_string(),
                    });
                }
            }
        }

        tracing::info!(
            provider = %self.config.provider_id,
            algorithm = ?sig_config.algorithm,
            "Webhook signature validated successfully"
        );

        Ok(())
    }

    /// Store the raw payload for audit and replay.
    ///
    /// Returns a placeholder [`StorageReference`] because generic providers do
    /// not yet have a dedicated blob-storage adapter. The delivery ID is
    /// embedded in the path so traces are still correlatable.
    async fn store_raw_payload(
        &self,
        request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        Ok(StorageReference {
            blob_path: format!("not-stored/{}", request.delivery_id()),
            stored_at: crate::Timestamp::now(),
            size_bytes: request.body.len() as u64,
        })
    }

    /// Normalise a generic webhook payload into a [`WrappedEvent`].
    ///
    /// Only valid in **wrap mode**. Extracts the repository identifier,
    /// optional entity ID, and optional action from the JSON body via the
    /// paths configured in [`FieldExtractionConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`NormalizationError::MissingRequiredField`] when the body
    /// cannot be parsed as JSON.
    async fn normalize_event(
        &self,
        request: &WebhookRequest,
    ) -> Result<WrappedEvent, NormalizationError> {
        let payload: serde_json::Value =
            serde_json::from_slice(&request.body).map_err(|e| {
                NormalizationError::MissingRequiredField {
                    field: format!("body (JSON parse error: {})", e),
                }
            })?;

        let event_type = self
            .config
            .event_type_source
            .as_ref()
            .and_then(|src| resolve_field_source(src, &request.raw_headers, &payload))
            .unwrap_or_else(|| "webhook".to_string());

        let action = self
            .config
            .field_extraction
            .as_ref()
            .and_then(|e| e.action_path.as_deref())
            .and_then(|path| resolve_json_path(&payload, path))
            .and_then(|v| v.as_str())
            .map(String::from);

        let event = WrappedEvent::new(
            self.config.provider_id.clone(),
            event_type,
            action,
            None, // Generic providers do not impose session-based ordering
            payload,
        );

        tracing::info!(
            provider = %self.config.provider_id,
            event_id = %event.event_id,
            event_type = %event.event_type,
            "Generic provider wrapped event normalised"
        );

        Ok(event)
    }
}

// ============================================================================
// Private mode dispatch helpers
// ============================================================================

impl GenericWebhookProvider {
    /// Process a webhook in wrap mode — normalise to [`WrappedEvent`].
    async fn process_wrap_mode(
        &self,
        request: WebhookRequest,
        payload: serde_json::Value,
        event_type: &str,
    ) -> Result<ProcessingOutput, WebhookError> {
        // In wrap mode the body must be valid JSON (checked during process_webhook).
        // If the caller passed Null it means JSON parsing already failed.
        if payload.is_null() && !request.body.is_empty() {
            return Err(WebhookError::MalformedPayload {
                message: format!(
                    "provider '{}': wrap mode requires a valid JSON body",
                    self.config.provider_id
                ),
            });
        }

        let extraction = self.config.field_extraction.as_ref()
            .expect("field_extraction required for wrap mode (validated at construction)");

        // Extract action from the configured path.
        let action = extraction
            .action_path
            .as_deref()
            .and_then(|path| resolve_json_path(&payload, path))
            .and_then(|v| v.as_str())
            .map(String::from);

        // Build the WrappedEvent.
        let event = WrappedEvent::new(
            self.config.provider_id.clone(),
            event_type.to_string(),
            action,
            None, // Generic providers do not support session-based ordering
            payload,
        );

        tracing::info!(
            provider = %self.config.provider_id,
            event_id = %event.event_id,
            event_type = %event.event_type,
            "Generic provider wrap-mode output"
        );

        Ok(ProcessingOutput::Wrapped(event))
    }

    /// Process a webhook in direct mode — forward the raw payload as-is.
    async fn process_direct_mode(
        &self,
        request: WebhookRequest,
        event_type: &str,
    ) -> Result<ProcessingOutput, WebhookError> {
        let metadata = DirectQueueMetadata::new(
            self.config.provider_id.as_str(),
            request.headers.content_type.as_str(),
        );

        tracing::info!(
            provider = %self.config.provider_id,
            event_type = %event_type,
            event_id = %metadata.event_id(),
            target_queue = ?self.config.target_queue,
            "Generic provider direct-mode output — forwarding raw payload"
        );

        Ok(ProcessingOutput::Direct {
            payload: request.body,
            metadata,
        })
    }
}

#[cfg(test)]
#[path = "generic_provider_tests.rs"]
mod tests;
