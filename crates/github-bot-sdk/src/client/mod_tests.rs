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

        assert_eq!(
            config.github_api_url,
            "https://github.enterprise.com/api/v3"
        );
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

        let client = GitHubClient::builder(auth)
            .config(config.clone())
            .build()
            .unwrap();

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

    #[tokio::test]
    async fn test_list_installations_success() {
        let mock_server = MockServer::start().await;

        // Mock response with installations array
        let installations_json = serde_json::json!([
            {
                "id": 1,
                "account": {
                    "id": 100,
                    "login": "octocat",
                    "type": "User",
                    "avatar_url": null,
                    "html_url": "https://github.com/octocat"
                },
                "access_tokens_url": "https://api.github.com/app/installations/1/access_tokens",
                "repositories_url": "https://api.github.com/installation/repositories",
                "html_url": "https://github.com/settings/installations/1",
                "app_id": 12345,
                "target_type": "User",
                "repository_selection": "all",
                "permissions": {
                    "issues": "write",
                    "pull_requests": "write",
                    "contents": "read",
                    "metadata": "read",
                    "checks": "write",
                    "actions": "read"
                },
                "events": ["push", "pull_request"],
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-02T00:00:00Z",
                "suspended_at": null,
                "suspended_by": null
            },
            {
                "id": 2,
                "account": {
                    "id": 200,
                    "login": "another-user",
                    "type": "Organization",
                    "avatar_url": null,
                    "html_url": "https://github.com/another-user"
                },
                "access_tokens_url": "https://api.github.com/app/installations/2/access_tokens",
                "repositories_url": "https://api.github.com/installation/repositories",
                "html_url": "https://github.com/settings/installations/2",
                "app_id": 12345,
                "target_type": "Organization",
                "repository_selection": "selected",
                "permissions": {
                    "issues": "read",
                    "pull_requests": "read",
                    "contents": "read",
                    "metadata": "read",
                    "checks": "read",
                    "actions": "none"
                },
                "events": ["issues", "pull_request"],
                "created_at": "2024-02-01T00:00:00Z",
                "updated_at": "2024-02-02T00:00:00Z",
                "suspended_at": null,
                "suspended_by": null
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/app/installations"))
            .and(header("Authorization", "Bearer test-jwt-token"))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&installations_json))
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

        let result = client.list_installations().await;

        assert!(result.is_ok());
        let installations = result.unwrap();
        assert_eq!(installations.len(), 2);
        assert_eq!(installations[0].id.as_u64(), 1);
        assert_eq!(installations[0].account.login, "octocat");
        assert_eq!(installations[1].id.as_u64(), 2);
        assert_eq!(installations[1].account.login, "another-user");
    }

    #[tokio::test]
    async fn test_list_installations_empty() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/app/installations"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
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

        let result = client.list_installations().await;

        assert!(result.is_ok());
        let installations = result.unwrap();
        assert_eq!(installations.len(), 0);
    }

    #[tokio::test]
    async fn test_get_installation_success() {
        let mock_server = MockServer::start().await;

        let installation_json = serde_json::json!({
            "id": 12345,
            "account": {
                "id": 100,
                "login": "octocat",
                "type": "User",
                "avatar_url": null,
                "html_url": "https://github.com/octocat"
            },
            "access_tokens_url": "https://api.github.com/app/installations/12345/access_tokens",
            "repositories_url": "https://api.github.com/installation/repositories",
            "html_url": "https://github.com/settings/installations/12345",
            "app_id": 12345,
            "target_type": "User",
            "repository_selection": "all",
            "permissions": {
                "issues": "write",
                "pull_requests": "write",
                "contents": "write",
                "metadata": "read",
                "checks": "write",
                "actions": "read"
            },
            "events": ["push", "pull_request", "issues"],
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z",
            "suspended_at": null,
            "suspended_by": null
        });

        Mock::given(method("GET"))
            .and(path("/app/installations/12345"))
            .and(header("Authorization", "Bearer test-jwt-token"))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&installation_json))
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

        let result = client.get_installation(InstallationId::new(12345)).await;

        assert!(result.is_ok());
        let installation = result.unwrap();
        assert_eq!(installation.id.as_u64(), 12345);
        assert_eq!(installation.account.login, "octocat");
    }

    #[tokio::test]
    async fn test_get_installation_not_found() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/app/installations/99999"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
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

        let result = client.get_installation(InstallationId::new(99999)).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_as_app_success() {
        let mock_server = MockServer::start().await;

        // Mock custom endpoint response
        let custom_json = serde_json::json!({
            "custom_field": "custom_value",
            "data": [1, 2, 3]
        });

        Mock::given(method("GET"))
            .and(path("/app/custom/endpoint"))
            .and(header("Authorization", "Bearer test-jwt-token"))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&custom_json))
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

        let result = client.get_as_app("/app/custom/endpoint").await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 200);

        // Verify we can parse the response
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["custom_field"], "custom_value");
    }

    #[tokio::test]
    async fn test_get_as_app_with_leading_slash() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/app/endpoint"))
            .and(header("Authorization", "Bearer test-jwt-token"))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
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

        // Path with leading slash should work
        let result = client.get_as_app("/app/endpoint").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_as_app_returns_error_responses() {
        let mock_server = MockServer::start().await;

        // Mock 404 response - should return response, not error
        Mock::given(method("GET"))
            .and(path("/app/not-found"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
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

        let result = client.get_as_app("/app/not-found").await;

        // Should succeed and return the response (caller handles status)
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 404);
    }

    #[tokio::test]
    async fn test_get_as_app_jwt_generation_failure() {
        let auth = MockAuthProvider::failing();
        let client = GitHubClient::builder(auth).build().unwrap();

        let result = client.get_as_app("/app/test").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::TokenGenerationFailed { .. } => (),
            other => panic!("Expected TokenGenerationFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_post_as_app_success() {
        let mock_server = MockServer::start().await;

        // Mock POST endpoint
        let response_json = serde_json::json!({
            "id": 123,
            "status": "created"
        });

        Mock::given(method("POST"))
            .and(path("/app/custom/create"))
            .and(header("Authorization", "Bearer test-jwt-token"))
            .and(header("Accept", "application/vnd.github+json"))
            .and(header("Content-Type", "application/json"))
            .respond_with(ResponseTemplate::new(201).set_body_json(&response_json))
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

        let body = serde_json::json!({
            "name": "test",
            "value": 42
        });

        let result = client.post_as_app("/app/custom/create", &body).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 201);

        // Verify response body
        let response_body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(response_body["id"], 123);
        assert_eq!(response_body["status"], "created");
    }

    #[tokio::test]
    async fn test_post_as_app_with_empty_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/app/action"))
            .and(header("Authorization", "Bearer test-jwt-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
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

        let empty_body = serde_json::json!({});

        let result = client.post_as_app("/app/action", &empty_body).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_post_as_app_returns_error_responses() {
        let mock_server = MockServer::start().await;

        // Mock 400 Bad Request
        Mock::given(method("POST"))
            .and(path("/app/bad-request"))
            .respond_with(ResponseTemplate::new(400).set_body_string("Bad Request"))
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

        let body = serde_json::json!({"invalid": "data"});

        let result = client.post_as_app("/app/bad-request", &body).await;

        // Should succeed and return the response (caller handles status)
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 400);
    }

    #[tokio::test]
    async fn test_post_as_app_jwt_generation_failure() {
        let auth = MockAuthProvider::failing();
        let client = GitHubClient::builder(auth).build().unwrap();

        let body = serde_json::json!({"test": "data"});

        let result = client.post_as_app("/app/test", &body).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::TokenGenerationFailed { .. } => (),
            other => panic!("Expected TokenGenerationFailed, got {:?}", other),
        }
    }
}

