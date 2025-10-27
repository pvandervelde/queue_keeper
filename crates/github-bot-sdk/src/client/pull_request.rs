// GENERATED FROM: github-bot-sdk-specs/interfaces/pull-request-operations.md
// Pull request and review operations for GitHub API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::issue::{IssueUser, Label, Milestone};
use crate::client::repository::Commit;
use crate::client::InstallationClient;
use crate::error::ApiError;

/// GitHub pull request.
///
/// Represents a pull request with all its metadata.
///
/// See github-bot-sdk-specs/interfaces/pull-request-operations.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    /// Unique pull request identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Pull request number (repository-specific)
    pub number: u64,

    /// Pull request title
    pub title: String,

    /// Pull request body content (Markdown)
    pub body: Option<String>,

    /// Pull request state
    pub state: String, // "open", "closed"

    /// User who created the pull request
    pub user: IssueUser,

    /// Head branch information
    pub head: PullRequestBranch,

    /// Base branch information
    pub base: PullRequestBranch,

    /// Whether the pull request is a draft
    pub draft: bool,

    /// Whether the pull request is merged
    pub merged: bool,

    /// Whether the pull request is mergeable
    pub mergeable: Option<bool>,

    /// Merge commit SHA (if merged)
    pub merge_commit_sha: Option<String>,

    /// Assigned users
    pub assignees: Vec<IssueUser>,

    /// Requested reviewers
    pub requested_reviewers: Vec<IssueUser>,

    /// Applied labels
    pub labels: Vec<Label>,

    /// Milestone
    pub milestone: Option<Milestone>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Close timestamp
    pub closed_at: Option<DateTime<Utc>>,

    /// Merge timestamp
    pub merged_at: Option<DateTime<Utc>>,

    /// Pull request URL
    pub html_url: String,
}

/// Branch information in a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestBranch {
    /// Branch name
    #[serde(rename = "ref")]
    pub branch_ref: String,

    /// Commit SHA
    pub sha: String,

    /// Repository information
    pub repo: PullRequestRepo,
}

/// Repository information in a pull request branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestRepo {
    /// Repository ID
    pub id: u64,

    /// Repository name
    pub name: String,

    /// Full repository name (owner/repo)
    pub full_name: String,
}

/// Pull request review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    /// Unique review identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// User who submitted the review
    pub user: IssueUser,

    /// Review body content (Markdown)
    pub body: Option<String>,

    /// Review state
    pub state: String, // "APPROVED", "CHANGES_REQUESTED", "COMMENTED", "DISMISSED", "PENDING"

    /// Commit SHA that was reviewed
    pub commit_id: String,

    /// Creation timestamp
    pub submitted_at: Option<DateTime<Utc>>,

    /// Review URL
    pub html_url: String,
}

/// Comment on a pull request (review comment on code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestComment {
    /// Unique comment identifier
    pub id: u64,

    /// Node ID for GraphQL API
    pub node_id: String,

    /// Comment body content (Markdown)
    pub body: String,

    /// User who created the comment
    pub user: IssueUser,

    /// File path
    pub path: String,

    /// Line number (if single-line comment)
    pub line: Option<u64>,

    /// Commit SHA
    pub commit_id: String,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Comment URL
    pub html_url: String,
}

/// Request to create a new pull request.
#[derive(Debug, Clone, Serialize)]
pub struct CreatePullRequestRequest {
    /// Pull request title (required)
    pub title: String,

    /// Head branch (required) - format: "username:branch" for forks
    pub head: String,

    /// Base branch (required)
    pub base: String,

    /// Pull request body content (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Whether to create as draft
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,

    /// Milestone number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<u64>,
}

/// Request to update an existing pull request.
#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdatePullRequestRequest {
    /// Pull request title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Pull request body content (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Pull request state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>, // "open" or "closed"

    /// Base branch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,

    /// Milestone number (None to clear milestone)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<u64>,
}

/// Request to merge a pull request.
#[derive(Debug, Clone, Serialize, Default)]
pub struct MergePullRequestRequest {
    /// Merge commit message title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_title: Option<String>,

    /// Merge commit message body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,

    /// SHA that pull request head must match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,

    /// Merge method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_method: Option<String>, // "merge", "squash", "rebase"
}

/// Result of merging a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    /// Whether the merge was successful
    pub merged: bool,

    /// Merge commit SHA
    pub sha: String,

    /// Message describing the result
    pub message: String,
}

/// Request to create a review.
#[derive(Debug, Clone, Serialize)]
pub struct CreateReviewRequest {
    /// Commit SHA to review (optional, defaults to PR head)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_id: Option<String>,

    /// Review body content (Markdown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Review event
    pub event: String, // "APPROVE", "REQUEST_CHANGES", "COMMENT"
}

/// Request to update a review.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateReviewRequest {
    /// Review body content (Markdown, required)
    pub body: String,
}

/// Request to dismiss a review.
#[derive(Debug, Clone, Serialize)]
pub struct DismissReviewRequest {
    /// Dismissal message (required)
    pub message: String,
}

/// Request to create a pull request comment.
#[derive(Debug, Clone, Serialize)]
pub struct CreatePullRequestCommentRequest {
    /// Comment body content (Markdown, required)
    pub body: String,
}

/// Request to set milestone on a pull request.
#[derive(Debug, Clone, Serialize)]
pub struct SetPullRequestMilestoneRequest {
    /// Milestone number (None to clear milestone)
    pub milestone: Option<u64>,
}

impl InstallationClient {
    // ========================================================================
    // Pull Request Operations
    // ========================================================================

    /// List pull requests in a repository.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: Option<&str>,
    ) -> Result<Vec<PullRequest>, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Get a specific pull request by number.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<PullRequest, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Create a new pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        request: CreatePullRequestRequest,
    ) -> Result<PullRequest, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Update an existing pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn update_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        request: UpdatePullRequestRequest,
    ) -> Result<PullRequest, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Merge a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        request: MergePullRequestRequest,
    ) -> Result<MergeResult, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Set the milestone on a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn set_pull_request_milestone(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        milestone_number: Option<u64>,
    ) -> Result<PullRequest, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    // ========================================================================
    // Pull Request Review Operations
    // ========================================================================

    /// List reviews on a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn list_reviews(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<Vec<Review>, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Get a specific review by ID.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn get_review(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        review_id: u64,
    ) -> Result<Review, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Create a review on a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn create_review(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        request: CreateReviewRequest,
    ) -> Result<Review, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Update a pending review.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn update_review(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        review_id: u64,
        request: UpdateReviewRequest,
    ) -> Result<Review, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Dismiss a review.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn dismiss_review(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        review_id: u64,
        request: DismissReviewRequest,
    ) -> Result<Review, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    // ========================================================================
    // Pull Request Comment Operations
    // ========================================================================

    /// List comments on a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn list_pull_request_comments(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<Vec<PullRequestComment>, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Create a comment on a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn create_pull_request_comment(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        request: CreatePullRequestCommentRequest,
    ) -> Result<PullRequestComment, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    // ========================================================================
    // Pull Request Label Operations
    // ========================================================================

    /// Add labels to a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn add_labels_to_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        labels: Vec<String>,
    ) -> Result<Vec<Label>, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }

    /// Remove a label from a pull request.
    ///
    /// See github-bot-sdk-specs/interfaces/pull-request-operations.md
    pub async fn remove_label_from_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        name: &str,
    ) -> Result<(), ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/pull-request-operations.md")
    }
}

#[cfg(test)]
#[path = "pull_request_tests.rs"]
mod tests;
