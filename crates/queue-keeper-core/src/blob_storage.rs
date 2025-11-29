//! # Blob Storage Interface
//!
//! Provides abstraction for webhook payload persistence and audit trail storage.
//!
//! See specs/interfaces/blob-storage.md for complete specification.

use crate::{EventId, Repository, Timestamp};
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;

// Custom serialization for Bytes
mod bytes_serde {
    use super::*;

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes.as_ref())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        Ok(Bytes::from(vec))
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute SHA-256 checksum of data
///
/// Returns hex-encoded checksum string for tamper detection.
///
/// # Examples
///
/// ```
/// use queue_keeper_core::blob_storage::compute_checksum;
/// use bytes::Bytes;
///
/// let data = Bytes::from("test data");
/// let checksum = compute_checksum(&data);
/// assert_eq!(checksum.len(), 64); // SHA-256 hex is 64 characters
/// ```
pub fn compute_checksum(data: &Bytes) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Verify checksum matches expected value
///
/// Performs constant-time comparison to prevent timing attacks.
///
/// # Examples
///
/// ```
/// use queue_keeper_core::blob_storage::{compute_checksum, verify_checksum};
/// use bytes::Bytes;
///
/// let data = Bytes::from("test data");
/// let checksum = compute_checksum(&data);
/// assert!(verify_checksum(&data, &checksum));
/// ```
pub fn verify_checksum(data: &Bytes, expected_checksum: &str) -> bool {
    let actual_checksum = compute_checksum(data);
    // Use constant-time comparison to prevent timing attacks
    constant_time_eq(actual_checksum.as_bytes(), expected_checksum.as_bytes())
}

/// Constant-time string comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

// ============================================================================
// Core Trait
// ============================================================================