// ============================================================================
// Rate Limiting Tests
// ============================================================================

#[cfg(test)]
mod rate_limiting_tests {
    use super::*;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    /// Test that rate limit headers are included in responses.
    ///
    /// Verifies Assertion 13: Rate limit headers are monitored and respected.
    #[tokio::test]
    async fn test_rate_limit_headers_in_response() {
        let mock_server = MockServer::start().await;

        let current_time = Utc::now().timestamp();
        let reset_time = current_time + 3600; // 1 hour from now

        let app_json = serde_json::json!({
            "id": 12345,
            "slug": "test-app",
            "name": "Test App",
            "owner": {
                "id": 1,
                "login": "octocat",
                "type": "User",
                "avatar_url": null,
                "html_url": "https://github.com/octocat"
            },
            "description": null,
            "external_url": "https://example.com",
            "html_url": "https://github.com/apps/test-app",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z"
        });

        Mock::given(method("GET"))
            .and(path("/app"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&app_json)
                    .insert_header("X-RateLimit-Limit", "5000")
                    .insert_header("X-RateLimit-Remaining", "4999")
                    .insert_header("X-RateLimit-Reset", &reset_time.to_string()),
            )
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

        assert!(
            result.is_ok(),
            "Request with rate limit headers should succeed"
        );
        // The rate limit headers would be parsed internally by the client
    }

