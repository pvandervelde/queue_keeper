# Pull Request Operations Interface Specification

**Module**: `github-bot-sdk::client::pull_request`
**File**: `crates/github-bot-sdk/src/client/pull_request.rs`
**Dependencies**: `InstallationClient`, `ApiError`, issue types (User, Label), shared types

## Overview

Pull request operations provide access to PR management, reviews, and merge operations. These are installation-scoped operations requiring appropriate repository permissions.

## Architectural Location

**Layer**: Infrastructure adapter (GitHub API operations)
**Purpose**: Pull request and review management
**Required Permissions**: `pull_requests:read` (minimum), `pull_requests:write` (for mutations)

## Core Types

### PullRequest

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: PullRequestState,
    pub user: User,
    pub head: PullRequestBranch,
    pub base: PullRequestBranch,
    pub draft: bool,
    pub merged: bool,
    pub mergeable: Option<bool>,
    pub labels: Vec<Label>,
    pub html_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub merged_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub closed_at: Option<OffsetDateTime>,
}
```

### PullRequestState

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PullRequestState {
    Open,
    Closed,
}
```

### PullRequestBranch

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestBranch {
    pub label: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
    pub repo: Option<PullRequestRepo>,
}
```

**Note**: Uses the shared `Commit` type from repository operations for commit references.

### PullRequestRepo

Repository information for pull request branches.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
}
```

### Review

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: u64,
    pub user: User,
    pub body: Option<String>,
    pub state: ReviewState,
    pub html_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub submitted_at: OffsetDateTime,
}
```

### ReviewState

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
}
```

## Pull Request Operations

### Get Pull Request

```rust
impl InstallationClient {
    /// Get a specific pull request by number.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    ///
    /// # Returns
    ///
    /// Returns `PullRequest` with full metadata.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - PR doesn't exist
    /// * `ApiError::PermissionDenied` - Missing `pull_requests:read`
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<PullRequest, ApiError>;
}
```

### List Pull Requests

```rust
impl InstallationClient {
    /// List pull requests in a repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `params` - Optional query parameters (state, head, base, etc.)
    ///
    /// # Returns
    ///
    /// Returns vector of pull requests matching criteria.
    pub async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        params: Option<&ListPullRequestsParams>,
    ) -> Result<Vec<PullRequest>, ApiError>;
}
```

### Create Pull Request

```rust
impl InstallationClient {
    /// Create a new pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `request` - PR creation data
    ///
    /// # Returns
    ///
    /// Returns the created `PullRequest`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `pull_requests:write`
    /// * `ApiError::HttpError` - Invalid branch or no commits (422)
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        request: &CreatePullRequestRequest,
    ) -> Result<PullRequest, ApiError>;
}
```

### Update Pull Request

```rust
impl InstallationClient {
    /// Update an existing pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    /// * `request` - Update data
    ///
    /// # Returns
    ///
    /// Returns the updated `PullRequest`.
    pub async fn update_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        request: &UpdatePullRequestRequest,
    ) -> Result<PullRequest, ApiError>;
}
```

### Merge Pull Request

```rust
impl InstallationClient {
    /// Merge a pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    /// * `request` - Merge options
    ///
    /// # Returns
    ///
    /// Returns merge result with SHA and status.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing merge permission
    /// * `ApiError::HttpError` - Not mergeable (405), conflicts exist (409)
    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        request: Option<&MergePullRequestRequest>,
    ) -> Result<MergeResult, ApiError>;
}
```

## Review Operations

### List Reviews

```rust
impl InstallationClient {
    /// List reviews for a pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    ///
    /// # Returns
    ///
    /// Returns vector of reviews in chronological order.
    pub async fn list_pull_request_reviews(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<Vec<Review>, ApiError>;
}
```

### Create Review

```rust
impl InstallationClient {
    /// Create a review for a pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    /// * `request` - Review data
    ///
    /// # Returns
    ///
    /// Returns the created `Review`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing permission
    /// * `ApiError::HttpError` - Already reviewed (422)
    pub async fn create_pull_request_review(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        request: &CreateReviewRequest,
    ) -> Result<Review, ApiError>;
}
```

## Comment Operations

Pull requests support issue-style comments (separate from review comments).

### List PR Comments

```rust
impl InstallationClient {
    /// List all comments on a pull request.
    ///
    /// These are issue-style comments, not review comments.
    /// For review comments, use `list_pull_request_reviews`.
    ///
    /// # Returns
    ///
    /// Returns vector of comments in chronological order.
    pub async fn list_pull_request_comments(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
    ) -> Result<Vec<Comment>, ApiError>;
}
```

