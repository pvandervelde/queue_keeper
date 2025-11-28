//! # Filesystem Blob Storage Adapter
//!
//! Local filesystem implementation of BlobStorage trait for development and testing.

use crate::blob_storage::*;
use crate::EventId;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

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
        _event_id: &EventId,
        _payload: &WebhookPayload,
    ) -> Result<BlobMetadata, BlobStorageError> {
        // TODO: Implement in Phase 2
        unimplemented!("store_payload not yet implemented")
    }

    async fn get_payload(
        &self,
        _event_id: &EventId,
    ) -> Result<Option<StoredWebhook>, BlobStorageError> {
        // TODO: Implement in Phase 2
        unimplemented!("get_payload not yet implemented")
    }

    async fn list_payloads(
        &self,
        _filter: &PayloadFilter,
    ) -> Result<Vec<BlobMetadata>, BlobStorageError> {
        // TODO: Implement in Phase 2
        unimplemented!("list_payloads not yet implemented")
    }

    async fn delete_payload(&self, _event_id: &EventId) -> Result<(), BlobStorageError> {
        // TODO: Implement in Phase 2
        unimplemented!("delete_payload not yet implemented")
    }

    async fn health_check(&self) -> Result<StorageHealthStatus, BlobStorageError> {
        // TODO: Implement in Phase 2
        unimplemented!("health_check not yet implemented")
    }
}

#[cfg(test)]
#[path = "filesystem_storage_tests.rs"]
mod tests;
