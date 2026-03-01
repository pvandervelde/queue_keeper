//! # Webhook Storage Adapter
//!
//! Adapts BlobStorage trait to PayloadStorer interface for webhook processing integration.

use super::{PayloadStorer, StorageError, StorageReference, ValidationStatus, WebhookRequest};
use crate::blob_storage::{BlobStorage, BlobStorageError, PayloadMetadata, WebhookPayload};
use crate::Timestamp;
use async_trait::async_trait;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

/// Adapter that implements PayloadStorer using BlobStorage
///
/// Bridges the webhook processing pipeline with the blob storage layer,
/// converting between webhook-specific types and blob storage types.
///
/// # Examples
///
/// ```rust,no_run
/// use queue_keeper_core::webhook::BlobStorageAdapter;
/// use queue_keeper_core::adapters::FilesystemBlobStorage;
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let blob_storage = FilesystemBlobStorage::new(PathBuf::from("./data")).await?;
/// let adapter = BlobStorageAdapter::new(Arc::new(blob_storage));
/// # Ok(())
/// # }
/// ```
pub struct BlobStorageAdapter {
    blob_storage: Arc<dyn BlobStorage>,
}

impl BlobStorageAdapter {
    /// Create new blob storage adapter
    ///
    /// # Arguments
    ///
    /// * `blob_storage` - Blob storage implementation to use
    pub fn new(blob_storage: Arc<dyn BlobStorage>) -> Self {
        Self { blob_storage }
    }

    /// Convert webhook request and validation status to WebhookPayload
    fn create_webhook_payload(
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> WebhookPayload {
        // Convert headers to HashMap
        let mut headers = HashMap::new();
        headers.insert(
            "x-github-event".to_string(),
            request.event_type().to_string(),
        );
        headers.insert(
            "x-github-delivery".to_string(),
            request.delivery_id().to_string(),
        );
        if let Some(signature) = request.signature() {
            headers.insert("x-hub-signature-256".to_string(), signature.to_string());
        }
        if let Some(user_agent) = &request.headers.user_agent {
            headers.insert("user-agent".to_string(), user_agent.clone());
        }
        headers.insert(
            "content-type".to_string(),
            request.headers.content_type.clone(),
        );

        // Parse JSON to extract repository for metadata
        // Note: If parsing fails, we'll use placeholder values
        let (repository, event_id) =
            match serde_json::from_slice::<serde_json::Value>(&request.body) {
                Ok(payload) => {
                    let repo = Self::extract_repository_from_payload(&payload);
                    let event_id = crate::EventId::new();
                    (repo, event_id)
                }
                Err(_) => {
                    // Parsing failed - use placeholder
                    let event_id = crate::EventId::new();
                    let repo = crate::Repository::new(
                        crate::RepositoryId::new(0),
                        "unknown".to_string(),
                        "unknown/unknown".to_string(),
                        crate::User {
                            id: crate::UserId::new(0),
                            login: "unknown".to_string(),
                            user_type: crate::UserType::User,
                        },
                        false,
                    );
                    (repo, event_id)
                }
            };

        WebhookPayload {
            body: request.body.clone(),
            headers,
            metadata: PayloadMetadata {
                event_id,
                event_type: request.event_type().to_string(),
                repository: Some(repository),
                signature_valid: matches!(validation_status, ValidationStatus::Valid),
                received_at: request.received_at,
                delivery_id: Some(request.delivery_id().to_string()),
            },
        }
    }

    /// Extract repository from JSON payload
    fn extract_repository_from_payload(payload: &serde_json::Value) -> crate::Repository {
        // Try to extract repository information
        if let Some(repo_data) = payload.get("repository") {
            if let (Some(id), Some(name), Some(full_name)) = (
                repo_data.get("id").and_then(|i| i.as_u64()),
                repo_data.get("name").and_then(|n| n.as_str()),
                repo_data.get("full_name").and_then(|f| f.as_str()),
            ) {
                let private = repo_data
                    .get("private")
                    .and_then(|p| p.as_bool())
                    .unwrap_or(false);

                // Extract owner
                let owner = if let Some(owner_data) = repo_data.get("owner") {
                    if let (Some(owner_id), Some(owner_login)) = (
                        owner_data.get("id").and_then(|i| i.as_u64()),
                        owner_data.get("login").and_then(|l| l.as_str()),
                    ) {
                        let owner_type = match owner_data.get("type").and_then(|t| t.as_str()) {
                            Some("User") => crate::UserType::User,
                            Some("Bot") => crate::UserType::Bot,
                            Some("Organization") => crate::UserType::Organization,
                            _ => crate::UserType::User,
                        };

                        crate::User {
                            id: crate::UserId::new(owner_id),
                            login: owner_login.to_string(),
                            user_type: owner_type,
                        }
                    } else {
                        // Owner fields missing - use placeholder
                        crate::User {
                            id: crate::UserId::new(0),
                            login: "unknown".to_string(),
                            user_type: crate::UserType::User,
                        }
                    }
                } else {
                    // Owner missing - use placeholder
                    crate::User {
                        id: crate::UserId::new(0),
                        login: "unknown".to_string(),
                        user_type: crate::UserType::User,
                    }
                };

                return crate::Repository::new(
                    crate::RepositoryId::new(id),
                    name.to_string(),
                    full_name.to_string(),
                    owner,
                    private,
                );
            }
        }

        // Repository extraction failed - use placeholder
        crate::Repository::new(
            crate::RepositoryId::new(0),
            "unknown".to_string(),
            "unknown/unknown".to_string(),
            crate::User {
                id: crate::UserId::new(0),
                login: "unknown".to_string(),
                user_type: crate::UserType::User,
            },
            false,
        )
    }

    /// Convert BlobStorageError to StorageError
    fn map_blob_storage_error(error: BlobStorageError) -> StorageError {
        match error {
            BlobStorageError::ConnectionFailed { message } => StorageError::Unavailable { message },
            BlobStorageError::AuthenticationFailed { message } => {
                StorageError::PermissionDenied { message }
            }
            BlobStorageError::PermissionDenied { operation } => {
                StorageError::PermissionDenied { message: operation }
            }
            BlobStorageError::QuotaExceeded => StorageError::OperationFailed {
                message: "Storage quota exceeded".to_string(),
            },
            BlobStorageError::Timeout { timeout_ms } => StorageError::OperationFailed {
                message: format!("Storage operation timed out after {}ms", timeout_ms),
            },
            BlobStorageError::InternalError { message } => {
                StorageError::OperationFailed { message }
            }
            _ => StorageError::OperationFailed {
                message: format!("Blob storage error: {}", error),
            },
        }
    }
}

#[async_trait]
impl PayloadStorer for BlobStorageAdapter {
    async fn store_payload(
        &self,
        request: &WebhookRequest,
        validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        // Convert to WebhookPayload
        let payload = Self::create_webhook_payload(request, validation_status);
        let event_id = payload.metadata.event_id;

        // Store via blob storage
        let metadata = self
            .blob_storage
            .store_payload(&event_id, &payload)
            .await
            .map_err(Self::map_blob_storage_error)?;

        // Convert to StorageReference
        Ok(StorageReference {
            blob_path: metadata.blob_path,
            stored_at: metadata.created_at,
            size_bytes: metadata.size_bytes,
        })
    }

