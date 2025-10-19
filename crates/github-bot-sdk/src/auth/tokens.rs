//! GitHub App token management and AuthProvider implementation.
//!
//! This module provides the concrete implementation of GitHub App authentication,
//! including JWT generation, installation token exchange, and intelligent caching.
//!
//! See `github-bot-sdk-specs/modules/auth.md` for complete specification.

use async_trait::async_trait;
use chrono::Duration;
use std::sync::Arc;

use super::{
    AuthenticationProvider, GitHubApiClient, Installation, InstallationId, InstallationToken,
    JsonWebToken, JwtClaims, JwtSigner, Repository, SecretProvider, TokenCache,
};
use crate::error::AuthError;

/// Configuration for authentication behavior.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT expiration duration (max 10 minutes per GitHub)
    pub jwt_expiration: Duration,

    /// JWT refresh margin (refresh if expires in this window)
    pub jwt_refresh_margin: Duration,

    /// Installation token cache TTL (refresh before GitHub's 1-hour expiry)
    pub token_cache_ttl: Duration,

    /// Installation token refresh margin (refresh if expires in this window)
    pub token_refresh_margin: Duration,

    /// GitHub API endpoint (for GitHub Enterprise support)
    pub github_api_url: String,

    /// User agent for GitHub API requests
    pub user_agent: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_expiration: Duration::minutes(10),
            jwt_refresh_margin: Duration::minutes(2),
            token_cache_ttl: Duration::minutes(55),
            token_refresh_margin: Duration::minutes(5),
            github_api_url: "https://api.github.com".to_string(),
            user_agent: "github-bot-sdk".to_string(),
        }
    }
}

/// Main GitHub App authentication provider.
///
/// Handles both app-level (JWT) and installation-level (installation token) authentication
/// with intelligent caching and automatic refresh.
pub struct GitHubAppAuth<S, J, A, C>
where
    S: SecretProvider,
    J: JwtSigner,
    A: GitHubApiClient,
    C: TokenCache,
{
    secret_provider: Arc<S>,
    jwt_signer: Arc<J>,
    api_client: Arc<A>,
    token_cache: Arc<C>,
    config: AuthConfig,
}

impl<S, J, A, C> GitHubAppAuth<S, J, A, C>
where
    S: SecretProvider,
    J: JwtSigner,
    A: GitHubApiClient,
    C: TokenCache,
{
    /// Create a new GitHub App authentication provider.
    pub fn new(
        secret_provider: S,
        jwt_signer: J,
        api_client: A,
        token_cache: C,
        config: AuthConfig,
    ) -> Self {
        Self {
            secret_provider: Arc::new(secret_provider),
            jwt_signer: Arc::new(jwt_signer),
            api_client: Arc::new(api_client),
            token_cache: Arc::new(token_cache),
            config,
        }
    }

    /// Get configuration.
    pub fn config(&self) -> &AuthConfig {
        &self.config
    }
}

#[async_trait]
impl<S, J, A, C> AuthenticationProvider for GitHubAppAuth<S, J, A, C>
where
    S: SecretProvider + 'static,
    J: JwtSigner + 'static,
    A: GitHubApiClient + 'static,
    C: TokenCache + 'static,
{
    async fn app_token(&self) -> Result<JsonWebToken, AuthError> {
        // Get app ID from secret provider
        let app_id = self
            .secret_provider
            .get_app_id()
            .await
            .map_err(AuthError::SecretError)?;

        // Check cache first (graceful fallback on cache errors)
        let cached_jwt = match self.token_cache.get_jwt(app_id).await {
            Ok(Some(jwt)) => Some(jwt),
            Ok(None) => None,
            Err(_) => {
                // Cache read error - log and continue with generation
                // This ensures cache failures don't block authentication
                None
            }
        };

        if let Some(jwt) = cached_jwt {
            // Return cached token if it's not expired and not expiring soon
            if !jwt.expires_soon(self.config.jwt_refresh_margin) {
                return Ok(jwt);
            }
        }

        // Generate new JWT
        let private_key = self
            .secret_provider
            .get_private_key()
            .await
            .map_err(AuthError::SecretError)?;

        let now = chrono::Utc::now();
        let expiration = now + self.config.jwt_expiration;

        let claims = JwtClaims {
            iss: app_id,
            iat: now.timestamp(),
            exp: expiration.timestamp(),
        };

        let jwt = self
            .jwt_signer
            .sign_jwt(claims, &private_key)
            .await
            .map_err(AuthError::SigningError)?;

        // Store in cache (ignore cache errors, we have the token)
        let _ = self.token_cache.store_jwt(jwt.clone()).await;

        Ok(jwt)
    }

    async fn installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        // Check cache first
        if let Some(token) = self
            .token_cache
            .get_installation_token(installation_id)
            .await
            .map_err(AuthError::CacheError)?
        {
            // Return cached token if it's not expired and not expiring soon
            if !token.expires_soon(self.config.token_refresh_margin) {
                return Ok(token);
            }
        }

        // Get fresh token from GitHub API
        let jwt = self.app_token().await?;

        let token = self
            .api_client
            .create_installation_access_token(installation_id, &jwt)
            .await
            .map_err(AuthError::ApiError)?;

        // Store in cache (ignore cache errors, we have the token)
        let _ = self
            .token_cache
            .store_installation_token(token.clone())
            .await;

        Ok(token)
    }

    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        // Invalidate cache first
        let _ = self
            .token_cache
            .invalidate_installation_token(installation_id)
            .await;

        // Get fresh token (bypasses cache since we just invalidated)
        let jwt = self.app_token().await?;

        let token = self
            .api_client
            .create_installation_access_token(installation_id, &jwt)
            .await
            .map_err(AuthError::ApiError)?;

        // Store in cache
        let _ = self
            .token_cache
            .store_installation_token(token.clone())
            .await;

        Ok(token)
    }

    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError> {
        let jwt = self.app_token().await?;

        self.api_client
            .list_app_installations(&jwt)
            .await
            .map_err(AuthError::ApiError)
    }

    async fn get_installation_repositories(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<Repository>, AuthError> {
        let token = self.installation_token(installation_id).await?;

        self.api_client
            .list_installation_repositories(installation_id, &token)
            .await
            .map_err(AuthError::ApiError)
    }
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
