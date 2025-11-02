// GENERATED FROM: github-bot-sdk-specs/interfaces/issue-operations.md
// Issue, label, and comment operations for GitHub API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::InstallationClient;
use crate::error::ApiError;

/// GitHub issue.
///
/// Represents a GitHub issue with all its metadata.
///
/// See github-bot-sdk-specs/interfaces/issue-operations.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    /// Unique issue identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Issue number (repository-specific)
    pub number: u64,

    /// Issue title
    pub title: String,

    /// Issue body content (Markdown)
    pub body: Option<String>,

    /// Issue state
    pub state: String, // "open" or "closed"

    /// User who created the issue
    pub user: IssueUser,

    /// Assigned users
    pub assignees: Vec<IssueUser>,

    /// Applied labels
    pub labels: Vec<Label>,

    /// Milestone
    pub milestone: Option<Milestone>,

    /// Number of comments
    pub comments: u64,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Close timestamp
    pub closed_at: Option<DateTime<Utc>>,

    /// Issue URL
    pub html_url: String,
}

/// User associated with an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueUser {
    /// User login name
    pub login: String,

    /// User ID
    pub id: u64,

    /// User node ID
    pub node_id: String,

    /// User type
    #[serde(rename = "type")]
    pub user_type: String,
}

/// Milestone associated with an issue or pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// Unique milestone identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Milestone number (repository-specific)
    pub number: u64,

    /// Milestone title
    pub title: String,

    /// Milestone description
    pub description: Option<String>,

    /// Milestone state
    pub state: String, // "open" or "closed"

    /// Number of open issues
    pub open_issues: u64,

    /// Number of closed issues
    pub closed_issues: u64,

    /// Due date
    pub due_on: Option<DateTime<Utc>>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Close timestamp
    pub closed_at: Option<DateTime<Utc>>,
}

/// GitHub label.
///
/// Labels are used to categorize issues and pull requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    /// Unique label identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Label name
    pub name: String,

    /// Label description
    pub description: Option<String>,

    /// Label color (6-digit hex code without #)
    pub color: String,

    /// Whether this is a default label
    pub default: bool,
}

/// Comment on an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    /// Unique comment identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Comment body content (Markdown)
    pub body: String,

    /// User who created the comment
    pub user: IssueUser,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Comment URL
    pub html_url: String,
}

/// Request to create a new issue.
#[derive(Debug, Clone, Serialize)]
pub struct CreateIssueRequest {
    /// Issue title (required)
    pub title: String,

    /// Issue body content (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Usernames to assign
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,

    /// Milestone number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<u64>,

    /// Label names to apply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

/// Request to update an existing issue.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateIssueRequest {
    /// Issue title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Issue body content (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Issue state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>, // "open" or "closed"

    /// Usernames to assign (replaces existing assignees)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,

    /// Milestone number (None to clear milestone)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<u64>,

    /// Label names (replaces existing labels)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

/// Request to create a label.
#[derive(Debug, Clone, Serialize)]
pub struct CreateLabelRequest {
    /// Label name (required)
    pub name: String,

    /// Label color (6-digit hex code without #)
    pub color: String,

    /// Label description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Request to update a label.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateLabelRequest {
    /// New label name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_name: Option<String>,

    /// Label color (6-digit hex code without #)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// Label description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Request to create a comment.
#[derive(Debug, Clone, Serialize)]
pub struct CreateCommentRequest {
    /// Comment body content (Markdown, required)
    pub body: String,
}

/// Request to update a comment.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateCommentRequest {
    /// Comment body content (Markdown, required)
    pub body: String,
}

/// Request to set milestone on an issue.
#[derive(Debug, Clone, Serialize)]
pub struct SetIssueMilestoneRequest {
    /// Milestone number (None to clear milestone)
    pub milestone: Option<u64>,
}

impl InstallationClient {
    // ========================================================================
    // Issue Operations
    // ========================================================================

