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
        Self {
            client,
            installation_id,
        }
    }

    /// Get the installation ID this client is bound to.
    pub fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Prepare a request with installation token authentication and normalized path.
    ///
    /// This helper extracts common logic for token retrieval, path normalization,
    /// and URL construction used by all HTTP methods.
    ///
    /// # Arguments
    ///
    /// * `path` - API path (leading slash optional)
    /// * `method` - HTTP method name for error messages
    ///
    /// # Returns
    ///
    /// Returns `(token, url)` tuple with the installation token and complete URL.
    ///
    /// # Errors
    ///
    /// Returns `ApiError::TokenGenerationFailed` if token retrieval fails.
    async fn prepare_request(
        &self,
        path: &str,
        method: &str,
    ) -> Result<(String, String), ApiError> {
        // Get installation token from auth provider
        let token = self
            .client
            .auth_provider()
            .installation_token(self.installation_id)
            .await
            .map_err(|e| ApiError::TokenGenerationFailed {
                message: format!(
                    "{} request to {}: failed to get installation token: {}",
                    method, path, e
                ),
            })?;

        // Normalize path - remove leading slash if present
        let normalized_path = path.strip_prefix('/').unwrap_or(path);

        // Build request URL
        let url = format!(
            "{}/{}",
            self.client.config().github_api_url,
            normalized_path
        );

        Ok((token.token().to_string(), url))
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
        let (token, url) = self.prepare_request(path, "GET").await?;

        // Make authenticated request
        self.client
            .http_client()
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| ApiError::HttpClientError(e))
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
        let (token, url) = self.prepare_request(path, "POST").await?;

        // Make authenticated request with JSON body
        self.client
            .http_client()
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::HttpClientError(e))
    }

    /// Make an authenticated PUT request to the GitHub API.
    pub async fn put<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        let (token, url) = self.prepare_request(path, "PUT").await?;

        // Make authenticated request with JSON body
        self.client
            .http_client()
            .put(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::HttpClientError(e))
    }

    /// Make an authenticated DELETE request to the GitHub API.
    pub async fn delete(&self, path: &str) -> Result<reqwest::Response, ApiError> {
        let (token, url) = self.prepare_request(path, "DELETE").await?;

        // Make authenticated request
        self.client
            .http_client()
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| ApiError::HttpClientError(e))
    }

    /// Make an authenticated PATCH request to the GitHub API.
    pub async fn patch<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        let (token, url) = self.prepare_request(path, "PATCH").await?;

        // Make authenticated request with JSON body
        self.client
            .http_client()
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::HttpClientError(e))
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
        // Create installation client immediately
        // Token validation will happen on first API call
        Ok(InstallationClient::new(
            Arc::new(self.clone()),
            installation_id,
        ))
    }
}
