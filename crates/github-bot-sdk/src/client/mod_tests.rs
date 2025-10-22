//! Tests for GitHub API client module.

use super::*;
use crate::auth::{GitHubAppId, Installation, InstallationId, InstallationToken, JsonWebToken};
use crate::error::AuthError;
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

// ============================================================================
// Mock Authentication Provider
// ============================================================================

#[derive(Clone)]
struct MockAuthProvider {
    jwt_token: Arc<RwLock<Option<JsonWebToken>>>,
    installation_token: Arc<RwLock<Option<InstallationToken>>>,
    should_fail: Arc<RwLock<bool>>,
}

impl MockAuthProvider {
    fn new() -> Self {
        Self {
            jwt_token: Arc::new(RwLock::new(None)),
            installation_token: Arc::new(RwLock::new(None)),
            should_fail: Arc::new(RwLock::new(false)),
        }
    }

    fn with_jwt(jwt: JsonWebToken) -> Self {
        let provider = Self::new();
        // Use a workaround: create the Arc directly with the value
        Self {
            jwt_token: Arc::new(RwLock::new(Some(jwt))),
            ..provider
        }
    }

    fn with_installation_token(token: InstallationToken) -> Self {
        let provider = Self::new();
        Self {
            installation_token: Arc::new(RwLock::new(Some(token))),
            ..provider
        }
    }

    fn failing() -> Self {
        let provider = Self::new();
        Self {
            should_fail: Arc::new(RwLock::new(true)),
            ..provider
        }
    }

    async fn set_should_fail(&self, should_fail: bool) {
        *self.should_fail.write().await = should_fail;
    }
}

#[async_trait::async_trait]
impl AuthenticationProvider for MockAuthProvider {
    async fn app_token(&self) -> Result<JsonWebToken, AuthError> {
        if *self.should_fail.read().await {
            return Err(AuthError::TokenGenerationFailed {
                message: "Mock failure".to_string(),
            });
        }

        self.jwt_token
            .read()
            .await
            .clone()
            .ok_or_else(|| AuthError::TokenGenerationFailed {
                message: "No JWT configured".to_string(),
            })
    }

    async fn installation_token(
        &self,
        _installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        if *self.should_fail.read().await {
            return Err(AuthError::TokenExchangeFailed {
                installation_id: _installation_id,
                message: "Mock failure".to_string(),
            });
        }

        self.installation_token
            .read()
            .await
            .clone()
            .ok_or_else(|| AuthError::TokenExchangeFailed {
                installation_id: _installation_id,
                message: "No installation token configured".to_string(),
            })
    }

    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        self.installation_token(installation_id).await
    }

    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError> {
        Ok(vec![])
    }

    async fn get_installation_repositories(
        &self,
        _installation_id: InstallationId,
    ) -> Result<Vec<crate::auth::Repository>, AuthError> {
        Ok(vec![])
    }
}

// ============================================================================
// ClientConfig Tests
// ============================================================================

