//! Circuit breaker wrapper for GitHub API client.

use std::sync::Arc;

use github_bot_sdk::{client::GitHubClient, error::ApiError};
use queue_keeper_core::circuit_breaker::{
    github_api_circuit_breaker_config, CircuitBreaker, CircuitBreakerError, CircuitBreakerFactory,
    DefaultCircuitBreaker, DefaultCircuitBreakerFactory,
};

/// GitHub client with circuit breaker protection.
///
/// Wraps github_bot_sdk::GitHubClient with circuit breaker protection to prevent
/// cascading failures when GitHub API experiences issues.
#[derive(Clone)]
pub struct CircuitBreakerGitHubClient {
    /// Underlying GitHub client
    inner: Arc<GitHubClient>,
    /// Circuit breaker for protecting GitHub API operations
    circuit_breaker: DefaultCircuitBreaker<serde_json::Value, ApiError>,
}

impl CircuitBreakerGitHubClient {
    /// Create new circuit breaker protected GitHub client.
    ///
    /// # Arguments
    /// - `inner`: Underlying GitHubClient to protect
    pub fn new(inner: GitHubClient) -> Self {
        let factory = DefaultCircuitBreakerFactory;
        let circuit_breaker_config = github_api_circuit_breaker_config();
        let circuit_breaker = factory.create_typed_circuit_breaker(circuit_breaker_config);

        Self {
            inner: Arc::new(inner),
            circuit_breaker,
        }
    }

    /// Get reference to inner client for operations not requiring circuit breaker.
    pub fn inner(&self) -> &GitHubClient {
        &self.inner
    }
}

/// Production-grade circuit breaker protection for GitHub API operations.
///
/// All operations are protected by circuit breaker to prevent cascading failures
/// when GitHub API experiences issues. The circuit breaker provides:
/// - Automatic failure detection and circuit opening
/// - Graceful degradation during outages
/// - Half-open state for recovery testing
/// - Comprehensive error mapping
impl CircuitBreakerGitHubClient {
    /// List installations with circuit breaker protection.
    pub async fn list_installations(
        &self,
    ) -> Result<Vec<github_bot_sdk::auth::Installation>, CircuitBreakerError<ApiError>> {
        let inner = Arc::clone(&self.inner);
        self.circuit_breaker
            .call(|| async move {
                let installations = inner.list_installations().await?;
                Ok(serde_json::to_value(&installations).unwrap())
            })
            .await
            .map(|v| serde_json::from_value(v).unwrap())
    }

    /// Get installation by ID with circuit breaker protection.
    pub async fn get_installation(
        &self,
        installation_id: github_bot_sdk::auth::InstallationId,
    ) -> Result<github_bot_sdk::auth::Installation, CircuitBreakerError<ApiError>> {
        let inner = Arc::clone(&self.inner);
        self.circuit_breaker
            .call(|| async move {
                let installation = inner.get_installation(installation_id).await?;
                Ok(serde_json::to_value(&installation).unwrap())
            })
            .await
            .map(|v| serde_json::from_value(v).unwrap())
    }

    /// Get app information with circuit breaker protection.
    pub async fn get_app(
        &self,
    ) -> Result<github_bot_sdk::client::App, CircuitBreakerError<ApiError>> {
        let inner = Arc::clone(&self.inner);
        self.circuit_breaker
            .call(|| async move {
                let app = inner.get_app().await?;
                Ok(serde_json::to_value(&app).unwrap())
            })
            .await
            .map(|v| serde_json::from_value(v).unwrap())
    }

    /// Get installation client with circuit breaker protection.
    ///
    /// Note: The returned InstallationClient still has its own retry logic,
    /// so the circuit breaker provides an additional layer of protection.
    pub async fn installation_by_id(
        &self,
        installation_id: github_bot_sdk::auth::InstallationId,
    ) -> Result<github_bot_sdk::client::InstallationClient, CircuitBreakerError<ApiError>> {
        let inner = Arc::clone(&self.inner);
        self.circuit_breaker
            .call(|| async move {
                // Getting the client itself is lightweight (no network call),
                // but we protect it in case future changes add validation
                let client = inner.installation_by_id(installation_id).await?;
                Ok(serde_json::to_value(true).unwrap()) // Dummy value
            })
            .await?;

        // If circuit is closed, actually create the client
        Ok(self
            .inner
            .installation_by_id(installation_id)
            .await
            .unwrap())
    }
}