/// Interface for blob storage operations
///
/// Abstracts blob storage for webhook payload persistence and replay capabilities.
/// All implementations must support immutable storage with strong consistency.
///
/// # Examples
///
/// ```no_run
/// use queue_keeper_core::{EventId, blob_storage::*};
/// # async fn example(storage: impl BlobStorage) -> Result<(), BlobStorageError> {
/// let event_id = EventId::new();
/// let payload = WebhookPayload {
///     body: bytes::Bytes::from("{}"),
///     headers: std::collections::HashMap::new(),
///     metadata: PayloadMetadata {
///         event_id: event_id.clone(),
///         event_type: "pull_request".to_string(),
///         repository: None,
///         signature_valid: true,
///         received_at: queue_keeper_core::Timestamp::now(),
///         delivery_id: Some("test-delivery".to_string()),
///     },
/// };
///
/// // Store payload
/// let metadata = storage.store_payload(&event_id, &payload).await?;
/// println!("Stored at: {}", metadata.blob_path);
///
/// // Retrieve for replay
/// if let Some(stored) = storage.get_payload(&event_id).await? {
///     println!("Retrieved {} bytes", stored.metadata.size_bytes);
/// }
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait BlobStorage: Send + Sync {
    /// Store webhook payload with metadata
    ///
    /// Creates an immutable blob containing the webhook payload and metadata.
    /// The blob path follows the convention:
    /// `webhook-payloads/year={year}/month={month}/day={day}/hour={hour}/{event_id}.json`
    ///
    /// # Arguments
    ///
    /// * `event_id` - Unique identifier for the event
    /// * `payload` - Webhook payload with body, headers, and metadata
    ///
    /// # Returns
    ///
    /// Metadata about the stored blob including path and size.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Storage service is unavailable
    /// - Authentication fails
    /// - Storage quota exceeded
    /// - Network timeout occurs
    async fn store_payload(
        &self,
        event_id: &EventId,
        payload: &WebhookPayload,
    ) -> Result<BlobMetadata, BlobStorageError>;

    /// Retrieve stored payload by event ID
    ///
    /// Reads the immutable blob and returns the complete stored webhook.
    /// Returns `None` if the blob does not exist.
    ///
    /// # Arguments
    ///
    /// * `event_id` - Unique identifier for the event
    ///
    /// # Returns
    ///
    /// Optional stored webhook with metadata and payload, or None if not found.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Storage service is unavailable
    /// - Authentication fails
    /// - Network timeout occurs
    async fn get_payload(
        &self,
        event_id: &EventId,
    ) -> Result<Option<StoredWebhook>, BlobStorageError>;

    /// List payloads by filter criteria
    ///
    /// Queries storage for payloads matching the filter. Useful for
    /// replay scenarios and audit queries.
    ///
    /// # Arguments
    ///
    /// * `filter` - Filter criteria including date range, repository, event type
    ///
    /// # Returns
    ///
    /// List of blob metadata for matching payloads.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Storage service is unavailable
    /// - Authentication fails
    /// - Invalid filter parameters
    async fn list_payloads(
        &self,
        filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError>;

    /// Delete payload (for retention policy)
    ///
    /// Removes a blob from storage. Used for retention policy enforcement.
    /// Once deleted, the payload cannot be retrieved.
    ///
    /// # Arguments
    ///
    /// * `event_id` - Unique identifier for the event to delete
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Blob does not exist
    /// - Storage service is unavailable
    /// - Permission denied
    async fn delete_payload(&self, event_id: &EventId) -> Result<(), BlobStorageError>;

    /// Check blob storage health
    ///
    /// Verifies storage connectivity and reports health status including
    /// performance metrics.
    ///
    /// # Returns
    ///
    /// Health status with connectivity, metrics, and any error information.
    ///
    /// # Errors
    ///
    /// Returns error if health check operation itself fails.
    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError>;
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Webhook payload with metadata for storage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebhookPayload {
    /// Raw webhook payload bytes
    #[serde(with = "bytes_serde")]
    pub body: Bytes,

    /// HTTP headers from webhook request
    pub headers: HashMap<String, String>,

    /// Event metadata extracted during processing
    pub metadata: PayloadMetadata,
}

/// Metadata extracted during webhook processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PayloadMetadata {
    /// Event ID (ULID)
    pub event_id: EventId,

    /// GitHub event type (e.g., "pull_request", "issues")
    ///
    /// Note: This is intentionally a String rather than an enum to match:
    /// - GitHub's webhook header format (X-GitHub-Event sends strings)
    /// - The pattern used throughout queue-keeper-core and github-bot-sdk
    /// - Forward compatibility (GitHub can add new event types without breaking us)
    ///
    /// While the GitHub SDK has typed event structs (PullRequestEvent, IssueEvent, etc.)
    /// for parsing payloads, the event_type discriminator remains a string for flexibility.
    pub event_type: String,

    /// Repository information (if available)
    pub repository: Option<Repository>,

    /// Signature validation status
    pub signature_valid: bool,

    /// Processing timestamp
    pub received_at: Timestamp,

    /// GitHub delivery ID
    pub delivery_id: Option<String>,
}

/// Metadata about stored blob
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlobMetadata {
    /// Event ID used as blob identifier
    pub event_id: EventId,

    /// Blob path in storage
    pub blob_path: String,

    /// Size of stored payload in bytes
    pub size_bytes: u64,

    /// Content type (always application/json)
    pub content_type: String,

    /// When blob was created
    pub created_at: Timestamp,

    /// SHA-256 checksum of the stored payload (hex-encoded)
    pub checksum_sha256: String,

    /// Payload metadata
    pub metadata: PayloadMetadata,
}

/// Complete webhook data retrieved from storage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredWebhook {
    /// Blob metadata
    pub metadata: BlobMetadata,

    /// Original webhook payload
    pub payload: WebhookPayload,
}

/// Filter criteria for listing stored payloads
#[derive(Debug, Clone, Default)]
pub struct PayloadFilter {
    /// Date range for filtering
    pub date_range: Option<DateRange>,

    /// Repository filter (full name: "owner/repo")
    pub repository: Option<String>,

    /// Event type filter (e.g., "pull_request")
    pub event_type: Option<String>,

    /// Maximum number of results
    pub limit: Option<usize>,

    /// Skip this many results (for pagination)
    pub offset: Option<usize>,
}

