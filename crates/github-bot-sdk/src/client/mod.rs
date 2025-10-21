//! GitHub API client for authenticated operations.
//!
//! This module provides the main `GitHubClient` for making authenticated API calls
//! to GitHub as a GitHub App. It supports both app-level operations (using JWT tokens)
//! and installation-level operations (using installation tokens).
//!
//! See `github-bot-sdk-specs/modules/client.md` for complete specification.

use std::sync::Arc;
use std::time::Duration;

use reqwest;

use crate::auth::AuthenticationProvider;
use crate::error::ApiError;

/// Configuration for GitHub API client behavior.
///
/// Controls timeouts, retry behavior, rate limiting, and API endpoints.
///
/// # Examples
///
/// ```
/// use github_bot_sdk::client::ClientConfig;
/// use std::time::Duration;
///
/// let config = ClientConfig::default()
///     .with_timeout(Duration::from_secs(60))
///     .with_max_retries(5);
/// ```
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// User agent string for API requests (required by GitHub)
    pub user_agent: String,
    /// Request timeout duration
    pub timeout: Duration,
    /// Maximum number of retry attempts for transient failures
    pub max_retries: u32,
    /// Base delay for exponential backoff retries
    pub initial_retry_delay: Duration,
    /// Maximum delay between retries
    pub max_retry_delay: Duration,
    /// Rate limit safety margin (0.0 to 1.0) - buffer before hitting limits
    pub rate_limit_margin: f64,
    /// GitHub API base URL
    pub github_api_url: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            user_agent: "github-bot-sdk/0.1.0".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(60),
            rate_limit_margin: 0.1, // Keep 10% buffer
            github_api_url: "https://api.github.com".to_string(),
        }
    }
}

impl ClientConfig {
    /// Create a new builder for client configuration.
    pub fn builder() -> ClientConfigBuilder {
        ClientConfigBuilder::new()
    }

    /// Set the user agent string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum number of retries.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the rate limit safety margin.
    pub fn with_rate_limit_margin(mut self, margin: f64) -> Self {
        self.rate_limit_margin = margin.clamp(0.0, 1.0);
        self
    }

    /// Set the GitHub API base URL.
    pub fn with_github_api_url(mut self, url: impl Into<String>) -> Self {
        self.github_api_url = url.into();
        self
    }
}

/// Builder for constructing `ClientConfig` instances.
#[derive(Debug)]
pub struct ClientConfigBuilder {
    config: ClientConfig,
}

impl ClientConfigBuilder {
    /// Create a new configuration builder with defaults.
    pub fn new() -> Self {
        Self {
            config: ClientConfig::default(),
        }
    }

    /// Set the user agent string.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.config.user_agent = user_agent.into();
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set the maximum number of retries.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// Set the rate limit safety margin.
    pub fn rate_limit_margin(mut self, margin: f64) -> Self {
        self.config.rate_limit_margin = margin.clamp(0.0, 1.0);
        self
    }

    /// Set the GitHub API base URL.
    pub fn github_api_url(mut self, url: impl Into<String>) -> Self {
        self.config.github_api_url = url.into();
        self
    }

    /// Build the final configuration.
    pub fn build(self) -> ClientConfig {
        self.config
    }
}

impl Default for ClientConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// GitHub API client for authenticated operations.
///
/// The main client for interacting with GitHub's REST API. Handles authentication,
/// rate limiting, retries, and provides both app-level and installation-level operations.
///
/// # Examples
///
/// ```no_run
/// # use github_bot_sdk::client::{GitHubClient, ClientConfig};
/// # use github_bot_sdk::auth::AuthenticationProvider;
/// # async fn example(auth: impl AuthenticationProvider + 'static) -> Result<(), Box<dyn std::error::Error>> {
/// let client = GitHubClient::builder(auth)
///     .config(ClientConfig::default())
///     .build()?;
///
/// // Get app information
/// let app = client.get_app().await?;
/// println!("App: {}", app.name);
/// # Ok(())
/// # }
/// ```
pub struct GitHubClient {
    auth: Arc<dyn AuthenticationProvider>,
    http_client: reqwest::Client,
    config: ClientConfig,
}

impl GitHubClient {
    /// Create a new builder for constructing a GitHub client.
    ///
    /// # Arguments
    ///
    /// * `auth` - Authentication provider for obtaining tokens
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use github_bot_sdk::client::GitHubClient;
    /// # use github_bot_sdk::auth::AuthenticationProvider;
    /// # async fn example(auth: impl AuthenticationProvider + 'static) {
    /// let client = GitHubClient::builder(auth).build().unwrap();
    /// # }
    /// ```
    pub fn builder(auth: impl AuthenticationProvider + 'static) -> GitHubClientBuilder {
        GitHubClientBuilder::new(auth)
    }

    /// Get the client configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Get the authentication provider.
    pub fn auth_provider(&self) -> &dyn AuthenticationProvider {
        self.auth.as_ref()
    }

    // App-level operations will be implemented in subsequent tasks
    // Installation-level operations will be implemented in task 5.0
}

impl std::fmt::Debug for GitHubClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubClient")
            .field("config", &self.config)
            .field("auth", &"<AuthenticationProvider>")
            .finish()
    }
}

/// Builder for constructing `GitHubClient` instances.
pub struct GitHubClientBuilder {
    auth: Arc<dyn AuthenticationProvider>,
    config: Option<ClientConfig>,
}

impl GitHubClientBuilder {
    /// Create a new client builder.
    fn new(auth: impl AuthenticationProvider + 'static) -> Self {
        Self {
            auth: Arc::new(auth),
            config: None,
        }
    }

    /// Set the client configuration.
    ///
    /// If not set, uses `ClientConfig::default()`.
    pub fn config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Build the GitHub client.
    ///
    /// # Errors
    ///
    /// Returns `ApiError::Configuration` if the HTTP client cannot be created.
    pub fn build(self) -> Result<GitHubClient, ApiError> {
        let config = self.config.unwrap_or_default();

        // Build reqwest client with timeout and user agent
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| ApiError::Configuration {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        Ok(GitHubClient {
            auth: self.auth,
            http_client,
            config,
        })
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
