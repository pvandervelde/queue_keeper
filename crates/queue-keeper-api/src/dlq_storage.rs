//! # Dead Letter Queue Storage Module
//!
//! Provides persistence for failed events that could not be delivered to bot queues
//! after exhausting retries or encountering permanent failures.
//!
//! DLQ records preserve:
//! - The original event envelope
//! - Failure context (error messages, failed queues)
//! - Retry history (attempts made, timestamps)
//! - Sufficient information for later replay
//!
//! See specs/requirements/functional-requirements.md REQ-007 for DLQ requirements.
//! See specs/vocabulary.md "Dead Letter Queue" for concept definition.

use chrono::{Datelike, Timelike};
use queue_keeper_core::{
    blob_storage::{BlobStorage, BlobStorageError, WebhookPayload},
    webhook::WrappedEvent,
    BotName, EventId, QueueName, Repository, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

// ============================================================================
// DLQ Record Types
// ============================================================================

/// Reason for event ending up in DLQ
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DlqReason {
    /// Transient failures exhausted all retry attempts
    RetriesExhausted {
        /// Number of retry attempts made
        attempts: u32,
    },

    /// Permanent failure that cannot be retried
    PermanentFailure {
        /// Description of the permanent failure
        reason: String,
    },

    /// All target queues failed delivery
    AllQueuesFailed {
        /// Number of queues that failed
        queue_count: usize,
    },

    /// Routing error prevented queue delivery
    RoutingError {
        /// Routing error description
        error: String,
    },
}

/// Information about a failed queue delivery attempt
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FailedQueueInfo {
    /// Bot that owns the queue
    pub bot_name: String,

    /// Queue that failed
    pub queue_name: String,

    /// Error message from delivery attempt
    pub error: String,

    /// Whether this was a transient (potentially retryable) failure
    pub was_transient: bool,
}

/// Complete record of a failed event for DLQ storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedEventRecord {
    /// Event ID for correlation
    pub event_id: EventId,

    /// Original wrapped event
    pub event: WrappedEvent,

    /// Why this event ended up in DLQ
    pub reason: DlqReason,

    /// List of queues that failed (if applicable)
    pub failed_queues: Vec<FailedQueueInfo>,

    /// List of queues that succeeded (if any)
    pub successful_queues: Vec<String>,

    /// Total retry attempts made
    pub retry_attempts: u32,

    /// When the first delivery attempt was made
    pub first_attempt_at: Timestamp,

    /// When the event was moved to DLQ
    pub moved_to_dlq_at: Timestamp,

    /// Correlation ID for tracing
    pub correlation_id: String,
}

impl FailedEventRecord {
    /// Create a new DLQ record for an event
    pub fn new(
        event: WrappedEvent,
        reason: DlqReason,
        failed_queues: Vec<FailedQueueInfo>,
        successful_queues: Vec<String>,
        retry_attempts: u32,
        first_attempt_at: Timestamp,
    ) -> Self {
        Self {
            event_id: event.event_id,
            correlation_id: event.correlation_id.to_string(),
            event,
            reason,
            failed_queues,
            successful_queues,
            retry_attempts,
            first_attempt_at,
            moved_to_dlq_at: Timestamp::now(),
        }
    }

    /// Get the blob path for this DLQ record
    ///
    /// DLQ records are stored under `dlq/` prefix with time-based partitioning
    pub fn to_blob_path(&self) -> String {
        let timestamp = self.moved_to_dlq_at.as_datetime();
        format!(
            "dlq/year={}/month={:02}/day={:02}/hour={:02}/{}.json",
            timestamp.year(),
            timestamp.month(),
            timestamp.day(),
            timestamp.hour(),
            self.event_id
        )
    }
}

// ============================================================================
// DLQ Metadata (stored separately for listing without loading full records)
// ============================================================================

/// Metadata about a DLQ record for quick listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqMetadata {
    /// Event ID
    pub event_id: EventId,

    /// Blob path in storage
    pub blob_path: String,

    /// DLQ reason type (without details)
    pub reason_type: String,

    /// Number of failed queues
    pub failed_queue_count: usize,

    /// When moved to DLQ
    pub moved_to_dlq_at: Timestamp,

    /// Size in bytes
    pub size_bytes: u64,
}

// ============================================================================
// DLQ Storage Service
// ============================================================================

/// Service for persisting failed events to blob storage
///
/// This wraps the standard BlobStorage trait to provide DLQ-specific
/// operations with proper path conventions and metadata handling.
#[derive(Clone)]
pub struct DlqStorageService {
    storage: Arc<dyn BlobStorage>,
}

impl std::fmt::Debug for DlqStorageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DlqStorageService")
            .field("storage", &"<BlobStorage>")
            .finish()
    }
}

impl DlqStorageService {
    /// Create a new DLQ storage service
    pub fn new(storage: Arc<dyn BlobStorage>) -> Self {
        Self { storage }
    }

