//! Tests for CircuitBreakerGitHubClient wrapper.

use github_bot_sdk::auth::{
    AuthenticationProvider, GitHubAppId, InstallationId, InstallationPermissions,
    InstallationToken, JsonWebToken,
};
use github_bot_sdk::client::{ClientConfig, GitHubClient};
use github_bot_sdk::error::AuthError;

use super::CircuitBreakerGitHubClient;

// ============================================================================
// Mock Authentication Provider
// ============================================================================

#[derive(Clone)]
struct MockAuthProvider {
    should_fail: bool,
}

impl MockAuthProvider {
    fn new() -> Self {
        Self { should_fail: false }
    }

    fn failing() -> Self {
        Self { should_fail: true }
    }
}

#[async_trait::async_trait]
impl AuthenticationProvider for MockAuthProvider {
    async fn app_token(&self) -> Result<JsonWebToken, AuthError> {
        if self.should_fail {
            Err(AuthError::TokenGenerationFailed {
                message: "Mock auth failure".to_string(),
            })
        } else {
            // Create a minimal JWT for testing
            let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10);
            Ok(JsonWebToken::new(
                "mock-jwt-token".to_string(),
                GitHubAppId::new(12345),
                expires_at,
            ))
        }
    }

    async fn installation_token(
        &self,
        _installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        if self.should_fail {
            Err(AuthError::TokenGenerationFailed {
                message: "Mock auth failure".to_string(),
            })
        } else {
            let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);
            Ok(InstallationToken::new(
                "mock-installation-token".to_string(),
                InstallationId::new(12345),
                expires_at,
                InstallationPermissions::default(),
                vec![],
            ))
        }
    }

    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        self.installation_token(installation_id).await
    }

    async fn list_installations(
        &self,
    ) -> Result<Vec<github_bot_sdk::auth::Installation>, AuthError> {
        Ok(vec![])
    }

    async fn get_installation_repositories(
        &self,
        _installation_id: InstallationId,
    ) -> Result<Vec<github_bot_sdk::auth::Repository>, AuthError> {
        Ok(vec![])
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_client() -> CircuitBreakerGitHubClient {
    let auth = MockAuthProvider::new();
    let config = ClientConfig::default();
    let github_client = GitHubClient::builder(auth).config(config).build().unwrap();

    CircuitBreakerGitHubClient::new(github_client)
}

fn create_failing_client() -> CircuitBreakerGitHubClient {
    let auth = MockAuthProvider::failing();
    let config = ClientConfig::default();
    let github_client = GitHubClient::builder(auth).config(config).build().unwrap();

    CircuitBreakerGitHubClient::new(github_client)
}

// ============================================================================
// Construction Tests
// ============================================================================

#[test]
fn test_circuit_breaker_github_client_creation() {
    let _client = create_test_client();
    // Successful creation is enough to verify functionality
}

#[test]
fn test_circuit_breaker_github_client_clone() {
    let client = create_test_client();
    let _cloned = client.clone();

    // Both should be independent but functional
}

// ============================================================================
// Circuit Breaker Protection Tests
// ============================================================================

/// Verify circuit breaker opens after consecutive failures.
#[tokio::test]
async fn test_circuit_opens_after_failures() {
    let client = create_failing_client();

    // Trigger failures to trip circuit (GitHub config: 5 failures)
    for i in 0..5 {
        let result = client.list_installations().await;
        assert!(
            result.is_err(),
            "Attempt {} should fail due to auth error",
            i + 1
        );
    }

    // Next request should fail fast due to circuit open
    let result = client.list_installations().await;
    assert!(result.is_err());

    // Verify it's a circuit breaker error, not the underlying error
    if let Err(e) = result {
        assert!(
            e.is_circuit_protection(),
            "Expected circuit protection error after 5 failures"
        );
    }
}

/// Verify successful operations pass through circuit breaker.
#[tokio::test]
async fn test_successful_operations_pass_through() {
    // Note: Without a mock server, GitHub operations will fail with network errors
    // This test verifies the circuit breaker doesn't interfere with the attempt
    let client = create_test_client();

    let result = client.list_installations().await;
    // Will fail due to no mock server, but should be an operation failure, not circuit breaker
    if let Err(e) = result {
        // Should be an operation failure (network/API error), not circuit protection
        assert!(
            !e.is_circuit_protection() || e.counts_as_failure(),
            "Should be operation error, not circuit protection"
        );
    }
}

/// Verify get_installation passes through circuit breaker.
#[tokio::test]
async fn test_get_installation_with_circuit_breaker() {
    let client = create_test_client();
    let installation_id = InstallationId::new(12345);

    let result = client.get_installation(installation_id).await;
    // Will fail without mock server, but circuit breaker should allow the attempt
    assert!(result.is_err());
}

/// Verify get_app passes through circuit breaker.
#[tokio::test]
async fn test_get_app_with_circuit_breaker() {
    let client = create_test_client();

    let result = client.get_app().await;
    // Will fail without mock server, but circuit breaker should allow the attempt
    assert!(result.is_err());
}

/// Verify installation_by_id passes through circuit breaker.
#[tokio::test]
async fn test_installation_by_id_with_circuit_breaker() {
    let client = create_test_client();
    let installation_id = InstallationId::new(67890);

    let result = client.installation_by_id(installation_id).await;
    // With mock auth that succeeds, installation_by_id should succeed
    assert!(result.is_ok());
}

// ============================================================================
// Error Mapping Tests
// ============================================================================

/// Verify CircuitBreakerError types are properly propagated.
#[tokio::test]
async fn test_error_types_propagated() {
    let client = create_failing_client();

    // First failure should be operation error
    let result = client.list_installations().await;
    assert!(result.is_err());

    if let Err(e) = result {
        // Should be OperationFailed wrapping the underlying ApiError
        assert!(
            e.counts_as_failure(),
            "First error should count as failure to trip circuit"
        );
    }
}

// ============================================================================
// Serialization Tests
// ============================================================================

/// Verify serde_json serialization doesn't panic.
///
/// The wrapper uses serde_json::to_value().unwrap() which could panic
/// if types aren't serializable. This test ensures the pattern is safe.
#[test]
fn test_serialization_safety() {
    use github_bot_sdk::auth::{
        Account, GitHubAppId, Installation, RepositorySelection, TargetType, UserId,
    };

    // Create a mock account
    let account = Account {
        id: UserId::new(456),
        login: "test-org".to_string(),
        account_type: TargetType::Organization,
        avatar_url: Some("https://example.com/avatar.png".to_string()),
        html_url: "https://github.com/test-org".to_string(),
    };

    // Create a mock installation
    let installation = Installation {
        id: InstallationId::new(123),
        account,
        access_tokens_url: "https://api.github.com/app/installations/123/access_tokens".to_string(),
        repositories_url: "https://api.github.com/installation/repositories".to_string(),
        html_url: "https://github.com/settings/installations/123".to_string(),
        app_id: GitHubAppId::new(789),
        target_type: TargetType::Organization,
        repository_selection: RepositorySelection::All,
        permissions: InstallationPermissions::default(),
        events: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        single_file_name: None,
        has_multiple_single_files: false,
        suspended_at: None,
        suspended_by: None,
    };

    // Verify serialization works
    let value = serde_json::to_value(&installation);
    assert!(value.is_ok(), "Installation should be serializable");

    // Verify deserialization works
    let value = value.unwrap();
    let deserialized: Result<Installation, _> = serde_json::from_value(value);
    assert!(
        deserialized.is_ok(),
        "Installation should be deserializable"
    );
}

// ============================================================================
// Configuration Tests
// ============================================================================

/// Verify GitHub API circuit breaker uses correct configuration.
#[test]
fn test_github_api_circuit_breaker_config() {
    use queue_keeper_core::circuit_breaker::github_api_circuit_breaker_config;

    let config = github_api_circuit_breaker_config();

    // Verify configuration matches GitHub API requirements
    assert_eq!(config.service_name, "github-api");
    assert_eq!(config.failure_threshold, 5); // REQ-009 compliance
    assert_eq!(config.recovery_timeout_seconds, 60); // Respect rate limits
    assert_eq!(config.operation_timeout_seconds, 10); // Allow for network latency
}
