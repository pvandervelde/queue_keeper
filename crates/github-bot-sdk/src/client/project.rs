// GENERATED FROM: github-bot-sdk-specs/interfaces/project-operations.md
// GitHub Projects v2 operations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::InstallationClient;
use crate::error::ApiError;

/// GitHub Projects v2 project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectV2 {
    /// Unique project identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Project number (unique within owner)
    pub number: u64,

    /// Project title
    pub title: String,

    /// Project description
    pub description: Option<String>,

    /// Project owner (organization or user)
    pub owner: ProjectOwner,

    /// Project visibility
    pub public: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Project URL
    pub url: String,
}

/// Project owner (organization or user).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOwner {
    /// Owner login name
    pub login: String,

    /// Owner type
    #[serde(rename = "type")]
    pub owner_type: String, // "Organization" or "User"

    /// Owner ID
    pub id: u64,

    /// Owner node ID
    pub node_id: String,
}

/// Item in a GitHub Projects v2 project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectV2Item {
    /// Unique item identifier (project-specific)
    pub id: String,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Content type
    pub content_type: String, // "Issue" or "PullRequest"

    /// Content node ID (issue or PR node ID)
    pub content_node_id: String,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Request to add an item to a project.
#[derive(Debug, Clone, Serialize)]
pub struct AddProjectV2ItemRequest {
    /// Node ID of the content to add (issue or pull request)
    pub content_node_id: String,
}

impl InstallationClient {
    // ========================================================================
    // Project Operations
    // ========================================================================

    /// List all Projects v2 for an organization.
    ///
    /// See github-bot-sdk-specs/interfaces/project-operations.md
    pub async fn list_organization_projects(&self, _org: &str) -> Result<Vec<ProjectV2>, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/project-operations.md")
    }

    /// List all Projects v2 for a user.
    ///
    /// See github-bot-sdk-specs/interfaces/project-operations.md
    pub async fn list_user_projects(&self, _username: &str) -> Result<Vec<ProjectV2>, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/project-operations.md")
    }

    /// Get details about a specific project.
    ///
    /// See github-bot-sdk-specs/interfaces/project-operations.md
    pub async fn get_project(
        &self,
        _owner: &str,
        _project_number: u64,
    ) -> Result<ProjectV2, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/project-operations.md")
    }

    /// Add an issue or pull request to a project.
    ///
    /// See github-bot-sdk-specs/interfaces/project-operations.md
    pub async fn add_item_to_project(
        &self,
        _owner: &str,
        _project_number: u64,
        _content_node_id: &str,
    ) -> Result<ProjectV2Item, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/project-operations.md")
    }

    /// Remove an item from a project.
    ///
    /// See github-bot-sdk-specs/interfaces/project-operations.md
    pub async fn remove_item_from_project(
        &self,
        _owner: &str,
        _project_number: u64,
        _item_id: &str,
    ) -> Result<(), ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/project-operations.md")
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
