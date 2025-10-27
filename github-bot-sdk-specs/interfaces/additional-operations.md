# Additional Operations Interface Specification

**Module**: `github-bot-sdk::client::{milestone, workflow, release}`
**Files**:

- `crates/github-bot-sdk/src/client/milestone.rs`
- `crates/github-bot-sdk/src/client/workflow.rs`
- `crates/github-bot-sdk/src/client/release.rs`

**Dependencies**: `InstallationClient`, `ApiError`, shared types

## Overview

This specification covers additional GitHub operations for milestones, workflows, and releases. These are installation-scoped operations requiring appropriate repository permissions.

## Milestone Operations

### Types

#### Milestone

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub description: Option<String>,
    pub state: MilestoneState,
    pub open_issues: u64,
    pub closed_issues: u64,
    pub html_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub due_on: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub closed_at: Option<OffsetDateTime>,
}
```

#### MilestoneState

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MilestoneState {
    Open,
    Closed,
}
```

### Operations

#### Get Milestone

```rust
impl InstallationClient {
    /// Get a specific milestone by number.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `milestone_number` - Milestone number
    ///
    /// # Returns
    ///
    /// Returns `Milestone` with full metadata.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Milestone doesn't exist
    pub async fn get_milestone(
        &self,
        owner: &str,
        repo: &str,
        milestone_number: u64,
    ) -> Result<Milestone, ApiError>;
}
```

#### List Milestones

```rust
impl InstallationClient {
    /// List milestones in a repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `state` - Optional state filter (open, closed, all)
    ///
    /// # Returns
    ///
    /// Returns vector of milestones.
    pub async fn list_milestones(
        &self,
        owner: &str,
        repo: &str,
        state: Option<MilestoneState>,
    ) -> Result<Vec<Milestone>, ApiError>;
}
```

#### Create Milestone

```rust
impl InstallationClient {
    /// Create a new milestone.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `request` - Milestone creation data
    ///
    /// # Returns
    ///
    /// Returns the created `Milestone`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing permission
    /// * `ApiError::HttpError` - Milestone with title already exists (422)
    pub async fn create_milestone(
        &self,
        owner: &str,
        repo: &str,
        request: &CreateMilestoneRequest,
    ) -> Result<Milestone, ApiError>;
}
```

#### Update Milestone

```rust
impl InstallationClient {
    /// Update an existing milestone.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `milestone_number` - Milestone number
    /// * `request` - Update data
    ///
    /// # Returns
    ///
    /// Returns the updated `Milestone`.
    pub async fn update_milestone(
        &self,
        owner: &str,
        repo: &str,
        milestone_number: u64,
        request: &UpdateMilestoneRequest,
    ) -> Result<Milestone, ApiError>;
}
```

### Request Types

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CreateMilestoneRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", with = "time::serde::rfc3339::option")]
    pub due_on: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateMilestoneRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<MilestoneState>,
    #[serde(skip_serializing_if = "Option::is_none", with = "time::serde::rfc3339::option")]
    pub due_on: Option<OffsetDateTime>,
}
```

## Workflow Operations

### Types

#### Workflow

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub state: WorkflowState,
    pub html_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}
```

#### WorkflowState

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowState {
    Active,
    Disabled,
}
```

#### WorkflowRun

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: String,
    pub workflow_id: u64,
    pub status: WorkflowRunStatus,
    pub conclusion: Option<WorkflowRunConclusion>,
    pub html_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}
```

#### WorkflowRunStatus

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Queued,
    InProgress,
    Completed,
}
```

#### WorkflowRunConclusion

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunConclusion {
    Success,
    Failure,
    Cancelled,
    Skipped,
}
```

### Operations

#### List Workflows

```rust
impl InstallationClient {
    /// List workflows in a repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns vector of workflows.
    pub async fn list_workflows(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Workflow>, ApiError>;
}
```

#### Get Workflow

```rust
impl InstallationClient {
    /// Get a specific workflow by ID.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `workflow_id` - Workflow ID
    ///
    /// # Returns
    ///
    /// Returns `Workflow` with metadata.
    pub async fn get_workflow(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
    ) -> Result<Workflow, ApiError>;
}
```

#### List Workflow Runs

```rust
impl InstallationClient {
    /// List runs for a specific workflow.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `workflow_id` - Workflow ID
    ///
    /// # Returns
    ///
    /// Returns vector of workflow runs.
    pub async fn list_workflow_runs(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
    ) -> Result<Vec<WorkflowRun>, ApiError>;
}
```

#### Trigger Workflow Dispatch