### Create PR Comment

```rust
impl InstallationClient {
    /// Add a comment to a pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    /// * `body` - Comment body (Markdown supported)
    ///
    /// # Returns
    ///
    /// Returns the created `Comment`.
    pub async fn create_pull_request_comment(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        body: &str,
    ) -> Result<Comment, ApiError>;
}
```

## Label Operations

Pull requests use the same label operations as issues.

### Add Labels to PR

```rust
impl InstallationClient {
    /// Add labels to a pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    /// * `labels` - Label names to add
    ///
    /// # Returns
    ///
    /// Returns updated list of PR labels.
    pub async fn add_labels_to_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        labels: &[String],
    ) -> Result<Vec<Label>, ApiError>;
}
```

### Remove Label from PR

```rust
impl InstallationClient {
    /// Remove a label from a pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    /// * `label_name` - Label name to remove
    pub async fn remove_label_from_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        label_name: &str,
    ) -> Result<(), ApiError>;
}
```

## Milestone Operations

### Set PR Milestone

```rust
impl InstallationClient {
    /// Set the milestone for a pull request.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `pull_number` - Pull request number
    /// * `milestone_number` - Milestone number (or None to remove)
    ///
    /// # Returns
    ///
    /// Returns the updated `PullRequest`.
    pub async fn set_pull_request_milestone(
        &self,
        owner: &str,
        repo: &str,
        pull_number: u64,
        milestone_number: Option<u64>,
    ) -> Result<PullRequest, ApiError>;
}
```

**Implementation**: Uses `update_pull_request` with milestone field.

## Request Types

### CreatePullRequestRequest

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CreatePullRequestRequest {
    pub title: String,
    pub head: String,
    pub base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
}
```

### UpdatePullRequestRequest

```rust
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdatePullRequestRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<PullRequestState>,
}
```

### MergePullRequestRequest

```rust
#[derive(Debug, Clone, Default, Serialize)]
pub struct MergePullRequestRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_method: Option<MergeMethod>,
}
```

### MergeMethod

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}
```

### MergeResult

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct MergeResult {
    pub sha: String,
    pub merged: bool,
    pub message: String,
}
```

### CreateReviewRequest

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CreateReviewRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub event: ReviewEvent,
}
```

### ReviewEvent

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewEvent {
    Approve,
    RequestChanges,
    Comment,
}
```

### ListPullRequestsParams

```rust
#[derive(Debug, Clone, Default)]
pub struct ListPullRequestsParams {
    pub state: Option<PullRequestState>,
    pub head: Option<String>,
    pub base: Option<String>,
}
```

## Usage Examples

### Create a Pull Request

```rust
let request = CreatePullRequestRequest {
    title: "Add new feature".to_string(),
    head: "feature-branch".to_string(),
    base: "main".to_string(),
    body: Some("Description of changes".to_string()),
    draft: Some(false),
};

let pr = client.create_pull_request("owner", "repo", &request).await?;
println!("Created PR #{}", pr.number);
```

### Approve a Pull Request

```rust
let review = CreateReviewRequest {
    body: Some("LGTM!".to_string()),
    event: ReviewEvent::Approve,
};

client.create_pull_request_review("owner", "repo", 42, &review).await?;
```

### Merge a Pull Request

```rust
let merge_opts = MergePullRequestRequest {
    commit_title: Some("Merge feature".to_string()),
    merge_method: Some(MergeMethod::Squash),
    ..Default::default()
};

let result = client.merge_pull_request("owner", "repo", 42, Some(&merge_opts)).await?;
println!("Merged: {}", result.sha);
```

## Implementation Notes

### API Paths

- Pull requests: `/repos/{owner}/{repo}/pulls`
- Pull request: `/repos/{owner}/{repo}/pulls/{pull_number}`
- Merge: `/repos/{owner}/{repo}/pulls/{pull_number}/merge`
- Reviews: `/repos/{owner}/{repo}/pulls/{pull_number}/reviews`

### Merge Conflicts

When merge fails due to conflicts:

- Returns `ApiError::HttpError` with status 409
- Message indicates conflicts exist

### Testing Strategy

- Mock all HTTP responses
- Test merge method variations
- Test review state transitions
- Verify error handling for conflicts

## References

- GitHub API: [Pull Requests](https://docs.github.com/en/rest/pulls/pulls)
- GitHub API: [Reviews](https://docs.github.com/en/rest/pulls/reviews)