mod client_config_tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = ClientConfig::default();

        assert_eq!(config.user_agent, "github-bot-sdk/0.1.0");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_retry_delay, Duration::from_millis(100));
        assert_eq!(config.max_retry_delay, Duration::from_secs(60));
        assert_eq!(config.rate_limit_margin, 0.1);
        assert_eq!(config.github_api_url, "https://api.github.com");
    }

    #[test]
    fn test_config_with_user_agent() {
        let config = ClientConfig::default().with_user_agent("my-bot/1.0");

        assert_eq!(config.user_agent, "my-bot/1.0");
    }

    #[test]
    fn test_config_with_timeout() {
        let config = ClientConfig::default().with_timeout(Duration::from_secs(60));

        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_with_max_retries() {
        let config = ClientConfig::default().with_max_retries(5);

        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_config_with_rate_limit_margin() {
        let config = ClientConfig::default().with_rate_limit_margin(0.2);

        assert_eq!(config.rate_limit_margin, 0.2);
    }

    #[test]
    fn test_config_rate_limit_margin_clamped() {
        let config1 = ClientConfig::default().with_rate_limit_margin(-0.5);
        assert_eq!(config1.rate_limit_margin, 0.0);

        let config2 = ClientConfig::default().with_rate_limit_margin(1.5);
        assert_eq!(config2.rate_limit_margin, 1.0);
    }

    #[test]
    fn test_config_with_github_api_url() {
        let config =
            ClientConfig::default().with_github_api_url("https://github.enterprise.com/api/v3");

        assert_eq!(config.github_api_url, "https://github.enterprise.com/api/v3");
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = ClientConfig::builder()
            .user_agent("test-bot/2.0")
            .timeout(Duration::from_secs(45))
            .max_retries(10)
            .rate_limit_margin(0.15)
            .github_api_url("https://custom.github.com")
            .build();

        assert_eq!(config.user_agent, "test-bot/2.0");
        assert_eq!(config.timeout, Duration::from_secs(45));
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.rate_limit_margin, 0.15);
        assert_eq!(config.github_api_url, "https://custom.github.com");
    }

    #[test]
    fn test_config_builder_default() {
        let config = ClientConfigBuilder::default().build();

        assert_eq!(config.user_agent, "github-bot-sdk/0.1.0");
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_config_method_chaining() {
        let config = ClientConfig::default()
            .with_user_agent("chained-bot/1.0")
            .with_timeout(Duration::from_secs(20))
            .with_max_retries(7);

        assert_eq!(config.user_agent, "chained-bot/1.0");
        assert_eq!(config.timeout, Duration::from_secs(20));
        assert_eq!(config.max_retries, 7);
    }
}

// ============================================================================
// GitHubClient Tests
// ============================================================================

mod github_client_tests {
    use super::*;

    fn create_test_jwt() -> JsonWebToken {
        let app_id = GitHubAppId::new(12345);
        let expires_at = Utc::now() + ChronoDuration::minutes(10);
        JsonWebToken::new("test.jwt.token".to_string(), app_id, expires_at)
    }

    fn create_test_installation_token() -> InstallationToken {
        let installation_id = InstallationId::new(67890);
        let expires_at = Utc::now() + ChronoDuration::hours(1);
        InstallationToken::new(
            "ghs_test_token".to_string(),
            installation_id,
            expires_at,
            Default::default(),
            vec![],
        )
    }

    #[test]
    fn test_client_builder_with_default_config() {
        let auth = MockAuthProvider::new();
        let client = GitHubClient::builder(auth).build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.config().timeout, Duration::from_secs(30));
        assert_eq!(client.config().max_retries, 3);
    }

    #[test]
    fn test_client_builder_with_custom_config() {
        let auth = MockAuthProvider::new();
        let config = ClientConfig::default()
            .with_user_agent("custom-bot/1.0")
            .with_timeout(Duration::from_secs(60));

        let client = GitHubClient::builder(auth).config(config).build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.config().user_agent, "custom-bot/1.0");
        assert_eq!(client.config().timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_client_has_auth_provider() {
        let jwt = create_test_jwt();
        let auth = MockAuthProvider::with_jwt(jwt);
        let client = GitHubClient::builder(auth).build().unwrap();

        // Verify auth provider is accessible
        let _ = client.auth_provider();
    }

    #[tokio::test]
    async fn test_client_can_get_jwt_from_provider() {
        let jwt = create_test_jwt();
        let auth = MockAuthProvider::with_jwt(jwt.clone());
        let client = GitHubClient::builder(auth).build().unwrap();

        let result = client.auth_provider().app_token().await;
        assert!(result.is_ok());

        let retrieved_jwt = result.unwrap();
        assert_eq!(retrieved_jwt.token(), jwt.token());
    }

    #[tokio::test]
    async fn test_client_can_get_installation_token_from_provider() {
        let token = create_test_installation_token();
        let installation_id = token.installation_id();
        let auth = MockAuthProvider::with_installation_token(token.clone());
        let client = GitHubClient::builder(auth).build().unwrap();

        let result = client
            .auth_provider()
            .installation_token(installation_id)
            .await;
        assert!(result.is_ok());

        let retrieved_token = result.unwrap();
        assert_eq!(retrieved_token.token(), token.token());
        assert_eq!(retrieved_token.installation_id(), installation_id);
    }

    #[tokio::test]
    async fn test_client_auth_provider_error_propagates() {
        let auth = MockAuthProvider::new();
        auth.set_should_fail(true).await;

        let client = GitHubClient::builder(auth).build().unwrap();

        let result = client.auth_provider().app_token().await;
        assert!(result.is_err());

        if let Err(AuthError::TokenGenerationFailed { message }) = result {
            assert_eq!(message, "Mock failure");
        } else {
            panic!("Expected TokenGenerationFailed error");
        }
    }

    #[test]
    fn test_client_debug_output_hides_sensitive_data() {
        let auth = MockAuthProvider::new();
        let client = GitHubClient::builder(auth).build().unwrap();

        let debug_output = format!("{:?}", client);

        // Should not contain actual auth provider details
        assert!(debug_output.contains("GitHubClient"));
        assert!(debug_output.contains("ClientConfig"));
        assert!(debug_output.contains("<AuthenticationProvider>"));
    }

    #[test]
    fn test_client_builder_fluent_interface() {
        let auth = MockAuthProvider::new();

        let client = GitHubClient::builder(auth)
            .config(
                ClientConfig::builder()
                    .user_agent("fluent-bot/1.0")
                    .timeout(Duration::from_secs(45))
                    .max_retries(5)
                    .build(),
            )
            .build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.config().user_agent, "fluent-bot/1.0");
        assert_eq!(client.config().timeout, Duration::from_secs(45));
        assert_eq!(client.config().max_retries, 5);
    }

    #[test]
    fn test_multiple_clients_with_same_auth_provider() {
        let auth = MockAuthProvider::new();

        let client1 = GitHubClient::builder(auth.clone()).build();
        let client2 = GitHubClient::builder(auth.clone()).build();

        assert!(client1.is_ok());
        assert!(client2.is_ok());
    }

    #[test]
    fn test_client_config_is_immutable_after_creation() {
        let auth = MockAuthProvider::new();
        let config = ClientConfig::default().with_timeout(Duration::from_secs(60));

        let client = GitHubClient::builder(auth).config(config.clone()).build().unwrap();

        // Original config should not affect client
        let _modified_config = config.with_timeout(Duration::from_secs(120));

        assert_eq!(client.config().timeout, Duration::from_secs(60));
    }
}

