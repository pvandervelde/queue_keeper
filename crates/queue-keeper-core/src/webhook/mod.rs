//! # Webhook Processing Module
//!
//! Handles GitHub webhook validation, normalization, and processing.
//!
//! See specs/interfaces/webhook-processing.md for complete specification.

use crate::{
    CorrelationId, EventId, Repository, RepositoryId, SessionId, Timestamp, User, UserId, UserType,
    ValidationError,
};
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

// ============================================================================
// Core Types
// ============================================================================

/// Raw HTTP request data from GitHub webhooks
#[derive(Debug, Clone)]
pub struct WebhookRequest {
    pub headers: WebhookHeaders,
    pub body: Bytes,
    pub received_at: Timestamp,
}

impl WebhookRequest {
    /// Create new webhook request
    pub fn new(headers: WebhookHeaders, body: Bytes) -> Self {
        Self {
            headers,
            body,
            received_at: Timestamp::now(),
        }
    }

    /// Get event type from headers
    pub fn event_type(&self) -> &str {
        &self.headers.event_type
    }

    /// Get delivery ID from headers
    pub fn delivery_id(&self) -> &str {
        &self.headers.delivery_id
    }

    /// Get signature from headers if present
    pub fn signature(&self) -> Option<&str> {
        self.headers.signature.as_deref()
    }
}

/// GitHub-specific HTTP headers required for processing
#[derive(Debug, Clone)]
pub struct WebhookHeaders {
    pub event_type: String,         // X-GitHub-Event
    pub delivery_id: String,        // X-GitHub-Delivery
    pub signature: Option<String>,  // X-Hub-Signature-256
    pub user_agent: Option<String>, // User-Agent
    pub content_type: String,       // Content-Type
}

impl WebhookHeaders {
    /// Parse headers from HTTP header map
    pub fn from_http_headers(headers: &HashMap<String, String>) -> Result<Self, ValidationError> {
        let event_type = headers
            .get("x-github-event")
            .or_else(|| headers.get("X-GitHub-Event"))
            .ok_or_else(|| ValidationError::Required {
                field: "X-GitHub-Event".to_string(),
            })?
            .clone();

        let delivery_id = headers
            .get("x-github-delivery")
            .or_else(|| headers.get("X-GitHub-Delivery"))
            .ok_or_else(|| ValidationError::Required {
                field: "X-GitHub-Delivery".to_string(),
            })?
            .clone();

        let signature = headers
            .get("x-hub-signature-256")
            .or_else(|| headers.get("X-Hub-Signature-256"))
            .cloned();

        let user_agent = headers
            .get("user-agent")
            .or_else(|| headers.get("User-Agent"))
            .cloned();

        let content_type = headers
            .get("content-type")
            .or_else(|| headers.get("Content-Type"))
            .unwrap_or(&"application/json".to_string())
            .clone();

        let headers = Self {
            event_type,
            delivery_id,
            signature,
            user_agent,
            content_type,
        };

        headers.validate()?;
        Ok(headers)
    }

    /// Validate header values
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.event_type.is_empty() {
            return Err(ValidationError::Required {
                field: "event_type".to_string(),
            });
        }

        if self.delivery_id.is_empty() {
            return Err(ValidationError::Required {
                field: "delivery_id".to_string(),
            });
        }

        // Validate delivery ID is UUID format
        if uuid::Uuid::parse_str(&self.delivery_id).is_err() {
            return Err(ValidationError::InvalidFormat {
                field: "delivery_id".to_string(),
                message: "must be valid UUID".to_string(),
            });
        }

        // Signature required for non-ping events
        if self.event_type != "ping" && self.signature.is_none() {
            return Err(ValidationError::Required {
                field: "signature".to_string(),
            });
        }

        // Content type must be JSON
        if !self.content_type.starts_with("application/json") {
            return Err(ValidationError::InvalidFormat {
                field: "content_type".to_string(),
                message: "must be application/json".to_string(),
            });
        }

        Ok(())
    }
}

