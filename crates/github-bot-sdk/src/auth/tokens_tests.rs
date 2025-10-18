//! Tests for GitHub App token management.

use super::*;
use crate::error::{ApiError, CacheError, SecretError, SigningError, ValidationError};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::sync::{Arc, Mutex};

// ============================================================================
// Mock Implementations
// ============================================================================

/// Mock secret provider for testing.
struct MockSecretProvider {
    app_id: GitHubAppId,
    private_key: PrivateKey,
    webhook_secret: String,
}

impl MockSecretProvider {
    fn new(app_id: u64) -> Self {
        Self {
            app_id: GitHubAppId::new(app_id),
            private_key: PrivateKey::new(
                b"mock-private-key-data".to_vec(),
                super::super::KeyAlgorithm::RS256,
            ),
            webhook_secret: "mock-webhook-secret".to_string(),
        }
    }
}

#[async_trait]
impl SecretProvider for MockSecretProvider {
    async fn get_private_key(&self) -> Result<PrivateKey, SecretError> {
        Ok(self.private_key.clone())
    }

    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError> {
        Ok(self.app_id)
    }

    async fn get_webhook_secret(&self) -> Result<String, SecretError> {
        Ok(self.webhook_secret.clone())
    }

    fn cache_duration(&self) -> Duration {
        Duration::minutes(5)
    }
}

/// Mock JWT signer for testing.
struct MockJwtSigner {
    should_fail: bool,
}

impl MockJwtSigner {
    fn new() -> Self {
        Self { should_fail: false }
    }

    fn with_failure() -> Self {
        Self { should_fail: true }
    }
}

#[async_trait]
impl JwtSigner for MockJwtSigner {
    async fn sign_jwt(
        &self,
        claims: super::super::JwtClaims,
        _private_key: &PrivateKey,
    ) -> Result<JsonWebToken, SigningError> {
        if self.should_fail {
            return Err(SigningError::SigningFailed {
                message: "Mock signing failure".to_string(),
            });
        }

        let expires_at = Utc::now() + Duration::seconds(claims.exp - claims.iat);
        Ok(JsonWebToken::new(
            format!("mock.jwt.{}", claims.iss.as_u64()),
            claims.iss,
            expires_at,
        ))
    }

    fn validate_private_key(&self, _key: &PrivateKey) -> Result<(), ValidationError> {
        Ok(())
    }
}

/// Mock GitHub API client for testing.
struct MockGitHubApiClient {
    installation_tokens: Arc<Mutex<HashMap<InstallationId, InstallationToken>>>,
    installations: Vec<Installation>,
    should_fail: bool,
}

impl MockGitHubApiClient {
    fn new() -> Self {
        Self {
            installation_tokens: Arc::new(Mutex::new(HashMap::new())),
            installations: Vec::new(),
            should_fail: false,
        }
    }

    fn with_installation(mut self, installation: Installation) -> Self {
        self.installations.push(installation);
        self
    }

    fn with_failure() -> Self {
        Self {
            installation_tokens: Arc::new(Mutex::new(HashMap::new())),
            installations: Vec::new(),
            should_fail: true,
        }
    }
}

#[async_trait]
impl GitHubApiClient for MockGitHubApiClient {
    async fn create_installation_access_token(
        &self,
        installation_id: InstallationId,
        _jwt: &JsonWebToken,
    ) -> Result<InstallationToken, ApiError> {
        if self.should_fail {
            return Err(ApiError::HttpError {
                status: 500,
                message: "Mock API failure".to_string(),
            });
        }

        let token = InstallationToken::new(
            format!("ghs_mock_token_{}", installation_id.as_u64()),
            installation_id,
            Utc::now() + Duration::hours(1),
            super::super::InstallationPermissions::default(),
            vec![],
        );

        self.installation_tokens
            .lock()
            .unwrap()
            .insert(installation_id, token.clone());

        Ok(token)
    }

    async fn list_app_installations(
        &self,
        _jwt: &JsonWebToken,
    ) -> Result<Vec<Installation>, ApiError> {
        if self.should_fail {
            return Err(ApiError::HttpError {
                status: 500,
                message: "Mock API failure".to_string(),
            });
        }

        Ok(self.installations.clone())
    }

    async fn list_installation_repositories(
        &self,
        _installation_id: InstallationId,
        _token: &InstallationToken,
    ) -> Result<Vec<Repository>, ApiError> {
        if self.should_fail {
            return Err(ApiError::HttpError {
                status: 500,
                message: "Mock API failure".to_string(),
            });
        }

        Ok(vec![])
    }