// ============================================================================
// App-Level Operations Tests
// ============================================================================

#[cfg(test)]
mod app_operations_tests {
    use super::*;
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn test_get_app_success() {
        // Start mock server
        let mock_server = MockServer::start().await;

        // Mock response matching GitHub API format
        let app_json = serde_json::json!({
            "id": 12345,
            "slug": "my-test-app",
            "name": "My Test App",
            "owner": {
                "id": 1,
                "login": "octocat",
                "type": "User",
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "html_url": "https://github.com/octocat"
            },
            "description": "A test GitHub App",
            "external_url": "https://example.com",
            "html_url": "https://github.com/apps/my-test-app",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z"
        });

        // Set up mock expectation
        Mock::given(method("GET"))
            .and(path("/app"))
            .and(header("Authorization", "Bearer test-jwt-token"))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&app_json))
            .mount(&mock_server)
            .await;

        // Create client with mock server URL
        let jwt = JsonWebToken::new(
            "test-jwt-token".to_string(),
            GitHubAppId::new(12345),
            Utc::now() + ChronoDuration::hours(1),
        );
        let auth = MockAuthProvider::with_jwt(jwt);

        let config = ClientConfig::default().with_github_api_url(mock_server.uri());

        let client = GitHubClient::builder(auth).config(config).build().unwrap();

        // Call get_app
        let result = client.get_app().await;

        assert!(result.is_ok());
        let app = result.unwrap();
        assert_eq!(app.id, 12345);
        assert_eq!(app.slug, "my-test-app");
        assert_eq!(app.name, "My Test App");
        assert_eq!(app.owner.login, "octocat");
        assert_eq!(app.description, Some("A test GitHub App".to_string()));
    }

    #[tokio::test]
    async fn test_get_app_jwt_generation_failure() {
        let auth = MockAuthProvider::failing();

        let client = GitHubClient::builder(auth).build().unwrap();

        let result = client.get_app().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::TokenGenerationFailed { .. } => (),
            other => panic!("Expected TokenGenerationFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_app_http_failure() {
        let mock_server = MockServer::start().await;

        // Mock 500 error response
        Mock::given(method("GET"))
            .and(path("/app"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let jwt = JsonWebToken::new(
            "test-jwt-token".to_string(),
            GitHubAppId::new(12345),
            Utc::now() + ChronoDuration::hours(1),
        );
        let auth = MockAuthProvider::with_jwt(jwt);
        let config = ClientConfig::default().with_github_api_url(mock_server.uri());

        let client = GitHubClient::builder(auth).config(config).build().unwrap();

        let result = client.get_app().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_app_malformed_response() {
        let mock_server = MockServer::start().await;

        // Mock with invalid JSON
        Mock::given(method("GET"))
            .and(path("/app"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
            .mount(&mock_server)
            .await;

        let jwt = JsonWebToken::new(
            "test-jwt-token".to_string(),
            GitHubAppId::new(12345),
            Utc::now() + ChronoDuration::hours(1),
        );
        let auth = MockAuthProvider::with_jwt(jwt);
        let config = ClientConfig::default().with_github_api_url(mock_server.uri());

        let client = GitHubClient::builder(auth).config(config).build().unwrap();

        let result = client.get_app().await;

        assert!(result.is_err());
    }
}
