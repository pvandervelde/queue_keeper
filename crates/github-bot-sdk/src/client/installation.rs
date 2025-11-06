//! Installation Client Types and Operations
//!
//! **Specification**: `github-bot-sdk-specs/interfaces/installation-client.md`
//!
//! This module provides installation-scoped access to GitHub API operations.
//! The `InstallationClient` is bound to a specific installation ID and uses
//! installation tokens (not JWTs) for authentication.

use crate::{
    auth::InstallationId,
    client::{calculate_rate_limit_delay, detect_secondary_rate_limit, GitHubClient},
    error::ApiError,
};
use std::future::Future;
use std::sync::Arc;

#[cfg(test)]
#[path = "installation_tests.rs"]
mod tests;

/// Calculate exponential backoff delay with optional jitter.
///
/// # Arguments
///
/// * `attempt` - Current retry attempt (0-indexed)
/// * `initial_delay` - Initial delay for first retry
/// * `max_delay` - Maximum delay cap
/// * `multiplier` - Backoff multiplier (typically 2.0)
/// * `use_jitter` - Whether to add random jitter (±25%)
fn calculate_exponential_backoff(
    attempt: u32,
    initial_delay: std::time::Duration,
    max_delay: std::time::Duration,
    multiplier: f64,
    use_jitter: bool,
) -> std::time::Duration {
    if attempt == 0 {
        return initial_delay;
    }

    // Calculate exponential backoff
    let exp_multiplier = multiplier.powi(attempt as i32);
    let delay_ms = (initial_delay.as_millis() as f64 * exp_multiplier) as u64;
    let mut delay = std::time::Duration::from_millis(delay_ms);

    // Cap at max delay
    if delay > max_delay {
        delay = max_delay;
    }

    // Add jitter if requested (±25% randomization)
    if use_jitter {
        use rand::Rng;
        let jitter_factor = rand::thread_rng().gen_range(0.75..=1.25);
        delay = std::time::Duration::from_millis((delay.as_millis() as f64 * jitter_factor) as u64);
    }

    delay
}

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

    /// Execute an HTTP request with retry logic for transient errors.
    ///
    /// This method wraps request execution with:
    /// - Exponential backoff retry on transient errors (5xx, 429, network failures)
    /// - Retry-After header parsing for 429 responses
    /// - Secondary rate limit detection for 403 responses
    /// - Maximum retry limit from client configuration
    ///
    /// # Arguments
    ///
    /// * `operation_name` - Name of the operation for logging/debugging
    /// * `request_fn` - Async function that executes the HTTP request
    ///
    /// # Returns
    ///
    /// Returns the successful response or the last error after exhausting retries.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if:
    /// - All retry attempts fail
    /// - A non-retryable error occurs (4xx except 429)
    async fn execute_with_retry<F, Fut>(
        &self,
        operation_name: &str,
        request_fn: F,
    ) -> Result<reqwest::Response, ApiError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<reqwest::Response, ApiError>>,
    {
        let max_retries = self.client.config().max_retries;
        let initial_delay = self.client.config().initial_retry_delay;
        let max_delay = self.client.config().max_retry_delay;
        let backoff_multiplier = 2.0;

        let mut last_error: Option<ApiError> = None;

        for attempt in 0..=max_retries {
            // Execute the request
            match request_fn().await {
                Ok(response) => {
                    let status = response.status().as_u16();

                    // Check for rate limit (429)
                    if status == 429 {
                        if attempt < max_retries {
                            let retry_after = response
                                .headers()
                                .get("Retry-After")
                                .and_then(|v| v.to_str().ok());
                            let rate_limit_reset = response
                                .headers()
                                .get("X-RateLimit-Reset")
                                .and_then(|v| v.to_str().ok());

                            let delay = calculate_rate_limit_delay(retry_after, rate_limit_reset);
                            tokio::time::sleep(delay).await;
                            continue;
                        } else {
                            return Err(ApiError::HttpError {
                                status,
                                message: "Rate limit exceeded after maximum retries".to_string(),
                            });
                        }
                    }

                    // Check for secondary rate limit (403 with abuse detection)
                    if status == 403 {
                        let body = response.text().await.unwrap_or_default();

                        if detect_secondary_rate_limit(status, &body) {
                            if attempt < max_retries {
                                // Secondary rate limits require longer backoff
                                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                                continue;
                            } else {
                                return Err(ApiError::SecondaryRateLimit);
                            }
                        } else {
                            // Not a rate limit, it's a permission error - don't retry
                            // Return HttpError with body content for debugging
                            return Err(ApiError::HttpError {
                                status,
                                message: body,
                            });
                        }
                    }

                    // Check for server errors (5xx)
                    if status >= 500 {
                        if attempt < max_retries {
                            let delay = calculate_exponential_backoff(
                                attempt,
                                initial_delay,
                                max_delay,
                                backoff_multiplier,
                                true, // with jitter
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        } else {
                            let body = response.text().await.unwrap_or_default();
                            return Err(ApiError::HttpError {
                                status,
                                message: body,
                            });
                        }
                    }

                    // Check for other client errors (4xx except 429 and 403 which are handled above)
                    // These are non-retryable - map to appropriate error types
                    if (400..500).contains(&status) {
                        let body = response.text().await.unwrap_or_default();
                        return Err(match status {
                            401 => ApiError::AuthenticationFailed,
                            403 => ApiError::AuthorizationFailed, // Permission denied (not rate limit)
                            404 => ApiError::NotFound,
                            422 => ApiError::InvalidRequest { message: body },
                            _ => ApiError::HttpError {
                                status,
                                message: body,
                            },
                        });
                    }

                    // Success (2xx or 3xx)
                    return Ok(response);
                }
                Err(e) => {
                    // Check if the error is transient
                    if e.is_transient() && attempt < max_retries {
                        last_error = Some(e);
                        let delay = calculate_exponential_backoff(
                            attempt,
                            initial_delay,
                            max_delay,
                            backoff_multiplier,
                            true, // with jitter
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        // Non-retryable error or max retries exhausted
                        return Err(e);
                    }
                }
            }
        }

        // This should never be reached, but return the last error if it happens
        Err(last_error.unwrap_or_else(|| ApiError::HttpError {
            status: 500,
            message: format!(
                "Max retries ({}) exhausted for {}",
                max_retries, operation_name
            ),
        }))
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
    /// Uses installation token for authentication and includes automatic retry logic
    /// for transient errors (5xx, 429, network failures).
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
        let path = path.to_string();
        self.execute_with_retry("GET", || async {
            let (token, url) = self.prepare_request(&path, "GET").await?;

            // Make authenticated request
            self.client
                .http_client()
                .get(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/vnd.github+json")
                .send()
                .await
                .map_err(ApiError::HttpClientError)
        })
        .await
    }

    /// Make an authenticated POST request to the GitHub API.
    ///
    /// Includes automatic retry logic for transient errors.
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
        let path = path.to_string();
        let body_json = serde_json::to_value(body).map_err(ApiError::JsonError)?;

        self.execute_with_retry("POST", || async {
            let (token, url) = self.prepare_request(&path, "POST").await?;

            // Make authenticated request with JSON body
            self.client
                .http_client()
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/vnd.github+json")
                .json(&body_json)
                .send()
                .await
                .map_err(ApiError::HttpClientError)
        })
        .await
    }

    /// Make an authenticated PUT request to the GitHub API.
    ///
    /// Includes automatic retry logic for transient errors.
    pub async fn put<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        let path = path.to_string();
        let body_json = serde_json::to_value(body).map_err(ApiError::JsonError)?;

        self.execute_with_retry("PUT", || async {
            let (token, url) = self.prepare_request(&path, "PUT").await?;

            // Make authenticated request with JSON body
            self.client
                .http_client()
                .put(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/vnd.github+json")
                .json(&body_json)
                .send()
                .await
                .map_err(ApiError::HttpClientError)
        })
        .await
    }

    /// Make an authenticated DELETE request to the GitHub API.
    ///
    /// Includes automatic retry logic for transient errors.
    pub async fn delete(&self, path: &str) -> Result<reqwest::Response, ApiError> {
        let path = path.to_string();
        self.execute_with_retry("DELETE", || async {
            let (token, url) = self.prepare_request(&path, "DELETE").await?;

            // Make authenticated request
            self.client
                .http_client()
                .delete(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/vnd.github+json")
                .send()
                .await
                .map_err(ApiError::HttpClientError)
        })
        .await
    }

    /// Make an authenticated PATCH request to the GitHub API.
    ///
    /// Includes automatic retry logic for transient errors.
    pub async fn patch<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        let path = path.to_string();
        let body_json = serde_json::to_value(body).map_err(ApiError::JsonError)?;

        self.execute_with_retry("PATCH", || async {
            let (token, url) = self.prepare_request(&path, "PATCH").await?;

            // Make authenticated request with JSON body
            self.client
                .http_client()
                .patch(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/vnd.github+json")
                .json(&body_json)
                .send()
                .await
                .map_err(ApiError::HttpClientError)
        })
        .await
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