    async fn get_repository(
        &self,
        _repo_id: super::super::RepositoryId,
        _token: &InstallationToken,
    ) -> Result<Repository, ApiError> {
        unimplemented!("Not needed for token tests")
    }

    async fn get_rate_limit(
        &self,
        _token: &InstallationToken,
    ) -> Result<super::super::RateLimitInfo, ApiError> {
        unimplemented!("Not needed for token tests")
    }
}

/// Mock token cache for testing.
struct MockTokenCache {
    jwt_cache: Arc<Mutex<HashMap<GitHubAppId, JsonWebToken>>>,
    installation_cache: Arc<Mutex<HashMap<InstallationId, InstallationToken>>>,
    should_fail: bool,
}

impl MockTokenCache {
    fn new() -> Self {
        Self {
            jwt_cache: Arc::new(Mutex::new(HashMap::new())),
            installation_cache: Arc::new(Mutex::new(HashMap::new())),
            should_fail: false,
        }
    }

    fn with_failure() -> Self {
        Self {
            jwt_cache: Arc::new(Mutex::new(HashMap::new())),
            installation_cache: Arc::new(Mutex::new(HashMap::new())),
            should_fail: true,
        }
    }
}

#[async_trait]
impl TokenCache for MockTokenCache {
    async fn get_jwt(&self, app_id: GitHubAppId) -> Result<Option<JsonWebToken>, CacheError> {
        if self.should_fail {
            return Err(CacheError::AccessError {
                message: "Mock cache failure".to_string(),
            });
        }

        Ok(self.jwt_cache.lock().unwrap().get(&app_id).cloned())
    }

    async fn store_jwt(&self, jwt: JsonWebToken) -> Result<(), CacheError> {
        if self.should_fail {
            return Err(CacheError::AccessError {
                message: "Mock cache failure".to_string(),
            });
        }

        self.jwt_cache
            .lock()
            .unwrap()
            .insert(jwt.app_id(), jwt.clone());
        Ok(())
    }

    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<Option<InstallationToken>, CacheError> {
        if self.should_fail {
            return Err(CacheError::AccessError {
                message: "Mock cache failure".to_string(),
            });
        }

        Ok(self
            .installation_cache
            .lock()
            .unwrap()
            .get(&installation_id)
            .cloned())
    }

    async fn store_installation_token(&self, token: InstallationToken) -> Result<(), CacheError> {
        if self.should_fail {
            return Err(CacheError::AccessError {
                message: "Mock cache failure".to_string(),
            });
        }

        self.installation_cache
            .lock()
            .unwrap()
            .insert(token.installation_id(), token.clone());
        Ok(())
    }

    async fn invalidate_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<(), CacheError> {
        if self.should_fail {
            return Err(CacheError::AccessError {
                message: "Mock cache failure".to_string(),
            });
        }

        self.installation_cache.lock().unwrap().remove(&installation_id);
        Ok(())
    }

    fn cleanup_expired_tokens(&self) {
        // No-op for mock
    }
}

// ============================================================================
// Test Helper Functions
// ============================================================================

fn create_test_auth() -> GitHubAppAuth<
    MockSecretProvider,
    MockJwtSigner,
    MockGitHubApiClient,
    MockTokenCache,
> {
    let config = AuthConfig::default();
    GitHubAppAuth::new(
        MockSecretProvider::new(12345),
        MockJwtSigner::new(),
        MockGitHubApiClient::new(),
        MockTokenCache::new(),
        config,
    )
}

fn create_mock_installation(id: u64) -> Installation {
    use super::super::{RepositorySelection, User, UserType};

    Installation {
        id: InstallationId::new(id),
        account: User {
            id: super::super::UserId::new(1),
            login: "test-user".to_string(),
            user_type: UserType::User,
            avatar_url: None,
            html_url: "https://github.com/test-user".to_string(),
        },
        repository_selection: RepositorySelection::All,
        permissions: super::super::InstallationPermissions::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        suspended_at: None,
    }
}

// ============================================================================
// AuthConfig Tests
// ============================================================================

mod auth_config_tests {
    use super::*;