/// Normalized event structure after webhook processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: EventId,
    pub event_type: String,
    pub action: Option<String>,
    pub repository: Repository,
    pub entity: EventEntity,
    pub session_id: SessionId,
    pub correlation_id: CorrelationId,
    pub occurred_at: Timestamp,
    pub processed_at: Timestamp,
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    /// Create new event envelope
    pub fn new(
        event_type: String,
        action: Option<String>,
        repository: Repository,
        entity: EventEntity,
        payload: serde_json::Value,
    ) -> Self {
        let event_id = EventId::new();
        let session_id = Self::generate_session_id(&repository, &entity);
        let correlation_id = CorrelationId::new();
        let now = Timestamp::now();

        Self {
            event_id,
            event_type,
            action,
            repository,
            entity,
            session_id,
            correlation_id,
            occurred_at: now,
            processed_at: now,
            payload,
        }
    }

    /// Generate session ID from repository and entity
    fn generate_session_id(repository: &Repository, entity: &EventEntity) -> SessionId {
        let entity_type = entity.entity_type();
        let entity_id = entity.entity_id();

        SessionId::from_parts(
            repository.owner_name(),
            repository.repo_name(),
            entity_type,
            &entity_id,
        )
    }
}

/// The primary GitHub object affected by the event (for session grouping)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventEntity {
    PullRequest { number: u32 },
    Issue { number: u32 },
    Branch { name: String },
    Release { tag: String },
    Repository,
    Unknown,
}

impl EventEntity {
    /// Extract entity from payload based on event type
    pub fn from_payload(event_type: &str, payload: &serde_json::Value) -> Self {
        match event_type {
            "pull_request" | "pull_request_review" | "pull_request_review_comment" => {
                if let Some(pr) = payload.get("pull_request") {
                    if let Some(number) = pr.get("number").and_then(|n| n.as_u64()) {
                        return Self::PullRequest {
                            number: number as u32,
                        };
                    }
                }
            }
            "issues" | "issue_comment" => {
                if let Some(issue) = payload.get("issue") {
                    if let Some(number) = issue.get("number").and_then(|n| n.as_u64()) {
                        return Self::Issue {
                            number: number as u32,
                        };
                    }
                }
            }
            "push" | "create" | "delete" => {
                if let Some(ref_str) = payload.get("ref").and_then(|r| r.as_str()) {
                    if let Some(branch_name) = ref_str.strip_prefix("refs/heads/") {
                        return Self::Branch {
                            name: branch_name.to_string(),
                        };
                    }
                }
            }
            "release" => {
                if let Some(release) = payload.get("release") {
                    if let Some(tag) = release.get("tag_name").and_then(|t| t.as_str()) {
                        return Self::Release {
                            tag: tag.to_string(),
                        };
                    }
                }
            }
            "repository" => {
                return Self::Repository;
            }
            _ => {}
        }

        Self::Unknown
    }

