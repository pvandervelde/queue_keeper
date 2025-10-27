//! Installation Client Types and Operations
//!
//! **Specification**: `github-bot-sdk-specs/interfaces/installation-client.md`
//!
//! This module provides installation-scoped access to GitHub API operations.
//! The `InstallationClient` is bound to a specific installation ID and uses
//! installation tokens (not JWTs) for authentication.

use crate::{auth::InstallationId, client::GitHubClient, error::ApiError};
use std::sync::Arc;

#[cfg(test)]
#[path = "installation_tests.rs"]
mod tests;

/// Installation-scoped GitHub API client.
///
/// Holds a reference to the parent `GitHubClient` for shared HTTP client,
/// auth provider, and rate limiter. All operations use installation tokens.
#[derive(Debug, Clone)]
pub struct InstallationClient {
    /// Parent GitHub client (shared HTTP client, auth provider, rate limiter)
    client: Arc<GitHubClient>,
    /// Installation ID this client is bound to
    installation_id: InstallationId,
}

impl InstallationClient {
    /// Create a new installation client.
    ///
    /// # Arguments
    ///
    /// * `client` - Parent GitHubClient
    /// * `installation_id` - Installation ID to bind to
    pub fn new(client: Arc<GitHubClient>, installation_id: InstallationId) -> Self {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }

    /// Get the installation ID this client is bound to.
    pub fn installation_id(&self) -> InstallationId {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }

    /// Make an authenticated GET request to the GitHub API.
    ///
    /// Uses installation token for authentication.
    ///
    /// # Arguments
    ///
    /// * `path` - API path (e.g., "/repos/owner/repo" or "repos/owner/repo")
    ///
    /// # Returns
    ///
    /// Returns the raw `reqwest::Response` for flexible handling.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` for HTTP errors, authentication failures, or network issues.
    pub async fn get(&self, path: &str) -> Result<reqwest::Response, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }

    /// Make an authenticated POST request to the GitHub API.
    ///
    /// # Arguments
    ///
    /// * `path` - API path
    /// * `body` - Request body (will be serialized to JSON)
    ///
    /// # Errors
    ///
    /// Returns `ApiError` for HTTP errors, serialization failures, or network issues.
    pub async fn post<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }

    /// Make an authenticated PUT request to the GitHub API.
    pub async fn put<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }

    /// Make an authenticated DELETE request to the GitHub API.
    pub async fn delete(&self, path: &str) -> Result<reqwest::Response, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }

    /// Make an authenticated PATCH request to the GitHub API.
    pub async fn patch<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }
}

impl GitHubClient {
    /// Create an installation-scoped client.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The GitHub App installation ID
    ///
    /// # Returns
    ///
    /// Returns an `InstallationClient` bound to the specified installation.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if the installation ID is invalid or inaccessible.
    pub async fn installation_by_id(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationClient, ApiError> {
        unimplemented!("See github-bot-sdk-specs/interfaces/installation-client.md")
    }
}