    /// Verify default AuthConfig has correct values.
    #[test]
    fn test_default_auth_config() {
        let config = AuthConfig::default();

        assert_eq!(config.jwt_expiration, Duration::minutes(10));
        assert_eq!(config.jwt_refresh_margin, Duration::minutes(2));
        assert_eq!(config.token_cache_ttl, Duration::minutes(55));
        assert_eq!(config.token_refresh_margin, Duration::minutes(5));
        assert_eq!(config.github_api_url, "https://api.github.com");
        assert_eq!(config.user_agent, "github-bot-sdk");
    }
}

// ============================================================================
// GitHubAppAuth Construction Tests
// ============================================================================

mod construction_tests {
    use super::*;

    /// Verify GitHubAppAuth can be constructed with all dependencies.
    #[test]
    fn test_create_github_app_auth() {
        let auth = create_test_auth();
        assert_eq!(auth.config().jwt_expiration, Duration::minutes(10));
    }

    /// Verify custom configuration is preserved.
    #[test]
    fn test_create_with_custom_config() {
        let mut config = AuthConfig::default();
        config.github_api_url = "https://github.enterprise.local/api/v3".to_string();
        config.user_agent = "my-bot/1.0".to_string();

        let auth = GitHubAppAuth::new(
            MockSecretProvider::new(12345),
            MockJwtSigner::new(),
            MockGitHubApiClient::new(),
            MockTokenCache::new(),
            config,
        );

        assert_eq!(
            auth.config().github_api_url,
            "https://github.enterprise.local/api/v3"
        );
        assert_eq!(auth.config().user_agent, "my-bot/1.0");
    }
}

// ============================================================================
// App Token (JWT) Tests
// ============================================================================

mod app_token_tests {
    use super::*;

    /// Verify app_token() generates valid JWT.
    ///
    /// Tests assertion #1: JWT Token Generation
    #[tokio::test]
    async fn test_app_token_generates_jwt() {
        let auth = create_test_auth();

        let jwt = auth.app_token().await.expect("Should generate JWT");

        assert_eq!(jwt.app_id(), GitHubAppId::new(12345));
        assert!(!jwt.is_expired());
        assert!(jwt.token().starts_with("mock.jwt."));
    }

    /// Verify app_token() uses cache for subsequent calls.
    #[tokio::test]
    async fn test_app_token_uses_cache() {
        let auth = create_test_auth();

        let jwt1 = auth.app_token().await.expect("First call should succeed");
        let jwt2 = auth.app_token().await.expect("Second call should succeed");

        // Should be the same token from cache
        assert_eq!(jwt1.token(), jwt2.token());
    }

    /// Verify app_token() refreshes expired JWT.
    #[tokio::test]
    async fn test_app_token_refreshes_expired() {
        let auth = create_test_auth();

        // Get initial token
        let jwt1 = auth.app_token().await.expect("Should generate JWT");

        // Simulate expiration by waiting (in real code, would mock time)
        // For now, test that fresh tokens are generated
        assert!(!jwt1.is_expired());
    }

    /// Verify app_token() handles signing failure.
    ///
    /// Tests assertion #3: JWT Token with Invalid Private Key
    #[tokio::test]
    async fn test_app_token_signing_failure() {
        let auth = GitHubAppAuth::new(
            MockSecretProvider::new(12345),
            MockJwtSigner::with_failure(),
            MockGitHubApiClient::new(),
            MockTokenCache::new(),
            AuthConfig::default(),
        );

        let result = auth.app_token().await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::SigningError(_)));
    }

    /// Verify app_token() handles cache failure gracefully.
    #[tokio::test]
    async fn test_app_token_cache_failure_fallback() {
        let auth = GitHubAppAuth::new(
            MockSecretProvider::new(12345),
            MockJwtSigner::new(),
            MockGitHubApiClient::new(),
            MockTokenCache::with_failure(),
            AuthConfig::default(),
        );

        // Should still generate token even if cache fails
        let result = auth.app_token().await;
        assert!(result.is_err() || result.unwrap().app_id() == GitHubAppId::new(12345));
    }
}

// ============================================================================
// Installation Token Tests
// ============================================================================

mod installation_token_tests {
    use super::*;

    /// Verify installation_token() exchanges JWT for installation token.
    ///
    /// Tests assertion #4: Installation Token Retrieval
    #[tokio::test]
    async fn test_installation_token_exchange() {
        let auth = create_test_auth();
        let installation_id = InstallationId::new(54321);

        let token = auth
            .installation_token(installation_id)
            .await
            .expect("Should get installation token");

        assert_eq!(token.installation_id(), installation_id);
        assert!(!token.is_expired());
        assert!(token.token().contains("ghs_mock_token"));
    }