    /// List issues in a repository.
    pub async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
    ) -> Result<Vec<Issue>, ApiError> {
        let mut path = format!("/repos/{}/{}/issues", owner, repo);
        if let Some(state_value) = state {
            path = format!("{}?state={}", path, state_value);
        }

        let response = self.get(&path).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Get a specific issue by number.
    pub async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<Issue, ApiError> {
        let path = format!("/repos/{}/{}/issues/{}", owner, repo, issue_number);
        let response = self.get(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Create a new issue.
    pub async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        request: CreateIssueRequest,
    ) -> Result<Issue, ApiError> {
        let path = format!("/repos/{}/{}/issues", owner, repo);
        let response = self.post(&path, &request).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                422 => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Validation failed".to_string());
                    ApiError::InvalidRequest { message }
                }
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Update an existing issue.
    pub async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        request: UpdateIssueRequest,
    ) -> Result<Issue, ApiError> {
        let path = format!("/repos/{}/{}/issues/{}", owner, repo, issue_number);
        let response = self.patch(&path, &request).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                422 => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Validation failed".to_string());
                    ApiError::InvalidRequest { message }
                }
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Set the milestone on an issue.
    pub async fn set_issue_milestone(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        milestone_number: Option<u64>,
    ) -> Result<Issue, ApiError> {
        let request = UpdateIssueRequest {
            milestone: Some(milestone_number.unwrap_or(0)),
            ..Default::default()
        };
        self.update_issue(owner, repo, issue_number, request).await
    }

    // ========================================================================
    // Label Operations
    // ========================================================================

    /// List all labels in a repository.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn list_labels(&self, owner: &str, repo: &str) -> Result<Vec<Label>, ApiError> {
        let path = format!("/repos/{}/{}/labels", owner, repo);
        let response = self.get(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Get a specific label by name.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn get_label(&self, owner: &str, repo: &str, name: &str) -> Result<Label, ApiError> {
        let path = format!("/repos/{}/{}/labels/{}", owner, repo, name);
        let response = self.get(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Create a new label.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn create_label(
        &self,
        owner: &str,
        repo: &str,
        request: CreateLabelRequest,
    ) -> Result<Label, ApiError> {
        let path = format!("/repos/{}/{}/labels", owner, repo);
        let response = self.post(&path, &request).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                422 => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Validation failed".to_string());
                    ApiError::InvalidRequest { message }
                }
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Update an existing label.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn update_label(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        request: UpdateLabelRequest,
    ) -> Result<Label, ApiError> {
        let path = format!("/repos/{}/{}/labels/{}", owner, repo, name);
        let response = self.patch(&path, &request).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                422 => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Validation failed".to_string());
                    ApiError::InvalidRequest { message }
                }
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Delete a label.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn delete_label(&self, owner: &str, repo: &str, name: &str) -> Result<(), ApiError> {
        let path = format!("/repos/{}/{}/labels/{}", owner, repo, name);
        let response = self.delete(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        Ok(())
    }

    /// Add labels to an issue.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn add_labels_to_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        labels: Vec<String>,
    ) -> Result<Vec<Label>, ApiError> {
        let path = format!("/repos/{}/{}/issues/{}/labels", owner, repo, issue_number);
        let response = self.post(&path, &labels).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                422 => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Validation failed".to_string());
                    ApiError::InvalidRequest { message }
                }
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Remove a label from an issue.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn remove_label_from_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        name: &str,
    ) -> Result<Vec<Label>, ApiError> {
        let path = format!(
            "/repos/{}/{}/issues/{}/labels/{}",
            owner, repo, issue_number, name
        );
        let response = self.delete(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    // ========================================================================
    // Comment Operations
    // ========================================================================

    /// List comments on an issue.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn list_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<Vec<Comment>, ApiError> {
        let path = format!("/repos/{}/{}/issues/{}/comments", owner, repo, issue_number);
        let response = self.get(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Get a specific comment by ID.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn get_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> Result<Comment, ApiError> {
        let path = format!("/repos/{}/{}/issues/comments/{}", owner, repo, comment_id);
        let response = self.get(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Create a comment on an issue.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn create_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        request: CreateCommentRequest,
    ) -> Result<Comment, ApiError> {
        let path = format!("/repos/{}/{}/issues/{}/comments", owner, repo, issue_number);
        let response = self.post(&path, &request).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                422 => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Validation failed".to_string());
                    ApiError::InvalidRequest { message }
                }
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Update an existing comment.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn update_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        request: UpdateCommentRequest,
    ) -> Result<Comment, ApiError> {
        let path = format!("/repos/{}/{}/issues/comments/{}", owner, repo, comment_id);
        let response = self.patch(&path, &request).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                422 => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Validation failed".to_string());
                    ApiError::InvalidRequest { message }
                }
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// Delete a comment.
    ///
    /// See github-bot-sdk-specs/interfaces/issue-operations.md
    pub async fn delete_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> Result<(), ApiError> {
        let path = format!("/repos/{}/{}/issues/comments/{}", owner, repo, comment_id);
        let response = self.delete(&path).await?;

        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                404 => ApiError::NotFound,
                403 => ApiError::AuthorizationFailed,
                401 => ApiError::AuthenticationFailed,
                _ => {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    ApiError::HttpError {
                        status: status.as_u16(),
                        message,
                    }
                }
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "issue_tests.rs"]
mod tests;
