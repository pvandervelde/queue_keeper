//! Tests for Installation Client
//!
//! **Specification**: `github-bot-sdk-specs/interfaces/installation-client.md`

use super::*;
use crate::auth::{
    AuthenticationProvider, InstallationId, InstallationPermissions, InstallationToken,
    JsonWebToken, RepositoryId,
};
use crate::client::ClientConfig;
use crate::error::{ApiError, AuthError};
use chrono::{Duration, Utc};
use std::sync::Arc;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// Mock AuthenticationProvider for Testing
// ============================================================================

#[derive(Clone)]
struct MockAuthProvider {
    installation_token: Result<InstallationToken, String>,
}

impl MockAuthProvider {
    fn new_with_token(token: &str) -> Self {
        let installation_id = InstallationId::new(12345);
        let expires_at = Utc::now() + Duration::hours(1);
        let permissions = InstallationPermissions::default();
        let repositories = Vec::new();

        Self {
            installation_token: Ok(InstallationToken::new(
                token.to_string(),
                installation_id,
                expires_at,
                permissions,
                repositories,
            )),
        }
    }

    fn new_with_error(error_message: &str) -> Self {
        Self {
            installation_token: Err(error_message.to_string()),
        }
    }
}

#[async_trait::async_trait]
impl AuthenticationProvider for MockAuthProvider {
    async fn app_token(&self) -> Result<JsonWebToken, AuthError> {
        // Not used in installation client tests
        Err(AuthError::TokenGenerationFailed {
            message: "Not implemented for mock".to_string(),
        })
    }

    async fn installation_token(
        &self,
        _installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        self.installation_token
            .clone()
            .map_err(|msg| AuthError::TokenGenerationFailed { message: msg })
    }

    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        // Delegate to installation_token for simplicity in tests
        self.installation_token(installation_id).await
    }

    async fn list_installations(&self) -> Result<Vec<crate::auth::Installation>, AuthError> {
        // Not used in installation client tests
        Err(AuthError::TokenGenerationFailed {
            message: "Not implemented for mock".to_string(),
        })
    }

    async fn get_installation_repositories(
        &self,
        _installation_id: InstallationId,
    ) -> Result<Vec<crate::auth::Repository>, AuthError> {
        // Not used in installation client tests
        Err(AuthError::TokenGenerationFailed {
            message: "Not implemented for mock".to_string(),
        })
    }
}

// ============================================================================
// Construction Tests
// ============================================================================

mod construction_tests {
    use super::*;

    /// Verify InstallationClient::new creates client with correct installation_id.
    ///
    /// From interface spec: InstallationClient wraps GitHubClient and stores installation_id.
    #[test]
    fn test_installation_client_creation() {
        let auth = MockAuthProvider::new_with_token("test-token");
        let github_client = GitHubClient::builder(auth).build().unwrap();
        let installation_id = InstallationId::new(98765);

        let client = InstallationClient::new(Arc::new(github_client), installation_id);

        assert_eq!(client.installation_id(), installation_id);
    }

    /// Verify installation_id() accessor returns the correct ID.
    ///
    /// From interface spec: InstallationClient should expose its installation_id.
    #[test]
    fn test_installation_id_accessor() {
        let auth = MockAuthProvider::new_with_token("test-token");
        let github_client = GitHubClient::builder(auth).build().unwrap();
        let installation_id = InstallationId::new(54321);

        let client = InstallationClient::new(Arc::new(github_client), installation_id);

        assert_eq!(client.installation_id(), InstallationId::new(54321));
    }

    /// Verify GitHubClient::installation_by_id creates installation client.
    ///
    /// From interface spec: Factory method should create InstallationClient bound to installation_id.
    /// Assertion #5: Installation-level operations use installation tokens.
    #[tokio::test]
    async fn test_github_client_installation_by_id() {
        let auth = MockAuthProvider::new_with_token("test-token");
        let github_client = GitHubClient::builder(auth).build().unwrap();
        let installation_id = InstallationId::new(12345);

        let result = github_client.installation_by_id(installation_id).await;

        assert!(result.is_ok());
        let client = result.unwrap();
        assert_eq!(client.installation_id(), installation_id);
    }
}

// ============================================================================
// HTTP Request Tests
// ============================================================================

mod http_request_tests {
    use super::*;