```rust
impl InstallationClient {
    /// Trigger a workflow dispatch event.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `workflow_id` - Workflow ID
    /// * `ref_name` - Git ref (branch or tag)
    /// * `inputs` - Optional workflow inputs
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `actions:write` permission
    /// * `ApiError::NotFound` - Workflow doesn't exist or no dispatch trigger
    pub async fn trigger_workflow_dispatch(
        &self,
        owner: &str,
        repo: &str,
        workflow_id: u64,
        ref_name: &str,
        inputs: Option<serde_json::Value>,
    ) -> Result<(), ApiError>;
}
```

## Release Operations

### Types

#### Release

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub id: u64,
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub html_url: String,
    pub tarball_url: String,
    pub zipball_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: OffsetDateTime,
}
```

### Operations

#### Get Release

```rust
impl InstallationClient {
    /// Get a specific release by ID.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `release_id` - Release ID
    ///
    /// # Returns
    ///
    /// Returns `Release` with full metadata.
    pub async fn get_release(
        &self,
        owner: &str,
        repo: &str,
        release_id: u64,
    ) -> Result<Release, ApiError>;
}
```

#### Get Release by Tag

```rust
impl InstallationClient {
    /// Get a release by tag name.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `tag` - Tag name
    ///
    /// # Returns
    ///
    /// Returns `Release` associated with the tag.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - No release for this tag
    pub async fn get_release_by_tag(
        &self,
        owner: &str,
        repo: &str,
        tag: &str,
    ) -> Result<Release, ApiError>;
}
```

#### List Releases

```rust
impl InstallationClient {
    /// List releases in a repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns vector of releases (most recent first).
    pub async fn list_releases(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Release>, ApiError>;
}
```

#### Create Release

```rust
impl InstallationClient {
    /// Create a new release.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `request` - Release creation data
    ///
    /// # Returns
    ///
    /// Returns the created `Release`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing permission
    /// * `ApiError::HttpError` - Tag doesn't exist (422)
    pub async fn create_release(
        &self,
        owner: &str,
        repo: &str,
        request: &CreateReleaseRequest,
    ) -> Result<Release, ApiError>;
}
```

#### Update Release

```rust
impl InstallationClient {
    /// Update an existing release.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `release_id` - Release ID
    /// * `request` - Update data
    ///
    /// # Returns
    ///
    /// Returns the updated `Release`.
    pub async fn update_release(
        &self,
        owner: &str,
        repo: &str,
        release_id: u64,
        request: &UpdateReleaseRequest,
    ) -> Result<Release, ApiError>;
}
```

#### Delete Release

```rust
impl InstallationClient {
    /// Delete a release.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `release_id` - Release ID
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing permission
    pub async fn delete_release(
        &self,
        owner: &str,
        repo: &str,
        release_id: u64,
    ) -> Result<(), ApiError>;
}
```

### Request Types

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CreateReleaseRequest {
    pub tag_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateReleaseRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
}
```

## API Paths

### Milestones

- List: `/repos/{owner}/{repo}/milestones`
- Get/Update: `/repos/{owner}/{repo}/milestones/{milestone_number}`

### Workflows

- List workflows: `/repos/{owner}/{repo}/actions/workflows`
- Get workflow: `/repos/{owner}/{repo}/actions/workflows/{workflow_id}`
- List runs: `/repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs`
- Dispatch: `/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches`

### Releases

- List: `/repos/{owner}/{repo}/releases`
- Get: `/repos/{owner}/{repo}/releases/{release_id}`
- Get by tag: `/repos/{owner}/{repo}/releases/tags/{tag}`

## Usage Examples

### Create Milestone

```rust
let request = CreateMilestoneRequest {
    title: "v1.0".to_string(),
    description: Some("First release".to_string()),
    due_on: Some(OffsetDateTime::now_utc() + Duration::days(30)),
};

let milestone = client.create_milestone("owner", "repo", &request).await?;
```

### Trigger Workflow

```rust
client.trigger_workflow_dispatch(
    "owner",
    "repo",
    12345,
    "main",
    Some(json!({"debug": true})),
).await?;
```

### Create Release

```rust
let request = CreateReleaseRequest {
    tag_name: "v1.0.0".to_string(),
    name: Some("Version 1.0".to_string()),
    body: Some("Release notes...".to_string()),
    draft: Some(false),
    prerelease: Some(false),
};

let release = client.create_release("owner", "repo", &request).await?;
```

## References

- GitHub API: [Milestones](https://docs.github.com/en/rest/issues/milestones)
- GitHub API: [Workflows](https://docs.github.com/en/rest/actions/workflows)
- GitHub API: [Releases](https://docs.github.com/en/rest/releases/releases)
