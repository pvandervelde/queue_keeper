# Repository Operations Interface Specification

**Module**: `github-bot-sdk::client::repository`
**File**: `crates/github-bot-sdk/src/client/repository.rs`
**Dependencies**: `InstallationClient`, `ApiError`, shared types

## Overview

Repository operations provide access to repository metadata, branch management, and Git reference operations. These are installation-scoped operations that require appropriate permissions.

## Architectural Location

**Layer**: Infrastructure adapter (GitHub API operations)
**Purpose**: Repository and Git reference management
**Required Permissions**: `contents:read` (minimum), `contents:write` (for mutations)

## Core Types

### Repository

Represents a GitHub repository with metadata.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: RepositoryOwner,
    pub description: Option<String>,
    pub private: bool,
    pub default_branch: String,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}
```

### RepositoryOwner

Repository owner (user or organization).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryOwner {
    pub login: String,
    pub id: u64,
    pub avatar_url: String,
    #[serde(rename = "type")]
    pub owner_type: OwnerType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OwnerType {
    User,
    Organization,
}
```

### Branch

Represents a Git branch.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub commit: Commit,
    pub protected: bool,
}
```

### Commit

Represents a Git commit reference (used in branches, tags, and pull requests).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub url: String,
}
```

### GitRef

Represents a Git reference (branch, tag, etc.).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub node_id: String,
    pub url: String,
    pub object: GitRefObject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRefObject {
    pub sha: String,
    #[serde(rename = "type")]
    pub object_type: GitObjectType,
    pub url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
}
```

### Tag

Represents a Git tag.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub commit: Commit,
    pub zipball_url: String,
    pub tarball_url: String,
}
```

## Repository Operations

### Get Repository

