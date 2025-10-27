# Issue Operations Interface Specification

**Module**: `github-bot-sdk::client::issue`
**File**: `crates/github-bot-sdk/src/client/issue.rs`
**Dependencies**: `InstallationClient`, `ApiError`, shared types

## Overview

Issue operations provide CRUD operations for GitHub issues, labels, and comments. These are installation-scoped operations requiring appropriate repository permissions.

## Architectural Location

**Layer**: Infrastructure adapter (GitHub API operations)
**Purpose**: Issue and label management
**Required Permissions**: `issues:read` (minimum), `issues:write` (for mutations)

## Core Types

### Issue

Represents a GitHub issue.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: IssueState,
    pub user: User,
    pub labels: Vec<Label>,
    pub assignees: Vec<User>,
    pub comments: u64,
    pub html_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub closed_at: Option<OffsetDateTime>,
}
```

### IssueState

Issue state enum.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
}
```

### Label

Represents an issue label.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
}
```

### Comment

Represents an issue comment.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub user: User,
    pub html_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}
```

### User

Represents a GitHub user.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    pub id: u64,
    pub avatar_url: String,
    pub html_url: String,
}
```

## Issue Operations

### Get Issue

```rust
impl InstallationClient {
    /// Get a specific issue by number.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `issue_number` - Issue number
    ///
    /// # Returns
    ///
    /// Returns `Issue` with full metadata, labels, and assignees.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Issue doesn't exist
    /// * `ApiError::PermissionDenied` - Missing `issues:read` permission
    ///
    /// # Examples
    ///
    /// ```rust
    /// let issue = client.get_issue("octocat", "Hello-World", 1).await?;
    /// println!("Issue: {}", issue.title);
    /// ```
    pub async fn get_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<Issue, ApiError>;
}
```

### List Issues

```rust
impl InstallationClient {
    /// List issues in a repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `params` - Optional query parameters (state, labels, assignee, etc.)
    ///
    /// # Returns
    ///
    /// Returns vector of `Issue` objects matching criteria.
    ///
    /// # Notes
    ///
    /// Returns all matching issues (not paginated).
    /// Future: Support pagination for repositories with many issues.
    pub async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        params: Option<&ListIssuesParams>,
    ) -> Result<Vec<Issue>, ApiError>;
}
```

### Create Issue

```rust
impl InstallationClient {
    /// Create a new issue.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `request` - Issue creation data
    ///
    /// # Returns
    ///
    /// Returns the created `Issue`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `issues:write` permission
    /// * `ApiError::ValidationError` - Invalid title or body
    ///
    /// # Examples
    ///
    /// ```rust
    /// let request = CreateIssueRequest {
    ///     title: "Bug found".to_string(),
    ///     body: Some("Description".to_string()),
    ///     labels: vec!["bug".to_string()],
    ///     assignees: vec!["octocat".to_string()],
    /// };
    /// let issue = client.create_issue("octocat", "Hello-World", &request).await?;
    /// ```
    pub async fn create_issue(
        &self,
        owner: &str,
        repo: &str,
        request: &CreateIssueRequest,
    ) -> Result<Issue, ApiError>;
}
```

### Update Issue

```rust
impl InstallationClient {
    /// Update an existing issue.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `issue_number` - Issue number
    /// * `request` - Update data (all fields optional)
    ///
    /// # Returns
    ///
    /// Returns the updated `Issue`.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Issue doesn't exist
    /// * `ApiError::PermissionDenied` - Missing `issues:write` permission
    pub async fn update_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        request: &UpdateIssueRequest,
    ) -> Result<Issue, ApiError>;
}
```

## Label Operations

### Get Label

```rust
impl InstallationClient {
    /// Get a repository label by name.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `label_name` - Label name
    ///
    /// # Returns
    ///
    /// Returns `Label` with metadata.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Label doesn't exist
    pub async fn get_label(
        &self,
        owner: &str,
        repo: &str,
        label_name: &str,
    ) -> Result<Label, ApiError>;
}
```

### List Labels

```rust
impl InstallationClient {
    /// List all labels in a repository.
    ///
    /// # Returns
    ///
    /// Returns vector of all repository labels.
    pub async fn list_labels(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Label>, ApiError>;
}
```

### Create Label

```rust
impl InstallationClient {
    /// Create a new label.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `request` - Label data (name, color, description)
    ///
    /// # Returns
    ///
    /// Returns the created `Label`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `issues:write`
    /// * `ApiError::HttpError` - Label already exists (422)
    pub async fn create_label(
        &self,
        owner: &str,
        repo: &str,
        request: &CreateLabelRequest,
    ) -> Result<Label, ApiError>;
}
```

### Add Labels to Issue

```rust
impl InstallationClient {
    /// Add labels to an issue.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `issue_number` - Issue number
    /// * `labels` - Label names to add
    ///
    /// # Returns
    ///
    /// Returns updated list of issue labels.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Issue or label doesn't exist
    /// * `ApiError::PermissionDenied` - Missing `issues:write`
    pub async fn add_labels_to_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        labels: &[String],
    ) -> Result<Vec<Label>, ApiError>;
}
```

### Remove Label from Issue

```rust
impl InstallationClient {
    /// Remove a label from an issue.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `issue_number` - Issue number
    /// * `label_name` - Label name to remove
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Issue or label doesn't exist
    /// * `ApiError::PermissionDenied` - Missing `issues:write`
    pub async fn remove_label_from_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        label_name: &str,
    ) -> Result<(), ApiError>;
}
```

## Comment Operations

### List Comments

```rust
impl InstallationClient {
    /// List all comments on an issue.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `issue_number` - Issue number
    ///
    /// # Returns
    ///
    /// Returns vector of comments in chronological order.
    pub async fn list_issue_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<Vec<Comment>, ApiError>;
}
```

### Create Comment

```rust
impl InstallationClient {
    /// Add a comment to an issue.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `issue_number` - Issue number
    /// * `body` - Comment body (Markdown supported)
    ///
    /// # Returns
    ///
    /// Returns the created `Comment`.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Issue doesn't exist
    /// * `ApiError::PermissionDenied` - Missing `issues:write`
    /// * `ApiError::ValidationError` - Empty body
    pub async fn create_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> Result<Comment, ApiError>;
}
```

### Update Comment

```rust
impl InstallationClient {
    /// Update an issue comment.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `comment_id` - Comment ID (not issue number)
    /// * `body` - New comment body
    ///
    /// # Returns
    ///
    /// Returns the updated `Comment`.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Comment doesn't exist
    /// * `ApiError::PermissionDenied` - Not comment author or missing permission
    pub async fn update_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> Result<Comment, ApiError>;
}
```

### Delete Comment

```rust
impl InstallationClient {
    /// Delete an issue comment.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `comment_id` - Comment ID
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Comment doesn't exist
    /// * `ApiError::PermissionDenied` - Not comment author or admin
    pub async fn delete_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> Result<(), ApiError>;
}
```

## Milestone Operations

### Set Issue Milestone

```rust
impl InstallationClient {
    /// Set the milestone for an issue.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `issue_number` - Issue number
    /// * `milestone_number` - Milestone number (or None to remove)
    ///
    /// # Returns
    ///
    /// Returns the updated `Issue`.
    pub async fn set_issue_milestone(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        milestone_number: Option<u64>,
    ) -> Result<Issue, ApiError>;
}
```

**Implementation**: Uses `update_issue` with milestone field.

## Request Types

### CreateIssueRequest

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CreateIssueRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assignees: Vec<String>,
}
```