/// Date range for filtering
#[derive(Debug, Clone)]
pub struct DateRange {
    /// Start of date range (inclusive)
    pub start: Timestamp,

    /// End of date range (exclusive)
    pub end: Timestamp,
}

/// Health status of blob storage
#[derive(Debug, Clone)]
pub struct StorageHealthStatus {
    /// Overall health status
    pub healthy: bool,

    /// Connection status
    pub connected: bool,

    /// Last successful operation
    pub last_success: Option<Timestamp>,

    /// Error message if unhealthy
    pub error_message: Option<String>,

    /// Performance metrics
    pub metrics: StorageMetrics,
}

/// Storage performance metrics
#[derive(Debug, Clone)]
pub struct StorageMetrics {
    /// Average write latency (milliseconds)
    pub avg_write_latency_ms: f64,

    /// Average read latency (milliseconds)
    pub avg_read_latency_ms: f64,

    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during blob storage operations
#[derive(Debug, Error)]
pub enum BlobStorageError {
    /// Connection to storage service failed
    #[error("Connection failed: {message}")]
    ConnectionFailed { message: String },

    /// Authentication with storage service failed
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    /// Blob not found for given event ID
    #[error("Blob not found: {event_id}")]
    BlobNotFound { event_id: EventId },

    /// Permission denied for operation
    #[error("Permission denied: {operation}")]
    PermissionDenied { operation: String },

    /// Storage quota exceeded
    #[error("Storage quota exceeded")]
    QuotaExceeded,

    /// Invalid blob path
    #[error("Invalid blob path: {path}")]
    InvalidPath { path: String },

    /// Serialization failed
    #[error("Serialization failed: {message}")]
    SerializationFailed { message: String },

    /// Network timeout
    #[error("Network timeout: {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Checksum mismatch detected (tampered data)
    #[error("Checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    /// Internal storage error
    #[error("Internal storage error: {message}")]
    InternalError { message: String },
}

impl BlobStorageError {
    /// Check if error is transient and worth retrying
    ///
    /// Transient errors are temporary conditions that may resolve:
    /// - Connection failures (network issues)
    /// - Timeouts (temporary overload)
    /// - Internal errors (transient service issues)
    ///
    /// Permanent errors should not be retried:
    /// - Authentication failures (credentials invalid)
    /// - Blob not found (won't appear with retry)
    /// - Permission denied (won't change)
    /// - Quota exceeded (requires intervention)
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed { .. } | Self::Timeout { .. } | Self::InternalError { .. }
        )
    }

    /// Check if error indicates data corruption or tampering
    ///
    /// Returns true for errors that indicate the stored data has been modified
    /// or corrupted. These errors require investigation and should not be retried.
    pub fn is_corrupted(&self) -> bool {
        matches!(self, Self::ChecksumMismatch { .. })
    }
}

// ============================================================================
// Helper Implementations
// ============================================================================

impl EventId {
    /// Generate blob path for storage
    ///
    /// Creates immutable path following convention:
    /// `webhook-payloads/year={year}/month={month}/day={day}/hour={hour}/{event_id}.json`
    ///
    /// # Examples
    ///
    /// ```
    /// use queue_keeper_core::EventId;
    ///
    /// let event_id = EventId::new();
    /// let path = event_id.to_blob_path();
    /// assert!(path.starts_with("webhook-payloads/year="));
    /// assert!(path.ends_with(".json"));
    /// ```
    pub fn to_blob_path(&self) -> String {
        let timestamp = self.timestamp();
        format!(
            "webhook-payloads/year={}/month={:02}/day={:02}/hour={:02}/{}.json",
            timestamp.year(),
            timestamp.month(),
            timestamp.day(),
            timestamp.hour(),
            self
        )
    }

    /// Extract timestamp from ULID for path generation
    fn timestamp(&self) -> Timestamp {
        // ULID contains milliseconds since Unix epoch in first 48 bits
        Timestamp::now() // TODO: Extract actual timestamp from ULID
    }
}

#[cfg(test)]
#[path = "blob_storage_tests.rs"]
mod tests;