    /// Persist a failed event to DLQ storage
    ///
    /// # Arguments
    ///
    /// * `record` - The failed event record to store
    ///
    /// # Returns
    ///
    /// The blob path where the record was stored
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    pub async fn persist_failed_event(
        &self,
        record: &FailedEventRecord,
    ) -> Result<String, BlobStorageError> {
        let blob_path = record.to_blob_path();

        info!(
            event_id = %record.event_id,
            blob_path = %blob_path,
            reason = ?record.reason,
            failed_queues = record.failed_queues.len(),
            retry_attempts = record.retry_attempts,
            "Persisting failed event to DLQ"
        );

        // Serialize the record to JSON
        let json = serde_json::to_string_pretty(record).map_err(|e| {
            BlobStorageError::SerializationFailed {
                message: format!("Failed to serialize DLQ record: {}", e),
            }
        })?;

        // Create a WebhookPayload wrapper for storage
        // We use the blob storage interface but with DLQ-specific path
        let payload = WebhookPayload {
            body: bytes::Bytes::from(json),
            headers: std::collections::HashMap::new(),
            metadata: queue_keeper_core::blob_storage::PayloadMetadata {
                event_id: record.event_id,
                event_type: record.event.event_type.clone(),
                repository: record
                    .event
                    .payload
                    .get("repository")
                    .and_then(|r| serde_json::from_value::<Repository>(r.clone()).ok()),
                signature_valid: true,
                received_at: record.first_attempt_at,
                delivery_id: None,
            },
        };

        // Store the payload
        let metadata = self
            .storage
            .store_payload(&record.event_id, &payload)
            .await?;

        info!(
            event_id = %record.event_id,
            blob_path = %metadata.blob_path,
            size_bytes = metadata.size_bytes,
            "Successfully persisted failed event to DLQ"
        );

        Ok(metadata.blob_path)
    }

    /// Retrieve a failed event record from DLQ
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID to retrieve
    ///
    /// # Returns
    ///
    /// The failed event record if found
    pub async fn get_failed_event(
        &self,
        event_id: &EventId,
    ) -> Result<Option<FailedEventRecord>, BlobStorageError> {
        match self.storage.get_payload(event_id).await? {
            Some(stored) => {
                // Deserialize the DLQ record from the payload body
                let record: FailedEventRecord = serde_json::from_slice(&stored.payload.body)
                    .map_err(|e| BlobStorageError::SerializationFailed {
                        message: format!("Failed to deserialize DLQ record: {}", e),
                    })?;

                Ok(Some(record))
            }
            None => Ok(None),
        }
    }
}

// ============================================================================
// Helper Functions for Queue Delivery Integration
// ============================================================================

/// Create a FailedEventRecord from queue delivery failure context
///
/// This is a convenience function for use in the queue delivery module.
pub fn create_failed_event_record(
    event: WrappedEvent,
    failed_queues: Vec<(BotName, QueueName, String, bool)>,
    successful_queues: Vec<(BotName, QueueName)>,
    retry_attempts: u32,
    first_attempt_at: Timestamp,
    reason: DlqReason,
) -> FailedEventRecord {
    let failed_info: Vec<FailedQueueInfo> = failed_queues
        .into_iter()
        .map(|(bot, queue, error, was_transient)| FailedQueueInfo {
            bot_name: bot.as_str().to_string(),
            queue_name: queue.as_str().to_string(),
            error,
            was_transient,
        })
        .collect();

    let successful_names: Vec<String> = successful_queues
        .into_iter()
        .map(|(bot, queue)| format!("{}/{}", bot.as_str(), queue.as_str()))
        .collect();

    FailedEventRecord::new(
        event,
        reason,
        failed_info,
        successful_names,
        retry_attempts,
        first_attempt_at,
    )
}

/// Persist failed delivery to DLQ storage
///
/// This is the main entry point for the queue delivery module to persist
/// failed events. It handles error logging and returns success status.
///
/// # Arguments
///
/// * `dlq_service` - Optional DLQ storage service (if DLQ is enabled)
/// * `record` - The failed event record to persist
///
/// # Returns
///
/// `true` if persisted successfully, `false` if DLQ is disabled or storage failed
pub async fn persist_to_dlq(
    dlq_service: Option<&DlqStorageService>,
    record: &FailedEventRecord,
) -> bool {
    match dlq_service {
        Some(service) => match service.persist_failed_event(record).await {
            Ok(blob_path) => {
                info!(
                    event_id = %record.event_id,
                    blob_path = %blob_path,
                    "Event persisted to DLQ"
                );
                true
            }
            Err(e) => {
                error!(
                    event_id = %record.event_id,
                    error = %e,
                    "Failed to persist event to DLQ - event may be lost"
                );
                false
            }
        },
        None => {
            warn!(
                event_id = %record.event_id,
                "DLQ service not configured - failed event not persisted"
            );
            false
        }
    }
}

#[cfg(test)]
#[path = "dlq_storage_tests.rs"]
mod tests;