```rust
impl InstallationClient {
    /// Get repository metadata.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner login
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns `Repository` with full metadata.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Repository doesn't exist or not accessible
    /// * `ApiError::PermissionDenied` - Missing `contents:read` permission
    /// * `ApiError::HttpError` - Other API errors
    ///
    /// # Examples
    ///
    /// ```rust
    /// let repo = client.get_repository("octocat", "Hello-World").await?;
    /// println!("Repository: {}", repo.full_name);
    /// ```
    pub async fn get_repository(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Repository, ApiError>;
}
```

**Implementation**:

1. Build path: `repos/{owner}/{repo}`
2. Call `self.get(path)`
3. Parse JSON response to `Repository`
4. Map errors appropriately

## Branch Operations

### List Branches

```rust
impl InstallationClient {
    /// List all branches in a repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns vector of `Branch` objects.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Repository not found
    /// * `ApiError::PermissionDenied` - Missing permissions
    ///
    /// # Notes
    ///
    /// This method returns all branches (not paginated for now).
    /// Future: Support pagination for repositories with many branches.
    pub async fn list_branches(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Branch>, ApiError>;
}
```

### Get Branch

```rust
impl InstallationClient {
    /// Get a specific branch by name.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `branch` - Branch name
    ///
    /// # Returns
    ///
    /// Returns `Branch` with commit SHA and protection status.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Branch doesn't exist
    pub async fn get_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<Branch, ApiError>;
}
```

## Git Reference Operations

### Get Git Reference

```rust
impl InstallationClient {
    /// Get a Git reference (branch or tag).
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Reference name (e.g., "heads/main", "tags/v1.0.0")
    ///
    /// # Returns
    ///
    /// Returns `GitRef` with SHA and metadata.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Reference doesn't exist
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Get main branch reference
    /// let git_ref = client.get_git_ref("octocat", "Hello-World", "heads/main").await?;
    /// println!("SHA: {}", git_ref.object.sha);
    ///
    /// // Get tag reference
    /// let tag_ref = client.get_git_ref("octocat", "Hello-World", "tags/v1.0.0").await?;
    /// ```
    pub async fn get_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
    ) -> Result<GitRef, ApiError>;
}
```

### Create Git Reference

```rust
impl InstallationClient {
    /// Create a new Git reference (branch or tag).
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Full reference name (e.g., "refs/heads/feature-branch")
    /// * `sha` - Commit SHA to point reference at
    ///
    /// # Returns
    ///
    /// Returns the created `GitRef`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `contents:write` permission
    /// * `ApiError::HttpError` - Reference already exists (422)
    /// * `ApiError::NotFound` - SHA doesn't exist
    ///
    /// # Examples
    ///
    /// ```rust
    /// let git_ref = client.create_git_ref(
    ///     "octocat",
    ///     "Hello-World",
    ///     "refs/heads/new-feature",
    ///     "abc123def456",
    /// ).await?;
    /// ```
    pub async fn create_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
        sha: &str,
    ) -> Result<GitRef, ApiError>;
}
```

### Update Git Reference

```rust
impl InstallationClient {
    /// Update an existing Git reference.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Reference name (e.g., "heads/main")
    /// * `sha` - New commit SHA
    /// * `force` - Whether to force update (allows non-fast-forward)
    ///
    /// # Returns
    ///
    /// Returns the updated `GitRef`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `contents:write`
    /// * `ApiError::HttpError` - Non-fast-forward without force (422)
    pub async fn update_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
        sha: &str,
        force: bool,
    ) -> Result<GitRef, ApiError>;
}
```

### Delete Git Reference

```rust
impl InstallationClient {
    /// Delete a Git reference.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Reference name (e.g., "heads/feature-branch")
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `contents:write`
    /// * `ApiError::NotFound` - Reference doesn't exist
    pub async fn delete_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
    ) -> Result<(), ApiError>;
}
```

## Tag Operations

### List Tags

```rust
impl InstallationClient {
    /// List all tags in a repository.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns vector of `Tag` objects.
    ///
    /// # Notes
    ///
    /// Returns all tags (not paginated).
    /// Future: Support pagination for repos with many tags.
    pub async fn list_tags(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<Tag>, ApiError>;
}
```

## Convenience Methods for Branch and Tag Creation

These methods provide a more intuitive API for common operations, wrapping the lower-level `create_git_ref` method.

### Create Branch

```rust
impl InstallationClient {
    /// Create a new branch.
    ///
    /// Convenience wrapper around `create_git_ref` for branch creation.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `branch_name` - Name of the new branch (without "refs/heads/" prefix)
    /// * `from_sha` - Commit SHA to create the branch from
    ///
    /// # Returns
    ///
    /// Returns the created `GitRef`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `contents:write` permission
    /// * `ApiError::HttpError` - Branch already exists (422)
    /// * `ApiError::NotFound` - SHA doesn't exist
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Create new branch from main
    /// let main = client.get_branch("owner", "repo", "main").await?;
    /// let branch = client.create_branch("owner", "repo", "feature", &main.commit.sha).await?;
    /// ```
    pub async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
        from_sha: &str,
    ) -> Result<GitRef, ApiError>;
}
```

**Implementation**: Calls `self.create_git_ref(owner, repo, &format!("refs/heads/{}", branch_name), from_sha)`

### Create Tag

```rust
impl InstallationClient {
    /// Create a new lightweight tag.
    ///
    /// Convenience wrapper around `create_git_ref` for tag creation.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `tag_name` - Name of the new tag (without "refs/tags/" prefix)
    /// * `from_sha` - Commit SHA to tag
    ///
    /// # Returns
    ///
    /// Returns the created `GitRef`.
    ///
    /// # Errors
    ///
    /// * `ApiError::PermissionDenied` - Missing `contents:write` permission
    /// * `ApiError::HttpError` - Tag already exists (422)
    /// * `ApiError::NotFound` - SHA doesn't exist
    ///
    /// # Examples
    ///
    /// ```rust
    /// // Create release tag
    /// let commit_sha = "abc123def456";
    /// let tag = client.create_tag("owner", "repo", "v1.0.0", commit_sha).await?;
    /// ```
    ///
    /// # Notes
    ///
    /// This creates a lightweight tag. For annotated tags with messages,
    /// use the Git Data API (not currently implemented).
    pub async fn create_tag(
        &self,
        owner: &str,
        repo: &str,
        tag_name: &str,
        from_sha: &str,
    ) -> Result<GitRef, ApiError>;
}
```

**Implementation**: Calls `self.create_git_ref(owner, repo, &format!("refs/tags/{}", tag_name), from_sha)`

## Request Body Types

### CreateGitRefRequest

```rust
#[derive(Debug, Serialize)]
struct CreateGitRefRequest {
    #[serde(rename = "ref")]
    ref_name: String,
    sha: String,
}
```

### UpdateGitRefRequest

```rust
#[derive(Debug, Serialize)]
struct UpdateGitRefRequest {
    sha: String,
    force: bool,
}
```

## Error Handling

### Permission Errors

Operations requiring `contents:write`:

- `create_git_ref`
- `update_git_ref`
- `delete_git_ref`

All return `ApiError::PermissionDenied` with 403 status.

### Not Found Errors

Return `ApiError::NotFound` (404) when:

- Repository doesn't exist
- Branch doesn't exist
- Reference doesn't exist
- Installation doesn't have access

## Usage Examples

### Get Repository Metadata

```rust
let repo = client.get_repository("octocat", "Hello-World").await?;
println!("Default branch: {}", repo.default_branch);
println!("Created: {}", repo.created_at);
```

### Create a New Branch

```rust
// Get current main branch SHA
let main_branch = client.get_branch("octocat", "Hello-World", "main").await?;
let sha = main_branch.commit.sha;

// Create new branch pointing at same SHA
let new_ref = client.create_git_ref(
    "octocat",
    "Hello-World",
    "refs/heads/feature-branch",
    &sha,
).await?;
```

### List All Tags

```rust
let tags = client.list_tags("octocat", "Hello-World").await?;
for tag in tags {
    println!("Tag: {} -> {}", tag.name, tag.commit.sha);
}
```

## Implementation Notes

### Path Construction

All operations use GitHub REST API v3 paths:

- Repository: `/repos/{owner}/{repo}`
- Branches: `/repos/{owner}/{repo}/branches`
- Branch: `/repos/{owner}/{repo}/branches/{branch}`
- Git refs: `/repos/{owner}/{repo}/git/refs/{ref}`
- Tags: `/repos/{owner}/{repo}/tags`

### Reference Names

Git references use specific prefixes:

- Branches: `refs/heads/{name}` or `heads/{name}`
- Tags: `refs/tags/{name}` or `tags/{name}`

API returns full `refs/` prefix, but accepts shortened form.

### Testing Strategy

- Mock HTTP responses for all operations
- Test error mapping (404, 403, 422)
- Verify correct path construction
- Test reference name normalization

## Assertions

Supports:

- **Assertion #3a**: Uses installation tokens
- **Assertion #6**: Repository-level operations

## References

- GitHub API: [Repositories](https://docs.github.com/en/rest/repos/repos)
- GitHub API: [Git References](https://docs.github.com/en/rest/git/refs)
- `github-bot-sdk-specs/modules/client.md`
