//! # Filesystem Blob Storage Adapter
//!
//! Local filesystem implementation of BlobStorage trait for development and testing.

use crate::blob_storage::*;
use crate::{EventId, Timestamp};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Filesystem-based blob storage implementation
///
/// Stores blobs as JSON files in a local directory structure following
/// the standard partitioning scheme.
///
/// # Examples
///
/// ```no_run
/// use queue_keeper_core::adapters::FilesystemBlobStorage;
/// use std::path::PathBuf;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let storage = FilesystemBlobStorage::new(PathBuf::from("./data/blobs")).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct FilesystemBlobStorage {
    base_path: PathBuf,
}

impl FilesystemBlobStorage {
    /// Create new filesystem blob storage
    ///
    /// # Arguments
    ///
    /// * `base_path` - Base directory for blob storage
    ///
    /// # Errors
    ///
    /// Returns error if base path cannot be created or accessed.
    pub async fn new(base_path: PathBuf) -> Result<Self, BlobStorageError> {
        // Verify or create base directory
        fs::create_dir_all(&base_path)
            .await
            .map_err(|e| BlobStorageError::InternalError {
                message: format!("Failed to create base directory: {}", e),
            })?;

        Ok(Self { base_path })
    }

    /// Get full path for event ID
    fn get_blob_path(&self, event_id: &EventId) -> PathBuf {
        let relative_path = event_id.to_blob_path();
        self.base_path.join(relative_path)
    }
}