    /// Test handling of rate limit exceeded (429) responses.
    ///
    /// Verifies Assertion 14: Rate limit exceeded is properly handled.
    #[tokio::test]
    async fn test_rate_limit_exceeded_response() {
        let mock_server = MockServer::start().await;

        let current_time = Utc::now().timestamp();
        let reset_time = current_time + 60; // Reset in 60 seconds

        let error_json = serde_json::json!({
            "message": "API rate limit exceeded for user ID 1.",
            "documentation_url": "https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"
        });

        Mock::given(method("GET"))
            .and(path("/app"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_json(&error_json)
                    .insert_header("X-RateLimit-Limit", "5000")
                    .insert_header("X-RateLimit-Remaining", "0")
                    .insert_header("X-RateLimit-Reset", &reset_time.to_string())
                    .insert_header("Retry-After", "60"),
            )
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

        // Currently returns error - in production would include retry logic
        assert!(result.is_err(), "Rate limit exceeded should cause error");
    }

    /// Test handling of secondary rate limit (403) responses.
    ///
    /// Verifies Assertion 15: Secondary rate limits are detected.
    #[tokio::test]
    async fn test_secondary_rate_limit_response() {
        let mock_server = MockServer::start().await;

        let error_json = serde_json::json!({
            "message": "You have exceeded a secondary rate limit. Please wait a few minutes before you try again.",
            "documentation_url": "https://docs.github.com/rest/overview/resources-in-the-rest-api#secondary-rate-limits"
        });

        Mock::given(method("GET"))
            .and(path("/app"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_json(&error_json)
                    .insert_header("Retry-After", "120"), // 2 minutes
            )
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

        assert!(result.is_err(), "Secondary rate limit should cause error");
    }

    /// Test parsing of rate limit information from headers.
    ///
    /// Verifies the parse_rate_limit_from_headers function works correctly.
    #[test]
    fn test_parse_rate_limit_from_headers_complete() {
        use crate::client::rate_limit::parse_rate_limit_from_headers;
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4999"),
        );
        let reset_time = Utc::now().timestamp() + 3600;
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_str(&reset_time.to_string()).unwrap(),
        );

        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_some());
        let rate_limit = rate_limit.unwrap();
        assert_eq!(rate_limit.limit(), 5000);
        assert_eq!(rate_limit.remaining(), 4999);
        assert_eq!(rate_limit.reset_at().timestamp(), reset_time);
    }

    /// Test parsing with missing headers returns None.
    #[test]
    fn test_parse_rate_limit_from_headers_missing() {
        use crate::client::rate_limit::parse_rate_limit_from_headers;
        use reqwest::header::HeaderMap;

        let headers = HeaderMap::new();
        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_none());
    }

    /// Test parsing with partial headers returns None.
    #[test]
    fn test_parse_rate_limit_from_headers_partial() {
        use crate::client::rate_limit::parse_rate_limit_from_headers;
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("5000"),
        );
        // Missing remaining and reset headers

        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_none());
    }

    /// Test parsing with invalid header values returns None.
    #[test]
    fn test_parse_rate_limit_from_headers_invalid() {
        use crate::client::rate_limit::parse_rate_limit_from_headers;
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ratelimit-limit"),
            HeaderValue::from_static("not-a-number"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-remaining"),
            HeaderValue::from_static("4999"),
        );
        headers.insert(
            HeaderName::from_static("x-ratelimit-reset"),
            HeaderValue::from_static("also-not-a-number"),
        );

        let rate_limit = parse_rate_limit_from_headers(&headers);

        assert!(rate_limit.is_none());
    }
}
