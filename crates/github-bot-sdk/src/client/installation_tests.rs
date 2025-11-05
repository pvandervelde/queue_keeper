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

// ============================================================================
// Retry Logic Tests
// ============================================================================

mod retry_logic_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc as StdArc;

    /// Verify retry on transient 500 server error succeeds after one retry.
    ///
    /// From spec: Transient errors (5xx) should trigger retry with exponential backoff.
    /// Assertion #20: Network connectivity failures trigger retry logic.
    /// Assertion #21: Server errors (5xx) are retried with backoff.
    #[tokio::test]
    async fn test_retry_on_500_error_succeeds() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        // First request fails with 500, second succeeds
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    ResponseTemplate::new(500).set_body_string("Internal Server Error")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
            .expect(2)
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

        let response = client.get("test").await;

        // Should succeed after retry
        assert!(response.is_ok());
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
    }

    /// Verify retry on 503 Service Unavailable succeeds after retries.
    ///
    /// From spec: 503 errors are transient and should be retried.
    #[tokio::test]
    async fn test_retry_on_503_error_succeeds() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    ResponseTemplate::new(503).set_body_string("Service Unavailable")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
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

        let response = client.get("test").await;

        assert!(response.is_ok());
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 3);
    }

    /// Verify max retries limit is respected.
    ///
    /// From spec: Should not retry indefinitely - respect max_retries configuration.
    #[tokio::test]
    async fn test_max_retries_exceeded() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        // Always return 500
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(500).set_body_string("Internal Server Error")
            })
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(
                ClientConfig::default()
                    .with_github_api_url(mock_server.uri())
                    .with_max_retries(3),
            )
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let response = client.get("test").await;

        // Should fail after max retries (1 initial + 3 retries = 4 attempts)
        assert!(response.is_err());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 4);
    }

    /// Verify non-retryable 404 error fails immediately.
    ///
    /// From spec: Client errors (4xx except 429) should not be retried.
    #[tokio::test]
    async fn test_non_retryable_404_fails_immediately() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(404).set_body_string("Not Found")
            })
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

        let response = client.get("test").await;

        // Should fail immediately without retries
        assert!(response.is_err());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 1);
    }

    /// Verify non-retryable 401 authentication error fails immediately.
    ///
    /// From spec: Authentication errors should not trigger retries.
    #[tokio::test]
    async fn test_non_retryable_401_fails_immediately() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(401).set_body_string("Unauthorized")
            })
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

        let response = client.get("test").await;

        assert!(response.is_err());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 1);
    }

    /// Verify 429 rate limit with Retry-After header is respected.
    ///
    /// From spec: 429 responses should parse Retry-After and delay accordingly.
    /// Assertion #13: Rate limiting headers are parsed and respected.
    #[tokio::test]
    async fn test_429_with_retry_after_header() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    // First request: rate limited with Retry-After
                    ResponseTemplate::new(429)
                        .insert_header("Retry-After", "2")
                        .set_body_json(serde_json::json!({
                            "message": "API rate limit exceeded"
                        }))
                } else {
                    // Second request: success
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
            .expect(2)
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

        let start = std::time::Instant::now();
        let response = client.get("test").await;
        let elapsed = start.elapsed();

        // Should succeed after waiting for Retry-After
        assert!(response.is_ok());
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
        // Should have waited at least 2 seconds (with some tolerance for jitter/overhead)
        assert!(elapsed.as_secs() >= 1);
    }

    /// Verify 403 secondary rate limit (abuse detection) is retried.
    ///
    /// From spec: 403 with abuse detection indicators should be retried with longer backoff.
    /// Assertion #21: Secondary rate limits trigger appropriate backoff.
    #[tokio::test]
    async fn test_403_secondary_rate_limit_retry() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    // First request: secondary rate limit
                    ResponseTemplate::new(403).set_body_json(serde_json::json!({
                        "message": "You have exceeded a secondary rate limit. Please wait a few minutes before you try again."
                    }))
                } else {
                    // Second request: success
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
            .expect(2)
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

        let response = client.get("test").await;

        // Should succeed after retry
        assert!(response.is_ok());
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
    }

    /// Verify 403 permission denied (non-abuse) fails immediately.
    ///
    /// From spec: 403 without abuse indicators is a permission error and should not retry.
    #[tokio::test]
    async fn test_403_permission_denied_fails_immediately() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(403).set_body_json(serde_json::json!({
                    "message": "Resource not accessible by integration"
                }))
            })
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

        let response = client.get("test").await;

        // Should fail immediately without retries (permission error)
        assert!(response.is_err());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 1);
    }

    /// Verify POST request retries on transient errors.
    ///
    /// From spec: Retry logic should work for all HTTP methods, not just GET.
    #[tokio::test]
    async fn test_post_request_retry() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("POST"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    ResponseTemplate::new(502).set_body_string("Bad Gateway")
                } else {
                    ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 42}))
                }
            })
            .expect(2)
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

        let body = serde_json::json!({"data": "test"});
        let response = client.post("test", &body).await;

        assert!(response.is_ok());
        assert_eq!(response.unwrap().status(), 201);
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
    }

    /// Verify PUT request retries on transient errors.
    #[tokio::test]
    async fn test_put_request_retry() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("PUT"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    ResponseTemplate::new(500)
                } else {
                    ResponseTemplate::new(200)
                }
            })
            .expect(2)
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

        let body = serde_json::json!({"data": "test"});
        let response = client.put("test", &body).await;

        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
    }

    /// Verify DELETE request retries on transient errors.
    #[tokio::test]
    async fn test_delete_request_retry() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("DELETE"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    ResponseTemplate::new(503)
                } else {
                    ResponseTemplate::new(204)
                }
            })
            .expect(2)
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

        let response = client.delete("test").await;

        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
    }

    /// Verify PATCH request retries on transient errors.
    #[tokio::test]
    async fn test_patch_request_retry() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("PATCH"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    ResponseTemplate::new(500)
                } else {
                    ResponseTemplate::new(200)
                }
            })
            .expect(2)
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

        let body = serde_json::json!({"data": "test"});
        let response = client.patch("test", &body).await;

        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
    }
}

