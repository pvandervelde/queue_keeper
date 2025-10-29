//! Repository Operations
//!
//! **Specification**: `github-bot-sdk-specs/interfaces/repository-operations.md`

use crate::{client::InstallationClient, error::ApiError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;

/// GitHub repository with metadata.
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Repository owner (user or organization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryOwner {
    pub login: String,
    pub id: u64,
    pub avatar_url: String,
    #[serde(rename = "type")]
    pub owner_type: OwnerType,
}

/// Owner type classification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OwnerType {
    User,
    Organization,
}

/// Git branch with commit information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub commit: Commit,
    pub protected: bool,
}

/// Commit reference (used in branches, tags, and pull requests).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub url: String,
}

/// Git reference (branch, tag, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub node_id: String,
    pub url: String,
    pub object: GitRefObject,
}

/// Git reference object information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRefObject {
    pub sha: String,
    #[serde(rename = "type")]
    pub object_type: GitObjectType,
    pub url: String,
}

/// Git object type classification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
}

/// Git tag information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub commit: Commit,
    pub zipball_url: String,
    pub tarball_url: String,
}

/// Request body for creating a Git reference.
#[derive(Debug, Serialize)]
struct CreateGitRefRequest {
    #[serde(rename = "ref")]
    ref_name: String,
    sha: String,
}

/// Request body for updating a Git reference.
#[derive(Debug, Serialize)]
struct UpdateGitRefRequest {
    sha: String,
    force: bool,
}

impl InstallationClient {
    /// Get repository metadata.
    ///
    /// Retrieves complete metadata for a repository including owner information,
    /// visibility settings, default branch, and timestamps.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner (username or organization)
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns `Repository` with complete metadata on success.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Repository does not exist or is not accessible
    /// * `ApiError::AuthorizationFailed` - Insufficient permissions to access repository
    /// * `ApiError::HttpError` - GitHub API returned an error
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let repo = client.get_repository("octocat", "Hello-World").await?;
    /// println!("Repository: {}", repo.full_name);
    /// println!("Default branch: {}", repo.default_branch);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_repository(&self, owner: &str, repo: &str) -> Result<Repository, ApiError> {
        let path = format!("/repos/{}/{}", owner, repo);
        let response = self.get(&path).await?;

        // Map HTTP status codes to appropriate errors
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