    /// Get entity type string
    pub fn entity_type(&self) -> &'static str {
        match self {
            Self::PullRequest { .. } => "pull_request",
            Self::Issue { .. } => "issue",
            Self::Branch { .. } => "branch",
            Self::Release { .. } => "release",
            Self::Repository => "repository",
            Self::Unknown => "unknown",
        }
    }

    /// Get entity ID string
    pub fn entity_id(&self) -> String {
        match self {
            Self::PullRequest { number } => number.to_string(),
            Self::Issue { number } => number.to_string(),
            Self::Branch { name } => name.clone(),
            Self::Release { tag } => tag.clone(),
            Self::Repository => "repository".to_string(),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Top-level error for webhook processing failures
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("Webhook validation failed: {0}")]
    Validation(#[from] ValidationError),

    #[error("Signature validation failed: {0}")]
    InvalidSignature(String),

    #[error("Payload storage failed: {0}")]
    Storage(#[from] StorageError),

    #[error("Event normalization failed: {0}")]
    Normalization(#[from] NormalizationError),

    #[error("Unknown event type: {event_type}")]
    UnknownEventType { event_type: String },

    #[error("Malformed payload: {message}")]
    MalformedPayload { message: String },

    #[error("JSON parsing failed: {0}")]
    JsonParsing(#[from] serde_json::Error),
}

impl WebhookError {
    /// Check if error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Storage(storage_error) => storage_error.is_transient(),
            Self::InvalidSignature(_) => false,
            Self::UnknownEventType { .. } => false,
            Self::MalformedPayload { .. } => false,
            Self::Validation(_) => false,
            Self::Normalization(_) => false,
            Self::JsonParsing(_) => false,
        }
    }

    /// Get error category for monitoring
    pub fn error_category(&self) -> crate::ErrorCategory {
        match self {
            Self::InvalidSignature(_) => crate::ErrorCategory::Security,
            Self::UnknownEventType { .. } => crate::ErrorCategory::Permanent,
            Self::MalformedPayload { .. } => crate::ErrorCategory::Permanent,
            Self::Storage(storage_error) => {
                if storage_error.is_transient() {
                    crate::ErrorCategory::Transient
                } else {
                    crate::ErrorCategory::Permanent
                }
            }
            Self::Validation(_) => crate::ErrorCategory::Permanent,
            Self::Normalization(_) => crate::ErrorCategory::Permanent,
            Self::JsonParsing(_) => crate::ErrorCategory::Permanent,
        }
    }

    /// Check if error should be retried
    pub fn should_retry(&self) -> bool {
        self.is_transient()
    }
}

/// Errors during event normalization process
#[derive(Debug, thiserror::Error)]
pub enum NormalizationError {
    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },

    #[error("Invalid field format: {field} - {message}")]
    InvalidFieldFormat { field: String, message: String },

    #[error("Repository extraction failed: {0}")]
    RepositoryExtraction(#[from] ExtractionError),

    #[error("JSON parsing failed: {0}")]
    JsonParsing(#[from] serde_json::Error),
}

/// Errors during repository/entity extraction
#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    #[error("Required field missing: {field}")]
    MissingField { field: String },

    #[error("Invalid field type: {field}")]
    InvalidFieldType { field: String },
}

/// Errors during payload storage operations
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Storage operation failed: {message}")]
    OperationFailed { message: String },

    #[error("Storage not available: {message}")]
    Unavailable { message: String },

    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    #[error("Payload too large: {size} bytes")]
    PayloadTooLarge { size: usize },
}

impl StorageError {
    /// Check if storage error is transient
    pub fn is_transient(&self) -> bool {
        match self {
            Self::OperationFailed { .. } => true,
            Self::Unavailable { .. } => true,
            Self::PermissionDenied { .. } => false,
            Self::PayloadTooLarge { .. } => false,
        }
    }
}

/// Storage reference for stored payloads
#[derive(Debug, Clone)]
pub struct StorageReference {
    pub blob_path: String,
    pub stored_at: Timestamp,
    pub size_bytes: u64,
}

/// Validation status for stored payloads
#[derive(Debug, Clone)]
pub enum ValidationStatus {
    Valid,
    InvalidSignature,
    MalformedPayload,
    UnknownEvent,
}

// ============================================================================
// Core Operations (Traits)
// ============================================================================

/// Main interface for webhook processing pipeline
#[async_trait]
pub trait WebhookProcessor: Send + Sync {
    /// Process complete webhook request through the pipeline
    async fn process_webhook(&self, request: WebhookRequest)
        -> Result<EventEnvelope, WebhookError>;

    /// Validate webhook signature
    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        event_type: &str,
    ) -> Result<(), ValidationError>;

    /// Store raw payload for audit/replay
    async fn store_raw_payload(
        &self,
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError>;

    /// Normalize event to standard format
    async fn normalize_event(
        &self,
        request: &WebhookRequest,
    ) -> Result<EventEnvelope, NormalizationError>;
}

/// Interface for GitHub webhook signature validation
#[async_trait]
pub trait SignatureValidator: Send + Sync {
    /// Validate webhook signature using HMAC-SHA256
    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        secret_key: &str,
    ) -> Result<(), ValidationError>;

    /// Get webhook secret for event type
    async fn get_webhook_secret(&self, event_type: &str) -> Result<String, SecretError>;

    /// Check if implementation supports constant-time comparison
    fn supports_constant_time_comparison(&self) -> bool;
}

/// Interface for persisting raw webhook payloads
#[async_trait]
pub trait PayloadStorer: Send + Sync {
    /// Store webhook payload with metadata
    async fn store_payload(
        &self,
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError>;

    /// Retrieve stored payload by reference
    async fn retrieve_payload(
        &self,
        storage_ref: &StorageReference,
    ) -> Result<WebhookRequest, StorageError>;

    /// List stored payloads with filters
    async fn list_payloads(
        &self,
        filters: PayloadFilters,
    ) -> Result<Vec<StorageReference>, StorageError>;
}

/// Interface for transforming GitHub payloads to standard event format
#[async_trait]
pub trait EventNormalizer: Send + Sync {
    /// Normalize webhook request to event envelope
    async fn normalize_event(
        &self,
        request: &WebhookRequest,
    ) -> Result<EventEnvelope, NormalizationError>;

    /// Extract repository information from payload
    fn extract_repository(
        &self,
        payload: &serde_json::Value,
    ) -> Result<Repository, ExtractionError>;