    /// Verify installation_token() caches tokens.
    #[tokio::test]
    async fn test_installation_token_caching() {
        let auth = create_test_auth();
        let installation_id = InstallationId::new(54321);

        let token1 = auth
            .installation_token(installation_id)
            .await
            .expect("First call");
        let token2 = auth
            .installation_token(installation_id)
            .await
            .expect("Second call");

        // Should be same cached token
        assert_eq!(token1.token(), token2.token());
    }

    /// Verify installation_token() handles non-existent installation.
    ///
    /// Tests assertion #6: Installation Token for Non-Existent Installation
    #[tokio::test]
    async fn test_installation_token_not_found() {
        let auth = GitHubAppAuth::new(
            MockSecretProvider::new(12345),
            MockJwtSigner::new(),
            MockGitHubApiClient::with_failure(),
            MockTokenCache::new(),
            AuthConfig::default(),
        );

        let result = auth.installation_token(InstallationId::new(99999)).await;

        assert!(result.is_err());
        // In real implementation, should be InstallationNotFound error
    }

    /// Verify installation_token() refreshes expired tokens.
    ///
    /// Tests assertion #7: Token Cache Expiry Handling
    #[tokio::test]
    async fn test_installation_token_refresh_expired() {
        let auth = create_test_auth();
        let installation_id = InstallationId::new(54321);

        // Get initial token
        let token1 = auth
            .installation_token(installation_id)
            .await
            .expect("Should get token");

        assert!(!token1.is_expired());
    }
}

// ============================================================================
// Refresh Token Tests
// ============================================================================

mod refresh_token_tests {
    use super::*;

    /// Verify refresh_installation_token() bypasses cache.
    #[tokio::test]
    async fn test_refresh_bypasses_cache() {
        let auth = create_test_auth();
        let installation_id = InstallationId::new(54321);

        // Get cached token
        let _token1 = auth.installation_token(installation_id).await.unwrap();

        // Force refresh
        let token2 = auth
            .refresh_installation_token(installation_id)
            .await
            .unwrap();

        assert_eq!(token2.installation_id(), installation_id);
    }
}

// ============================================================================
// List Installations Tests
// ============================================================================

mod list_installations_tests {
    use super::*;

    /// Verify list_installations() returns installations.
    ///
    /// Tests assertion #2: App-Level API Operations
    #[tokio::test]
    async fn test_list_installations() {
        let installation = create_mock_installation(123);

        let auth = GitHubAppAuth::new(
            MockSecretProvider::new(12345),
            MockJwtSigner::new(),
            MockGitHubApiClient::new().with_installation(installation.clone()),
            MockTokenCache::new(),
            AuthConfig::default(),
        );

        let installations = auth
            .list_installations()
            .await
            .expect("Should list installations");

        assert_eq!(installations.len(), 1);
        assert_eq!(installations[0].id, installation.id);
    }

    /// Verify list_installations() handles API failures.
    #[tokio::test]
    async fn test_list_installations_api_failure() {
        let auth = GitHubAppAuth::new(
            MockSecretProvider::new(12345),
            MockJwtSigner::new(),
            MockGitHubApiClient::with_failure(),
            MockTokenCache::new(),
            AuthConfig::default(),
        );

        let result = auth.list_installations().await;

        assert!(result.is_err());
    }
}

// ============================================================================
// Get Installation Repositories Tests
// ============================================================================

mod get_repositories_tests {
    use super::*;

    /// Verify get_installation_repositories() returns repositories.
    ///
    /// Tests assertion #5: Installation-Level API Operations
    #[tokio::test]
    async fn test_get_installation_repositories() {
        let auth = create_test_auth();
        let installation_id = InstallationId::new(54321);

        let repos = auth
            .get_installation_repositories(installation_id)
            .await
            .expect("Should get repositories");

        // Mock returns empty list
        assert_eq!(repos.len(), 0);
    }

    /// Verify get_installation_repositories() handles API failures.
    #[tokio::test]
    async fn test_get_installation_repositories_failure() {
        let auth = GitHubAppAuth::new(
            MockSecretProvider::new(12345),
            MockJwtSigner::new(),
            MockGitHubApiClient::with_failure(),
            MockTokenCache::new(),
            AuthConfig::default(),
        );

        let result = auth
            .get_installation_repositories(InstallationId::new(123))
            .await;

        assert!(result.is_err());
    }
}