    /// Verify GET request with installation token authentication.
    ///
    /// From interface spec: GET method should use installation token in Authorization header.
    /// Assertion #3a: Installation operations use installation tokens (not JWT).
    #[tokio::test]
    async fn test_get_request() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_installation_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1296269,
                "name": "Hello-World"
            })))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let response = client.get("repos/octocat/Hello-World").await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert!(response.status().is_success());
    }

    /// Verify POST request with JSON body serialization.
    ///
    /// From interface spec: POST method should serialize body as JSON and use installation token.
    #[tokio::test]
    async fn test_post_request() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/issues"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": 1,
                "number": 42
            })))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let body = serde_json::json!({"title": "Bug report"});
        let response = client.post("repos/octocat/Hello-World/issues", &body).await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.status(), 201);
    }

    /// Verify PUT request with JSON body.
    ///
    /// From interface spec: PUT method should serialize body and authenticate.
    #[tokio::test]
    async fn test_put_request() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("PUT"))
            .and(path("/repos/octocat/Hello-World/subscription"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let body = serde_json::json!({"subscribed": true});
        let response = client
            .put("repos/octocat/Hello-World/subscription", &body)
            .await;

        assert!(response.is_ok());
        assert!(response.unwrap().status().is_success());
    }

    /// Verify DELETE request.
    ///
    /// From interface spec: DELETE method should authenticate with installation token.
    #[tokio::test]
    async fn test_delete_request() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("DELETE"))
            .and(path("/repos/octocat/Hello-World/subscription"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let response = client
            .delete("repos/octocat/Hello-World/subscription")
            .await;

        assert!(response.is_ok());
        assert_eq!(response.unwrap().status(), 204);
    }

    /// Verify PATCH request with JSON body.
    ///
    /// From interface spec: PATCH method should serialize body and authenticate.
    #[tokio::test]
    async fn test_patch_request() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/issues/1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let body = serde_json::json!({"state": "closed"});
        let response = client
            .patch("repos/octocat/Hello-World/issues/1", &body)
            .await;

        assert!(response.is_ok());
        assert!(response.unwrap().status().is_success());
    }
}

// ============================================================================
// Path Normalization Tests
// ============================================================================

mod path_normalization_tests {
    use super::*;

    /// Verify paths with leading slash are normalized.
    ///
    /// From interface spec: Path normalization should remove leading slash if present.
    #[tokio::test]
    async fn test_path_with_leading_slash() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        // Mock expects path WITHOUT leading slash
        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        // Pass path WITH leading slash - should be normalized
        let response = client.get("/repos/octocat/Hello-World").await;

        assert!(response.is_ok());
    }

    /// Verify paths without leading slash work correctly.
    ///
    /// From interface spec: Paths without leading slash should work as-is.
    #[tokio::test]
    async fn test_path_without_leading_slash() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        // Pass path WITHOUT leading slash
        let response = client.get("repos/octocat/Hello-World").await;

        assert!(response.is_ok());
    }
}

// ============================================================================
// Token Management Tests
// ============================================================================

mod token_management_tests {
    use super::*;

    /// Verify installation token is obtained from auth provider.
    ///
    /// From interface spec: InstallationClient should get installation token via auth provider.
    /// Assertion #3a: Installation operations use installation tokens.
    #[tokio::test]
    async fn test_installation_token_retrieval() {
        let mock_server = MockServer::start().await;
        let expected_token = "ghs_specific_installation_token";

        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header(
                "Authorization",
                format!("Bearer {}", expected_token),
            ))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(expected_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let response = client.get("test").await;

        assert!(response.is_ok());
    }

    /// Verify token generation failures are mapped to ApiError.
    ///
    /// From interface spec: Token errors should be mapped to ApiError::TokenGenerationFailed.
    #[tokio::test]
    async fn test_token_error_propagation() {
        let auth = MockAuthProvider::new_with_error("Token generation failed");
        let github_client = GitHubClient::builder(auth).build().unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let response = client.get("test").await;

        assert!(response.is_err());
        match response.unwrap_err() {
            ApiError::TokenGenerationFailed { .. } => {
                // Expected error type
            }
            other => panic!("Expected TokenGenerationFailed, got: {:?}", other),
        }
    }
}

// ============================================================================
// Authorization Header Tests
// ============================================================================

mod authorization_header_tests {
    use super::*;

    /// Verify Authorization: Bearer header is set correctly.
    ///
    /// From interface spec: All requests must include Authorization: Bearer {installation_token}.
    /// Assertion #5: Installation-level operations use installation tokens.
    #[tokio::test]
    async fn test_bearer_token_header() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_installation_token_123";

        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let _response = client.get("test").await.unwrap();
        // Mock expectation will verify the header was sent
    }

    /// Verify Accept: application/vnd.github+json header is set.
    ///
    /// From interface spec: All requests must include Accept header for GitHub API.
    #[tokio::test]
    async fn test_accept_header() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token("test-token");
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let _response = client.get("test").await.unwrap();
        // Mock expectation will verify the header was sent
    }

    /// Verify User-Agent header is set from client config.
    ///
    /// From interface spec: User-Agent should be set from ClientConfig.
    #[tokio::test]
    async fn test_user_agent_header() {
        let mock_server = MockServer::start().await;
        let custom_user_agent = "my-bot/1.0.0";

        Mock::given(method("GET"))
            .and(path("/test"))
            .and(header("User-Agent", custom_user_agent))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token("test-token");
        let github_client = GitHubClient::builder(auth)
            .config(
                ClientConfig::default()
                    .with_github_api_url(mock_server.uri())
                    .with_user_agent(custom_user_agent),
            )
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let _response = client.get("test").await.unwrap();
        // Mock expectation will verify the header was sent
    }
}