    /// Extract entity information from payload
    fn extract_entity(&self, event_type: &str, payload: &serde_json::Value) -> EventEntity;

    /// Generate session ID for repository and entity
    fn generate_session_id(&self, repository: &Repository, entity: &EventEntity) -> SessionId;
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Filters for listing stored payloads
#[derive(Debug, Clone)]
pub struct PayloadFilters {
    pub event_type: Option<String>,
    pub repository_id: Option<RepositoryId>,
    pub validation_status: Option<ValidationStatus>,
    pub start_date: Option<Timestamp>,
    pub end_date: Option<Timestamp>,
    pub limit: Option<usize>,
}

impl Default for PayloadFilters {
    fn default() -> Self {
        Self {
            event_type: None,
            repository_id: None,
            validation_status: None,
            start_date: None,
            end_date: None,
            limit: Some(100),
        }
    }
}

/// Error type for secret operations
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("Secret not found: {key}")]
    NotFound { key: String },

    #[error("Access denied to secret: {key}")]
    AccessDenied { key: String },

    #[error("Secret provider unavailable: {0}")]
    ProviderUnavailable(String),

    #[error("Invalid secret format: {key}")]
    InvalidFormat { key: String },
}

// ============================================================================
// Default Implementations
// ============================================================================

/// Webhook processor implementation with dependency injection
///
/// This implementation follows the dependency injection pattern to allow
/// for testability and flexibility. Optional dependencies can be omitted
/// for testing or when features are not yet implemented.
pub struct WebhookProcessorImpl {
    signature_validator: Option<std::sync::Arc<dyn SignatureValidator>>,
    payload_storer: Option<std::sync::Arc<dyn PayloadStorer>>,
}

impl WebhookProcessorImpl {
    /// Create new webhook processor with optional dependencies
    ///
    /// # Arguments
    ///
    /// * `signature_validator` - Optional signature validator for webhook authentication
    /// * `payload_storer` - Optional payload storer for audit trail
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use queue_keeper_core::webhook::WebhookProcessorImpl;
    ///
    /// // Create processor without dependencies (for testing)
    /// let processor = WebhookProcessorImpl::new(None, None);
    ///
    /// // Create processor with signature validation only
    /// // let validator = Arc::new(my_validator);
    /// // let processor = WebhookProcessorImpl::new(Some(validator), None);
    /// ```
    pub fn new(
        signature_validator: Option<std::sync::Arc<dyn SignatureValidator>>,
        payload_storer: Option<std::sync::Arc<dyn PayloadStorer>>,
    ) -> Self {
        Self {
            signature_validator,
            payload_storer,
        }
    }

    /// Extract repository information from payload
    ///
    /// Parses repository data from GitHub webhook payload, including
    /// repository metadata and owner information.
    ///
    /// # Errors
    ///
    /// Returns `NormalizationError` if:
    /// - Repository field is missing
    /// - Required repository fields are missing or invalid
    /// - Owner information is incomplete
    fn extract_repository(
        &self,
        payload: &serde_json::Value,
    ) -> Result<Repository, NormalizationError> {
        let repo_data =
            payload
                .get("repository")
                .ok_or_else(|| NormalizationError::MissingRequiredField {
                    field: "repository".to_string(),
                })?;

        let id = repo_data
            .get("id")
            .and_then(|i| i.as_u64())
            .ok_or_else(|| NormalizationError::MissingRequiredField {
                field: "repository.id".to_string(),
            })?;

        let name = repo_data
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| NormalizationError::MissingRequiredField {
                field: "repository.name".to_string(),
            })?
            .to_string();

        let full_name = repo_data
            .get("full_name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| NormalizationError::MissingRequiredField {
                field: "repository.full_name".to_string(),
            })?
            .to_string();

        let private = repo_data
            .get("private")
            .and_then(|p| p.as_bool())
            .unwrap_or(false);

        // Extract owner information
        let owner_data =
            repo_data
                .get("owner")
                .ok_or_else(|| NormalizationError::MissingRequiredField {
                    field: "repository.owner".to_string(),
                })?;

        let owner_id = owner_data
            .get("id")
            .and_then(|i| i.as_u64())
            .ok_or_else(|| NormalizationError::MissingRequiredField {
                field: "repository.owner.id".to_string(),
            })?;

        let owner_login = owner_data
            .get("login")
            .and_then(|l| l.as_str())
            .ok_or_else(|| NormalizationError::MissingRequiredField {
                field: "repository.owner.login".to_string(),
            })?
            .to_string();

        let owner_type = match owner_data.get("type").and_then(|t| t.as_str()) {
            Some("User") => UserType::User,
            Some("Bot") => UserType::Bot,
            Some("Organization") => UserType::Organization,
            _ => UserType::User, // Default fallback
        };

        let owner = User {
            id: UserId::new(owner_id),
            login: owner_login,
            user_type: owner_type,
        };

        let repository = Repository::new(RepositoryId::new(id), name, full_name, owner, private);

        Ok(repository)
    }
}