#[async_trait]
impl BlobStorage for FilesystemBlobStorage {
    async fn store_payload(
        &self,
        event_id: &EventId,
        payload: &WebhookPayload,
    ) -> Result<BlobMetadata, BlobStorageError> {
        let blob_path = self.get_blob_path(event_id);

        // Create parent directories
        if let Some(parent) = blob_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| BlobStorageError::InternalError {
                    message: format!("Failed to create directory structure: {}", e),
                })?;
        }

        // Create temporary metadata for initial serialization
        let created_at = Timestamp::now();
        let temp_metadata = BlobMetadata {
            event_id: *event_id,
            blob_path: event_id.to_blob_path(),
            size_bytes: 0, // Will be updated after writing
            content_type: "application/json".to_string(),
            created_at,
            checksum_sha256: String::new(), // Temporary placeholder
            metadata: payload.metadata.clone(),
        };

        let temp_webhook = StoredWebhook {
            metadata: temp_metadata,
            payload: payload.clone(),
        };

        // Compute checksum of the payload body (not the entire serialized JSON)
        let checksum = crate::blob_storage::compute_checksum(&payload.body);

        // Create final metadata with checksum
        let final_metadata = BlobMetadata {
            checksum_sha256: checksum.clone(),
            ..temp_webhook.metadata
        };

        let final_webhook = StoredWebhook {
            metadata: final_metadata,
            payload: payload.clone(),
        };

        // Final serialization with correct checksum
        let json = serde_json::to_string_pretty(&final_webhook).map_err(|e| {
            BlobStorageError::SerializationFailed {
                message: format!("Failed to serialize payload: {}", e),
            }
        })?;

        // Write to temporary file first (atomic write pattern)
        let temp_path = blob_path.with_extension("tmp");
        let mut file =
            fs::File::create(&temp_path)
                .await
                .map_err(|e| BlobStorageError::InternalError {
                    message: format!("Failed to create temp file: {}", e),
                })?;

        file.write_all(json.as_bytes())
            .await
            .map_err(|e| BlobStorageError::InternalError {
                message: format!("Failed to write payload: {}", e),
            })?;

        file.flush()
            .await
            .map_err(|e| BlobStorageError::InternalError {
                message: format!("Failed to flush file: {}", e),
            })?;

        // Rename to final path (atomic on most filesystems)
        fs::rename(&temp_path, &blob_path)
            .await
            .map_err(|e| BlobStorageError::InternalError {
                message: format!("Failed to rename temp file: {}", e),
            })?;

        // Get file size and update metadata
        let file_metadata =
            fs::metadata(&blob_path)
                .await
                .map_err(|e| BlobStorageError::InternalError {
                    message: format!("Failed to read file metadata: {}", e),
                })?;

        Ok(BlobMetadata {
            event_id: *event_id,
            blob_path: event_id.to_blob_path(),
            size_bytes: file_metadata.len(),
            content_type: "application/json".to_string(),
            created_at,
            checksum_sha256: checksum,
            metadata: payload.metadata.clone(),
        })
    }

    async fn get_payload(
        &self,
        event_id: &EventId,
    ) -> Result<Option<StoredWebhook>, BlobStorageError> {
        let blob_path = self.get_blob_path(event_id);

        // Check if file exists
        if !blob_path.exists() {
            return Ok(None);
        }

        // Read file contents
        let json =
            fs::read_to_string(&blob_path)
                .await
                .map_err(|e| BlobStorageError::InternalError {
                    message: format!("Failed to read blob: {}", e),
                })?;

        // Deserialize stored webhook
        let stored: StoredWebhook =
            serde_json::from_str(&json).map_err(|e| BlobStorageError::SerializationFailed {
                message: format!("Failed to deserialize payload: {}", e),
            })?;

        // Verify checksum against the payload body (not the entire JSON)
        let computed_checksum = crate::blob_storage::compute_checksum(&stored.payload.body);
        if !crate::blob_storage::verify_checksum(
            &stored.payload.body,
            &stored.metadata.checksum_sha256,
        ) {
            return Err(BlobStorageError::ChecksumMismatch {
                path: blob_path.display().to_string(),
                expected: stored.metadata.checksum_sha256.clone(),
                actual: computed_checksum,
            });
        }

        Ok(Some(stored))
    }

    async fn list_payloads(
        &self,
        filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError> {
        let mut results = Vec::new();

        // Walk the directory tree
        let base_path = self.base_path.join("webhook-payloads");
        if !base_path.exists() {
            return Ok(results);
        }

        // Recursively find all .json files
        let mut entries = vec![base_path];
        while let Some(path) = entries.pop() {
            let mut read_dir =
                fs::read_dir(&path)
                    .await
                    .map_err(|e| BlobStorageError::InternalError {
                        message: format!("Failed to read directory: {}", e),
                    })?;

            while let Some(entry) =
                read_dir
                    .next_entry()
                    .await
                    .map_err(|e| BlobStorageError::InternalError {
                        message: format!("Failed to read directory entry: {}", e),
                    })?
            {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    entries.push(entry_path);
                } else if entry_path.extension().and_then(|s| s.to_str()) == Some("json") {
                    // Read and parse this blob
                    if let Ok(json) = fs::read_to_string(&entry_path).await {
                        if let Ok(stored) = serde_json::from_str::<StoredWebhook>(&json) {
                            // Apply filters
                            let mut matches = true;

                            if let Some(ref repo_filter) = filter.repository {
                                if let Some(ref repo) = stored.payload.metadata.repository {
                                    if &repo.full_name != repo_filter {
                                        matches = false;
                                    }
                                } else {
                                    matches = false;
                                }
                            }

                            if let Some(ref event_type_filter) = filter.event_type {
                                if &stored.payload.metadata.event_type != event_type_filter {
                                    matches = false;
                                }
                            }

                            if let Some(ref date_range) = filter.date_range {
                                if stored.payload.metadata.received_at < date_range.start
                                    || stored.payload.metadata.received_at >= date_range.end
                                {
                                    matches = false;
                                }
                            }

                            if matches {
                                results.push(stored.metadata);
                            }
                        }
                    }
                }
            }
        }

        // Apply limit and offset
        if let Some(offset) = filter.offset {
            if offset < results.len() {
                results = results.into_iter().skip(offset).collect();
            } else {
                results.clear();
            }
        }

        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn delete_payload(&self, event_id: &EventId) -> Result<(), BlobStorageError> {
        let blob_path = self.get_blob_path(event_id);

        if !blob_path.exists() {
            return Err(BlobStorageError::BlobNotFound {
                event_id: *event_id,
            });
        }

        fs::remove_file(&blob_path)
            .await
            .map_err(|e| BlobStorageError::InternalError {
                message: format!("Failed to delete blob: {}", e),
            })?;

        Ok(())
    }

    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError> {
        // Check if base path is accessible
        let accessible = self.base_path.exists() && self.base_path.is_dir();

        if accessible {
            Ok(StorageHealthStatus {
                healthy: true,
                connected: true,
                last_success: Some(Timestamp::now()),
                error_message: None,
                metrics: StorageMetrics {
                    avg_write_latency_ms: 0.0,
                    avg_read_latency_ms: 0.0,
                    success_rate: 1.0,
                },
            })
        } else {
            Ok(StorageHealthStatus {
                healthy: false,
                connected: false,
                last_success: None,
                error_message: Some("Base path not accessible".to_string()),
                metrics: StorageMetrics {
                    avg_write_latency_ms: 0.0,
                    avg_read_latency_ms: 0.0,
                    success_rate: 0.0,
                },
            })
        }
    }
}

#[cfg(test)]
#[path = "filesystem_storage_tests.rs"]
mod tests;