        // Parse successful response
        response.json().await.map_err(|e| ApiError::from(e))
    }

    /// List all branches in a repository.
    ///
    /// Returns an array of all branches with their commit information and protection status.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns `Vec<Branch>` with all repository branches.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Repository does not exist
    /// * `ApiError::AuthorizationFailed` - Insufficient permissions
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let branches = client.list_branches("octocat", "Hello-World").await?;
    /// for branch in branches {
    ///     println!("Branch: {} (protected: {})", branch.name, branch.protected);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_branches(&self, owner: &str, repo: &str) -> Result<Vec<Branch>, ApiError> {
        let path = format!("/repos/{}/{}/branches", owner, repo);
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

    /// Get a specific branch by name.
    ///
    /// Retrieves detailed information about a single branch including commit SHA
    /// and protection status.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `branch` - Branch name
    ///
    /// # Returns
    ///
    /// Returns `Branch` with branch details.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Branch or repository does not exist
    /// * `ApiError::AuthorizationFailed` - Insufficient permissions
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let branch = client.get_branch("octocat", "Hello-World", "main").await?;
    /// println!("Branch {} at commit {}", branch.name, branch.commit.sha);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<Branch, ApiError> {
        let path = format!("/repos/{}/{}/branches/{}", owner, repo, branch);
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

    /// Get a Git reference (branch or tag).
    ///
    /// Retrieves information about a Git reference including the SHA it points to.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Reference name (e.g., "heads/main" or "tags/v1.0.0")
    ///
    /// # Returns
    ///
    /// Returns `GitRef` with reference details.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Reference does not exist
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let git_ref = client.get_git_ref("octocat", "Hello-World", "heads/main").await?;
    /// println!("Ref {} points to {}", git_ref.ref_name, git_ref.object.sha);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
    ) -> Result<GitRef, ApiError> {
        let path = format!("/repos/{}/{}/git/refs/{}", owner, repo, ref_name);
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

    /// Create a new Git reference (branch or tag).
    ///
    /// Creates a new reference pointing to the specified SHA.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Full reference name (e.g., "refs/heads/new-branch")
    /// * `sha` - SHA that the reference should point to
    ///
    /// # Returns
    ///
    /// Returns `GitRef` for the newly created reference.
    ///
    /// # Errors
    ///
    /// * `ApiError::InvalidRequest` - Reference already exists or invalid SHA
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let git_ref = client.create_git_ref(
    ///     "octocat",
    ///     "Hello-World",
    ///     "refs/heads/new-feature",
    ///     "aa218f56b14c9653891f9e74264a383fa43fefbd"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
        sha: &str,
    ) -> Result<GitRef, ApiError> {
        let path = format!("/repos/{}/{}/git/refs", owner, repo);
        let request_body = CreateGitRefRequest {
            ref_name: ref_name.to_string(),
            sha: sha.to_string(),
        };

        let response = self.post(&path, &request_body).await?;

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

    /// Update an existing Git reference.
    ///
    /// Updates a reference to point to a new SHA. Use `force=true` to allow
    /// non-fast-forward updates.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Reference name (e.g., "heads/main")
    /// * `sha` - New SHA for the reference
    /// * `force` - Allow non-fast-forward updates
    ///
    /// # Returns
    ///
    /// Returns `GitRef` with updated reference details.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Reference does not exist
    /// * `ApiError::InvalidRequest` - Invalid SHA or non-fast-forward without force
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let git_ref = client.update_git_ref(
    ///     "octocat",
    ///     "Hello-World",
    ///     "heads/feature",
    ///     "bb218f56b14c9653891f9e74264a383fa43fefbd",
    ///     false
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
        sha: &str,
        force: bool,
    ) -> Result<GitRef, ApiError> {
        let path = format!("/repos/{}/{}/git/refs/{}", owner, repo, ref_name);
        let request_body = UpdateGitRefRequest {
            sha: sha.to_string(),
            force,
        };

        let response = self.patch(&path, &request_body).await?;

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

    /// Delete a Git reference.
    ///
    /// Permanently deletes a Git reference. Use with caution as this operation
    /// cannot be undone.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `ref_name` - Reference name (e.g., "heads/old-feature")
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Reference does not exist
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// client.delete_git_ref("octocat", "Hello-World", "heads/old-feature").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_git_ref(
        &self,
        owner: &str,
        repo: &str,
        ref_name: &str,
    ) -> Result<(), ApiError> {
        let path = format!("/repos/{}/{}/git/refs/{}", owner, repo, ref_name);
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

    /// List all tags in a repository.
    ///
    /// Returns an array of all tags with their associated commit information.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    ///
    /// # Returns
    ///
    /// Returns `Vec<Tag>` with all repository tags. Returns empty vector if no tags exist.
    ///
    /// # Errors
    ///
    /// * `ApiError::NotFound` - Repository does not exist
    /// * `ApiError::AuthorizationFailed` - Insufficient permissions
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let tags = client.list_tags("octocat", "Hello-World").await?;
    /// for tag in tags {
    ///     println!("Tag: {} at {}", tag.name, tag.commit.sha);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_tags(&self, owner: &str, repo: &str) -> Result<Vec<Tag>, ApiError> {
        let path = format!("/repos/{}/{}/tags", owner, repo);
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

    /// Create a new branch (convenience wrapper around create_git_ref).
    ///
    /// Creates a new branch reference pointing to the specified commit.
    /// This is a convenience method that automatically adds the "refs/heads/" prefix.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `branch_name` - Branch name (without "refs/heads/" prefix)
    /// * `from_sha` - SHA of the commit to branch from
    ///
    /// # Returns
    ///
    /// Returns `GitRef` for the newly created branch.
    ///
    /// # Errors
    ///
    /// * `ApiError::InvalidRequest` - Branch already exists or invalid SHA
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let branch = client.create_branch(
    ///     "octocat",
    ///     "Hello-World",
    ///     "new-feature",
    ///     "aa218f56b14c9653891f9e74264a383fa43fefbd"
    /// ).await?;
    /// println!("Created branch: {}", branch.ref_name);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
        from_sha: &str,
    ) -> Result<GitRef, ApiError> {
        let ref_name = format!("refs/heads/{}", branch_name);
        self.create_git_ref(owner, repo, &ref_name, from_sha).await
    }

    /// Create a new tag (convenience wrapper around create_git_ref).
    ///
    /// Creates a new tag reference pointing to the specified commit.
    /// This is a convenience method that automatically adds the "refs/tags/" prefix.
    ///
    /// # Arguments
    ///
    /// * `owner` - Repository owner
    /// * `repo` - Repository name
    /// * `tag_name` - Tag name (without "refs/tags/" prefix)
    /// * `from_sha` - SHA of the commit to tag
    ///
    /// # Returns
    ///
    /// Returns `GitRef` for the newly created tag.
    ///
    /// # Errors
    ///
    /// * `ApiError::InvalidRequest` - Tag already exists or invalid SHA
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::InstallationClient;
    /// # async fn example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let tag = client.create_tag(
    ///     "octocat",
    ///     "Hello-World",
    ///     "v1.0.0",
    ///     "aa218f56b14c9653891f9e74264a383fa43fefbd"
    /// ).await?;
    /// println!("Created tag: {}", tag.ref_name);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_tag(
        &self,
        owner: &str,
        repo: &str,
        tag_name: &str,
        from_sha: &str,
    ) -> Result<GitRef, ApiError> {
        let ref_name = format!("refs/tags/{}", tag_name);
        self.create_git_ref(owner, repo, &ref_name, from_sha).await
    }
}