#[async_trait]
impl WebhookProcessor for WebhookProcessorImpl {
    async fn process_webhook(
        &self,
        request: WebhookRequest,
    ) -> Result<EventEnvelope, WebhookError> {
        info!(
            event_type = %request.event_type(),
            delivery_id = %request.delivery_id(),
            "Processing webhook request"
        );

        // 1. Validate headers and basic structure
        request.headers.validate()?;

        // 2. Validate webhook signature (if present and validator available)
        if let Some(signature) = request.signature() {
            self.validate_signature(&request.body, signature, request.event_type())
                .await?;
        }

        // 3. Store raw payload for audit/replay (if storer available)
        let validation_status = ValidationStatus::Valid;
        let _storage_ref = self.store_raw_payload(&request, validation_status).await?;

        // 4. Normalize to standard event format
        let event_envelope = self.normalize_event(&request).await?;

        info!(
            event_id = %event_envelope.event_id,
            session_id = %event_envelope.session_id,
            entity = ?event_envelope.entity,
            "Successfully processed webhook"
        );

        Ok(event_envelope)
    }

    async fn validate_signature(
        &self,
        payload: &[u8],
        signature: &str,
        event_type: &str,
    ) -> Result<(), ValidationError> {
        if let Some(validator) = &self.signature_validator {
            // Get webhook secret for this event type
            let secret = validator
                .get_webhook_secret(event_type)
                .await
                .map_err(|e| ValidationError::InvalidFormat {
                    field: "signature".to_string(),
                    message: format!("Failed to retrieve webhook secret: {}", e),
                })?;

            // Validate signature using constant-time comparison
            validator
                .validate_signature(payload, signature, &secret)
                .await?;

            info!(
                event_type = %event_type,
                "Webhook signature validated successfully"
            );
        } else {
            info!(
                event_type = %event_type,
                "Signature validation skipped - no validator configured"
            );
        }

        Ok(())
    }

    async fn store_raw_payload(
        &self,
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        if let Some(storer) = &self.payload_storer {
            // Store payload with metadata
            let storage_ref = storer.store_payload(request, validation_status).await?;

            info!(
                blob_path = %storage_ref.blob_path,
                size_bytes = storage_ref.size_bytes,
                "Webhook payload stored successfully"
            );

            Ok(storage_ref)
        } else {
            // No storer configured - return placeholder reference
            // This allows processing to continue without storage (useful for testing)
            info!("Payload storage skipped - no storer configured");

            Ok(StorageReference {
                blob_path: format!("not-stored/{}", request.delivery_id()),
                stored_at: Timestamp::now(),
                size_bytes: request.body.len() as u64,
            })
        }
    }

    async fn normalize_event(
        &self,
        request: &WebhookRequest,
    ) -> Result<EventEnvelope, NormalizationError> {
        // Parse JSON payload
        let payload: serde_json::Value = serde_json::from_slice(&request.body)?;

        // Extract repository (required for all events)
        let repository = self.extract_repository(&payload)?;

        // Extract entity based on event type
        let entity = EventEntity::from_payload(request.event_type(), &payload);

        // Extract action if present
        let action = payload
            .get("action")
            .and_then(|a| a.as_str())
            .map(String::from);

        // Create normalized event envelope
        let event = EventEnvelope::new(
            request.event_type().to_string(),
            action,
            repository,
            entity,
            payload,
        );

        info!(
            event_id = %event.event_id,
            event_type = %event.event_type,
            entity_type = %event.entity.entity_type(),
            "Event normalized successfully"
        );

        Ok(event)
    }
}

/// Default webhook processor implementation (backward compatibility)
///
/// This is a type alias for `WebhookProcessorImpl` without any dependencies.
/// Provided for backward compatibility with existing code.
pub type DefaultWebhookProcessor = WebhookProcessorImpl;

// Storage adapter
mod storage_adapter;
pub use storage_adapter::BlobStorageAdapter;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