### UpdateIssueRequest

```rust
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateIssueRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<IssueState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
}
```

### ListIssuesParams

```rust
#[derive(Debug, Clone, Default)]
pub struct ListIssuesParams {
    pub state: Option<IssueState>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
}
```

### CreateLabelRequest

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CreateLabelRequest {
    pub name: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
```

## Error Handling

### Permission Errors

Operations requiring `issues:write`:

- `create_issue`
- `update_issue`
- `create_label`
- `add_labels_to_issue`
- `remove_label_from_issue`
- `create_issue_comment`
- `update_issue_comment`
- `delete_issue_comment`

### Validation Errors

Return `ApiError::ValidationError` when:

- Issue title is empty
- Comment body is empty
- Label color is invalid format

## Usage Examples

### Create and Label an Issue

```rust
let request = CreateIssueRequest {
    title: "Bug: Application crashes on startup".to_string(),
    body: Some("Steps to reproduce...".to_string()),
    labels: vec!["bug".to_string(), "high-priority".to_string()],
    assignees: vec!["maintainer".to_string()],
};

let issue = client.create_issue("owner", "repo", &request).await?;
println!("Created issue #{}", issue.number);
```

### Add Comment to Issue

```rust
let comment = client.create_issue_comment(
    "owner",
    "repo",
    42,
    "Thanks for reporting! We're investigating.",
).await?;
```

### Close an Issue

```rust
let update = UpdateIssueRequest {
    state: Some(IssueState::Closed),
    ..Default::default()
};

client.update_issue("owner", "repo", 42, &update).await?;
```

## Implementation Notes

### API Paths

- Issues: `/repos/{owner}/{repo}/issues`
- Issue: `/repos/{owner}/{repo}/issues/{issue_number}`
- Labels: `/repos/{owner}/{repo}/labels`
- Issue labels: `/repos/{owner}/{repo}/issues/{issue_number}/labels`
- Comments: `/repos/{owner}/{repo}/issues/{issue_number}/comments`
- Comment: `/repos/{owner}/{repo}/issues/comments/{comment_id}`

### Pull Requests

GitHub's API returns pull requests in issue listings. To filter issues only, check that the `pull_request` field is absent.

### Testing Strategy

- Mock all HTTP responses
- Test permission error mapping
- Test validation errors
- Verify correct JSON serialization

## Assertions

Supports:

- **Assertion #3a**: Uses installation tokens
- **Assertion #7**: Issue management operations

## References

- GitHub API: [Issues](https://docs.github.com/en/rest/issues/issues)
- GitHub API: [Labels](https://docs.github.com/en/rest/issues/labels)
- GitHub API: [Comments](https://docs.github.com/en/rest/issues/comments)
