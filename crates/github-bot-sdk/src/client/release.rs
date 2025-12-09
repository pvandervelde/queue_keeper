// GENERATED FROM: github-bot-sdk-specs/interfaces/additional-operations.md (Release section)
// Release and release asset operations for GitHub API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::issue::IssueUser;
use crate::client::InstallationClient;
use crate::error::ApiError;

/// GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    /// Unique release identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Release tag name
    pub tag_name: String,

    /// Target commitish (branch or commit SHA)
    pub target_commitish: String,

    /// Release name
    pub name: Option<String>,

    /// Release body (Markdown)
    pub body: Option<String>,

    /// Whether this is a draft release
    pub draft: bool,

    /// Whether this is a prerelease
    pub prerelease: bool,

    /// User who created the release
    pub author: IssueUser,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Publication timestamp
    pub published_at: Option<DateTime<Utc>>,

    /// Release URL
    pub url: String,

    /// Release HTML URL
    pub html_url: String,

    /// Release assets
    pub assets: Vec<ReleaseAsset>,
}

/// Asset attached to a release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    /// Unique asset identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Asset filename
    pub name: String,

    /// Asset label
    pub label: Option<String>,

    /// Asset content type
    pub content_type: String,

    /// Asset state
    pub state: String, // "uploaded", "open"

    /// Asset size in bytes
    pub size: u64,

    /// Download count
    pub download_count: u64,

    /// User who uploaded the asset
    pub uploader: IssueUser,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Asset download URL
    pub browser_download_url: String,
}

/// Request to create a release.
#[derive(Debug, Clone, Serialize)]
pub struct CreateReleaseRequest {
    /// Tag name (required)
    pub tag_name: String,

    /// Target commitish (branch or commit SHA)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_commitish: Option<String>,

    /// Release name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Release body (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Whether to create as draft
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,

    /// Whether to mark as prerelease
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
}

/// Request to update a release.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateReleaseRequest {
    /// Tag name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_name: Option<String>,

    /// Target commitish
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_commitish: Option<String>,

    /// Release name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Release body (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Whether this is a draft
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,

    /// Whether this is a prerelease
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
}

impl InstallationClient {
    // ========================================================================
    // Release Operations
    // ========================================================================

    /// List releases in a repository.
    ///
    /// See github-bot-sdk-specs/interfaces/additional-operations.md
    pub async fn list_releases(&self, _owner: &str, _repo: &str) -> Result<Vec<Release>, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/additional-operations.md")
    }

    /// Get the latest published release.
    ///
    /// See github-bot-sdk-specs/interfaces/additional-operations.md
    pub async fn get_latest_release(&self, _owner: &str, _repo: &str) -> Result<Release, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/additional-operations.md")
    }

    /// Get a release by tag name.
    ///
    /// See github-bot-sdk-specs/interfaces/additional-operations.md
    pub async fn get_release_by_tag(
        &self,
        _owner: &str,
        _repo: &str,
        _tag: &str,
    ) -> Result<Release, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/additional-operations.md")
    }

    /// Get a release by ID.
    ///
    /// See github-bot-sdk-specs/interfaces/additional-operations.md
    pub async fn get_release(
        &self,
        _owner: &str,
        _repo: &str,
        _release_id: u64,
    ) -> Result<Release, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/additional-operations.md")
    }

    /// Create a new release.
    ///
    /// See github-bot-sdk-specs/interfaces/additional-operations.md
    pub async fn create_release(
        &self,
        _owner: &str,
        _repo: &str,
        _request: CreateReleaseRequest,
    ) -> Result<Release, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/additional-operations.md")
    }

    /// Update an existing release.
    ///
    /// See github-bot-sdk-specs/interfaces/additional-operations.md
    pub async fn update_release(
        &self,
        _owner: &str,
        _repo: &str,
        _release_id: u64,
        _request: UpdateReleaseRequest,
    ) -> Result<Release, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/additional-operations.md")
    }

    /// Delete a release.
    ///
    /// See github-bot-sdk-specs/interfaces/additional-operations.md
    pub async fn delete_release(
        &self,
        _owner: &str,
        _repo: &str,
        _release_id: u64,
    ) -> Result<(), ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/additional-operations.md")
    }
}

#[cfg(test)]
#[path = "release_tests.rs"]
mod tests;