    async fn retrieve_payload(
        &self,
        storage_ref: &StorageReference,
    ) -> Result<WebhookRequest, StorageError> {
        // Extract event ID from blob path
        // Path format: webhook-payloads/year=X/month=X/day=X/hour=X/{event_id}.json
        let event_id_str = storage_ref
            .blob_path
            .rsplit('/')
            .next()
            .and_then(|s| s.strip_suffix(".json"))
            .ok_or_else(|| StorageError::OperationFailed {
                message: format!("Invalid blob path format: {}", storage_ref.blob_path),
            })?;

        let event_id =
            crate::EventId::from_str(event_id_str).map_err(|_| StorageError::OperationFailed {
                message: format!("Invalid event ID in path: {}", event_id_str),
            })?;

        // Retrieve from blob storage
        let stored = self
            .blob_storage
            .get_payload(&event_id)
            .await
            .map_err(Self::map_blob_storage_error)?
            .ok_or_else(|| StorageError::OperationFailed {
                message: format!("Payload not found for event: {}", event_id),
            })?;

        // Convert headers back to WebhookHeaders
        let headers = super::WebhookHeaders {
            event_type: stored
                .payload
                .headers
                .get("x-github-event")
                .cloned()
                .unwrap_or_default(),
            delivery_id: stored
                .payload
                .headers
                .get("x-github-delivery")
                .cloned()
                .unwrap_or_default(),
            signature: stored.payload.headers.get("x-hub-signature-256").cloned(),
            user_agent: stored.payload.headers.get("user-agent").cloned(),
            content_type: stored
                .payload
                .headers
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| "application/json".to_string()),
        };

        // Reconstruct WebhookRequest (no raw headers available from blob storage)
        Ok(WebhookRequest {
            headers,
            body: stored.payload.body,
            received_at: stored.payload.metadata.received_at,
            raw_headers: std::collections::HashMap::new(),
        })
    }

    async fn list_payloads(
        &self,
        filters: super::PayloadFilters,
    ) -> Result<Vec<StorageReference>, StorageError> {
        // Convert to blob storage filter
        let blob_filter = crate::blob_storage::PayloadFilter {
            date_range: if filters.start_date.is_some() || filters.end_date.is_some() {
                Some(crate::blob_storage::DateRange {
                    start: filters.start_date.unwrap_or_else(|| {
                        // Default to 30 days ago
                        Timestamp::now()
                    }),
                    end: filters.end_date.unwrap_or_else(Timestamp::now),
                })
            } else {
                None
            },
            repository: None, // Filter by repository_id not directly supported
            event_type: filters.event_type,
            limit: filters.limit,
            offset: None,
        };

        // List from blob storage
        let metadata_list = self
            .blob_storage
            .list_payloads(&blob_filter)
            .await
            .map_err(Self::map_blob_storage_error)?;

        // Convert to storage references
        Ok(metadata_list
            .into_iter()
            .map(|metadata| StorageReference {
                blob_path: metadata.blob_path,
                stored_at: metadata.created_at,
                size_bytes: metadata.size_bytes,
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "storage_adapter_tests.rs"]
mod tests;