// ============================================================================
// Comprehensive Rate Limiting and Retry Integration Tests
// ============================================================================

mod rate_limit_integration_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc as StdArc;

    /// Verify exponential backoff increases delay on repeated failures.
    ///
    /// From spec: Exponential backoff with increasing delays.
    /// Assertion #14: Exponential backoff implemented correctly.
    #[tokio::test]
    async fn test_exponential_backoff_progression() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        // Fail 3 times, then succeed
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt < 3 {
                    ResponseTemplate::new(500).set_body_string("Internal Server Error")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(
                ClientConfig::default()
                    .with_github_api_url(mock_server.uri())
                    .with_max_retries(5), // Allow enough retries
            )
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let start = std::time::Instant::now();
        let response = client.get("test").await;
        let elapsed = start.elapsed();

        // Should succeed after retries
        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 4);

        // Should have waited for backoff delays
        // With exponential backoff: 100ms, 200ms, 400ms = ~700ms minimum
        // (actual may be higher due to jitter)
        assert!(elapsed.as_millis() >= 500); // Allow some tolerance
    }

    /// Verify 429 rate limit triggers retry with proper Retry-After delay.
    ///
    /// From spec: 429 responses should parse Retry-After and delay accordingly.
    /// Assertion #14: Retry delays respect Retry-After header.
    #[tokio::test]
    async fn test_429_respects_retry_after_header_integration() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    ResponseTemplate::new(429)
                        .insert_header("Retry-After", "2")
                        .set_body_json(serde_json::json!({
                            "message": "API rate limit exceeded"
                        }))
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
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

        let start = std::time::Instant::now();
        let response = client.get("test").await;
        let elapsed = start.elapsed();

        // Should succeed after waiting
        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);

        // Should have waited at least 2 seconds (with tolerance for jitter)
        assert!(elapsed.as_secs() >= 1);
    }

    /// Verify secondary rate limit (403 abuse) triggers appropriate retry.
    ///
    /// From spec: 403 with abuse detection should retry with longer backoff.
    /// Assertion #15: Secondary rate limits detected and handled.
    #[tokio::test]
    async fn test_secondary_rate_limit_handling_integration() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    // Secondary rate limit response
                    ResponseTemplate::new(403).set_body_json(serde_json::json!({
                        "message": "You have exceeded a secondary rate limit. Please wait a few minutes before you try again.",
                        "documentation_url": "https://docs.github.com/en/rest/overview/resources-in-the-rest-api#secondary-rate-limits"
                    }))
                } else {
                    // Success after backoff
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
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

        let response = client.get("test").await;

        // Should succeed after retry
        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);
    }

    /// Verify complete lifecycle: healthy → approaching limit → throttled → recovered.
    ///
    /// From spec: Full rate limiting lifecycle handling.
    /// Assertion #13: Complete rate limit lifecycle.
    #[tokio::test]
    async fn test_complete_rate_limit_lifecycle() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let request_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = request_counter.clone();

        let initial_reset = Utc::now() + Duration::hours(1);

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);

                match count {
                    // Healthy: plenty of requests remaining
                    0 => ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"status": "healthy"}))
                        .insert_header("X-RateLimit-Limit", "5000")
                        .insert_header("X-RateLimit-Remaining", "4500")
                        .insert_header("X-RateLimit-Reset", initial_reset.timestamp().to_string()),

                    // Approaching exhaustion: few requests left
                    1 => ResponseTemplate::new(200)
                        .set_body_json(serde_json::json!({"status": "approaching"}))
                        .insert_header("X-RateLimit-Limit", "5000")
                        .insert_header("X-RateLimit-Remaining", "100")
                        .insert_header("X-RateLimit-Reset", initial_reset.timestamp().to_string()),

                    // Exhausted: hit rate limit
                    2 => ResponseTemplate::new(429)
                        .insert_header("Retry-After", "1")
                        .set_body_json(serde_json::json!({
                            "message": "API rate limit exceeded"
                        })),

                    // After reset: fresh limits
                    _ => {
                        let new_reset = Utc::now() + Duration::hours(1);
                        ResponseTemplate::new(200)
                            .set_body_json(serde_json::json!({"status": "recovered"}))
                            .insert_header("X-RateLimit-Limit", "5000")
                            .insert_header("X-RateLimit-Remaining", "5000")
                            .insert_header("X-RateLimit-Reset", new_reset.timestamp().to_string())
                    }
                }
            })
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

        // Phase 1: Healthy - request succeeds
        let response = client.get("test").await;
        assert!(response.is_ok());

        // Phase 2: Approaching exhaustion - request still succeeds
        let response = client.get("test").await;
        assert!(response.is_ok());

        // Phase 3: Hit rate limit (429), then retry and succeed
        let response = client.get("test").await;
        assert!(response.is_ok()); // Should succeed after retry

        // Verify all requests eventually succeeded
        assert_eq!(request_counter.load(Ordering::SeqCst), 4);
    }

    /// Verify retry behavior with missing Retry-After header uses default delay.
    ///
    /// From spec: Default 60 second delay when Retry-After not provided.
    /// Assertion #14: Default delay used when headers missing.
    #[tokio::test]
    async fn test_429_without_retry_after_uses_default_delay() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    // 429 without Retry-After header
                    ResponseTemplate::new(429).set_body_json(serde_json::json!({
                        "message": "API rate limit exceeded"
                    }))
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
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

        let start = std::time::Instant::now();
        let response = client.get("test").await;
        let elapsed = start.elapsed();

        // Should succeed after retry
        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 2);

        // Should have waited at least the default delay (60 seconds default minus jitter)
        // In practice, this will wait ~60 seconds, but we'll verify it waited a significant time
        assert!(elapsed.as_secs() >= 45); // 60s with 25% jitter = 45-75s range
    }

    /// Verify retry logic handles malformed rate limit headers gracefully.
    ///
    /// From spec: Graceful handling of invalid headers.
    #[tokio::test]
    async fn test_handles_malformed_rate_limit_headers() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": true}))
                    // Malformed headers
                    .insert_header("X-RateLimit-Limit", "not-a-number")
                    .insert_header("X-RateLimit-Remaining", "invalid")
                    .insert_header("X-RateLimit-Reset", "bad-timestamp"),
            )
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

        // Should succeed despite malformed headers
        let response = client.get("test").await;
        assert!(response.is_ok());
    }

    /// Verify multiple consecutive retries for persistent transient errors.
    ///
    /// From spec: Retry with backoff for transient errors up to max attempts.
    /// Assertion #14: Multiple retries with progressive backoff.
    #[tokio::test]
    async fn test_multiple_consecutive_retries() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";
        let attempt_counter = StdArc::new(AtomicU32::new(0));
        let counter_clone = attempt_counter.clone();

        // Fail 4 times with 502, then succeed
        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(move |_req: &wiremock::Request| {
                let attempt = counter_clone.fetch_add(1, Ordering::SeqCst);
                if attempt < 4 {
                    ResponseTemplate::new(502).set_body_string("Bad Gateway")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true}))
                }
            })
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(
                ClientConfig::default()
                    .with_github_api_url(mock_server.uri())
                    .with_max_retries(5), // Allow 5 retries
            )
            .build()
            .unwrap();

        let installation_id = InstallationId::new(12345);
        let client = github_client
            .installation_by_id(installation_id)
            .await
            .unwrap();

        let response = client.get("test").await;

        // Should succeed after multiple retries
        assert!(response.is_ok());
        assert_eq!(attempt_counter.load(Ordering::SeqCst), 5);
    }

    /// Verify concurrent requests from same installation don't interfere.
    ///
    /// From spec: Thread-safe rate limit tracking.
    #[tokio::test]
    async fn test_concurrent_requests_same_installation() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let reset_time = Utc::now() + Duration::hours(1);

        Mock::given(method("GET"))
            .and(path("/test"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": true}))
                    .insert_header("X-RateLimit-Limit", "5000")
                    .insert_header("X-RateLimit-Remaining", "4995")
                    .insert_header("X-RateLimit-Reset", reset_time.timestamp().to_string()),
            )
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

        // Make 5 concurrent requests
        let mut handles = vec![];
        for _ in 0..5 {
            let client_clone = client.clone();
            let handle = tokio::spawn(async move { client_clone.get("test").await });
            handles.push(handle);
        }

        // All should succeed
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
    }
}
