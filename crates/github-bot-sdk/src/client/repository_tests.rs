//! Tests for Repository Operations
//!
//! **Specification**: `github-bot-sdk-specs/interfaces/repository-operations.md`

use super::*;
use crate::auth::{
    AuthenticationProvider, InstallationId, InstallationPermissions, InstallationToken,
    JsonWebToken,
};
use crate::client::{ClientConfig, GitHubClient};
use crate::error::{ApiError, AuthError};
use chrono::{Duration, Utc};
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
}

#[async_trait::async_trait]
impl AuthenticationProvider for MockAuthProvider {
    async fn app_token(&self) -> Result<JsonWebToken, AuthError> {
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
        self.installation_token(installation_id).await
    }

    async fn list_installations(&self) -> Result<Vec<crate::auth::Installation>, AuthError> {
        Err(AuthError::TokenGenerationFailed {
            message: "Not implemented for mock".to_string(),
        })
    }

    async fn get_installation_repositories(
        &self,
        _installation_id: InstallationId,
    ) -> Result<Vec<crate::auth::Repository>, AuthError> {
        Err(AuthError::TokenGenerationFailed {
            message: "Not implemented for mock".to_string(),
        })
    }
}

mod repository_operations_tests {
    use super::*;

    /// Verify get_repository returns repository metadata.
    ///
    /// Assertion #8: Repository Information Retrieval returns complete metadata.
    #[tokio::test]
    async fn test_get_repository() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let repo_json = serde_json::json!({
            "id": 1296269,
            "name": "Hello-World",
            "full_name": "octocat/Hello-World",
            "owner": {
                "login": "octocat",
                "id": 1,
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "type": "User"
            },
            "description": "This your first repo!",
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/octocat/Hello-World",
            "clone_url": "https://github.com/octocat/Hello-World.git",
            "ssh_url": "git@github.com:octocat/Hello-World.git",
            "created_at": "2011-01-26T19:01:12Z",
            "updated_at": "2011-01-26T19:14:43Z"
        });

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(repo_json))
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

        let result = client.get_repository("octocat", "Hello-World").await;

        assert!(result.is_ok());
        let repo = result.unwrap();
        assert_eq!(repo.id, 1296269);
        assert_eq!(repo.name, "Hello-World");
        assert_eq!(repo.full_name, "octocat/Hello-World");
        assert_eq!(repo.owner.login, "octocat");
        assert_eq!(repo.default_branch, "main");
        assert_eq!(repo.description, Some("This your first repo!".to_string()));
    }

    /// Verify get_repository returns NotFound for missing repo.
    ///
    /// From GitHub API: 404 indicates repository not found.
    #[tokio::test]
    async fn test_get_repository_not_found() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/NonExistent"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found",
                "documentation_url": "https://docs.github.com/rest/repos/repos#get-a-repository"
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

        let result = client.get_repository("octocat", "NonExistent").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::NotFound));
    }

    /// Verify get_repository returns AuthorizationFailed for inaccessible repo.
    ///
    /// Assertion #9: Repository Access Without Permission returns proper error.
    /// From GitHub API: 403 indicates insufficient permissions.
    #[tokio::test]
    async fn test_get_repository_permission_denied() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/private-org/secret-repo"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "message": "Resource not accessible by integration",
                "documentation_url": "https://docs.github.com/rest/repos/repos#get-a-repository"
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

        let result = client.get_repository("private-org", "secret-repo").await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::AuthorizationFailed));
    }
}

mod branch_operations_tests {
    use super::*;

    /// Verify list_branches returns all repository branches.
    ///
    /// From GitHub API: GET /repos/{owner}/{repo}/branches returns array of branches.
    #[tokio::test]
    async fn test_list_branches() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let branches_json = serde_json::json!([
            {
                "name": "main",
                "commit": {
                    "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
                    "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b"
                },
                "protected": true
            },
            {
                "name": "development",
                "commit": {
                    "sha": "7dcb09b5b57875f334f61aebed695e2e4193db5e",
                    "url": "https://api.github.com/repos/octocat/Hello-World/commits/7dcb09b"
                },
                "protected": false
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/branches"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(branches_json))
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

        let result = client.list_branches("octocat", "Hello-World").await;

        assert!(result.is_ok());
        let branches = result.unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].name, "main");
        assert!(branches[0].protected);
        assert_eq!(branches[1].name, "development");
        assert!(!branches[1].protected);
    }

    /// Verify get_branch returns specific branch information.
    ///
    /// From GitHub API: GET /repos/{owner}/{repo}/branches/{branch} returns branch details.
    #[tokio::test]
    async fn test_get_branch() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let branch_json = serde_json::json!({
            "name": "main",
            "commit": {
                "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
                "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b"
            },
            "protected": true
        });

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/branches/main"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(branch_json))
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

        let result = client.get_branch("octocat", "Hello-World", "main").await;

        assert!(result.is_ok());
        let branch = result.unwrap();
        assert_eq!(branch.name, "main");
        assert_eq!(
            branch.commit.sha,
            "6dcb09b5b57875f334f61aebed695e2e4193db5e"
        );
        assert!(branch.protected);
    }

    /// Verify get_branch returns NotFound for missing branch.
    ///
    /// From GitHub API: 404 indicates branch does not exist.
    #[tokio::test]
    async fn test_get_branch_not_found() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/branches/nonexistent"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Branch not found"
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

        let result = client
            .get_branch("octocat", "Hello-World", "nonexistent")
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::NotFound));
    }
}

mod git_ref_operations_tests {
    use super::*;

    /// Verify get_git_ref returns reference information.
    ///
    /// From GitHub API: GET /repos/{owner}/{repo}/git/refs/{ref} returns ref details.
    #[tokio::test]
    async fn test_get_git_ref() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let ref_json = serde_json::json!({
            "ref": "refs/heads/feature-a",
            "node_id": "MDM6UmVmcmVmcy9oZWFkcy9mZWF0dXJlLWE=",
            "url": "https://api.github.com/repos/octocat/Hello-World/git/refs/heads/feature-a",
            "object": {
                "sha": "aa218f56b14c9653891f9e74264a383fa43fefbd",
                "type": "commit",
                "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/aa218f5"
            }
        });

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/git/refs/heads/feature-a"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(200).set_body_json(ref_json))
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

        let result = client
            .get_git_ref("octocat", "Hello-World", "heads/feature-a")
            .await;

        assert!(result.is_ok());
        let git_ref = result.unwrap();
        assert_eq!(git_ref.ref_name, "refs/heads/feature-a");
        assert_eq!(
            git_ref.object.sha,
            "aa218f56b14c9653891f9e74264a383fa43fefbd"
        );
    }

    /// Verify create_git_ref creates new reference.
    ///
    /// From GitHub API: POST /repos/{owner}/{repo}/git/refs creates a new ref.
    #[tokio::test]
    async fn test_create_git_ref() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let created_ref_json = serde_json::json!({
            "ref": "refs/heads/new-feature",
            "node_id": "MDM6UmVmcmVmcy9oZWFkcy9uZXctZmVhdHVyZQ==",
            "url": "https://api.github.com/repos/octocat/Hello-World/git/refs/heads/new-feature",
            "object": {
                "sha": "aa218f56b14c9653891f9e74264a383fa43fefbd",
                "type": "commit",
                "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/aa218f5"
            }
        });

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/git/refs"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(201).set_body_json(created_ref_json))
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

        let result = client
            .create_git_ref(
                "octocat",
                "Hello-World",
                "refs/heads/new-feature",
                "aa218f56b14c9653891f9e74264a383fa43fefbd",
            )
            .await;

        assert!(result.is_ok());
        let git_ref = result.unwrap();
        assert_eq!(git_ref.ref_name, "refs/heads/new-feature");
    }

    /// Verify create_git_ref returns error when ref exists.
    ///
    /// From GitHub API: 422 indicates validation failed (ref already exists).
    #[tokio::test]
    async fn test_create_git_ref_already_exists() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/git/refs"))
            .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
                "message": "Reference already exists"
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

        let result = client
            .create_git_ref(
                "octocat",
                "Hello-World",
                "refs/heads/main",
                "aa218f56b14c9653891f9e74264a383fa43fefbd",
            )
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::InvalidRequest { .. }));
    }

    /// Verify update_git_ref updates reference (fast-forward).
    ///
    /// From GitHub API: PATCH /repos/{owner}/{repo}/git/refs/{ref} updates ref.
    #[tokio::test]
    async fn test_update_git_ref_fast_forward() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_ref_json = serde_json::json!({
            "ref": "refs/heads/feature-a",
            "node_id": "MDM6UmVmcmVmcy9oZWFkcy9mZWF0dXJlLWE=",
            "url": "https://api.github.com/repos/octocat/Hello-World/git/refs/heads/feature-a",
            "object": {
                "sha": "bb218f56b14c9653891f9e74264a383fa43fefbd",
                "type": "commit",
                "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/bb218f5"
            }
        });

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/git/refs/heads/feature-a"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_ref_json))
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

        let result = client
            .update_git_ref(
                "octocat",
                "Hello-World",
                "heads/feature-a",
                "bb218f56b14c9653891f9e74264a383fa43fefbd",
                false,
            )
            .await;

        assert!(result.is_ok());
        let git_ref = result.unwrap();
        assert_eq!(
            git_ref.object.sha,
            "bb218f56b14c9653891f9e74264a383fa43fefbd"
        );
    }

    /// Verify update_git_ref with force flag allows non-fast-forward.
    ///
    /// From GitHub API: force=true in PATCH allows non-fast-forward updates.
    #[tokio::test]
    async fn test_update_git_ref_force() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_ref_json = serde_json::json!({
            "ref": "refs/heads/feature-a",
            "node_id": "MDM6UmVmcmVmcy9oZWFkcy9mZWF0dXJlLWE=",
            "url": "https://api.github.com/repos/octocat/Hello-World/git/refs/heads/feature-a",
            "object": {
                "sha": "cc218f56b14c9653891f9e74264a383fa43fefbd",
                "type": "commit",
                "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/cc218f5"
            }
        });

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/git/refs/heads/feature-a"))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_ref_json))
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

        let result = client
            .update_git_ref(
                "octocat",
                "Hello-World",
                "heads/feature-a",
                "cc218f56b14c9653891f9e74264a383fa43fefbd",
                true,
            )
            .await;

        assert!(result.is_ok());
        let git_ref = result.unwrap();
        assert_eq!(
            git_ref.object.sha,
            "cc218f56b14c9653891f9e74264a383fa43fefbd"
        );
    }

    /// Verify delete_git_ref deletes reference.
    ///
    /// From GitHub API: DELETE /repos/{owner}/{repo}/git/refs/{ref} deletes ref.
    #[tokio::test]
    async fn test_delete_git_ref() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("DELETE"))
            .and(path("/repos/octocat/Hello-World/git/refs/heads/feature-a"))
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

        let result = client
            .delete_git_ref("octocat", "Hello-World", "heads/feature-a")
            .await;

        assert!(result.is_ok());
    }

    /// Verify delete_git_ref returns NotFound for missing ref.
    ///
    /// From GitHub API: 404 indicates ref does not exist.
    #[tokio::test]
    async fn test_delete_git_ref_not_found() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("DELETE"))
            .and(path(
                "/repos/octocat/Hello-World/git/refs/heads/nonexistent",
            ))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "message": "Not Found"
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

        let result = client
            .delete_git_ref("octocat", "Hello-World", "heads/nonexistent")
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ApiError::NotFound));
    }
}

mod tag_operations_tests {
    use super::*;

    /// Verify list_tags returns all repository tags.
    ///
    /// From GitHub API: GET /repos/{owner}/{repo}/tags returns array of tags.
    #[tokio::test]
    async fn test_list_tags() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let tags_json = serde_json::json!([
            {
                "name": "v1.0.0",
                "commit": {
                    "sha": "c5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc",
                    "url": "https://api.github.com/repos/octocat/Hello-World/commits/c5b97d5"
                },
                "zipball_url": "https://github.com/octocat/Hello-World/zipball/v1.0.0",
                "tarball_url": "https://github.com/octocat/Hello-World/tarball/v1.0.0"
            },
            {
                "name": "v0.1.0",
                "commit": {
                    "sha": "b5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc",
                    "url": "https://api.github.com/repos/octocat/Hello-World/commits/b5b97d5"
                },
                "zipball_url": "https://github.com/octocat/Hello-World/zipball/v0.1.0",
                "tarball_url": "https://github.com/octocat/Hello-World/tarball/v0.1.0"
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/tags"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .and(header("Accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(tags_json))
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

        let result = client.list_tags("octocat", "Hello-World").await;

        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "v1.0.0");
        assert_eq!(tags[1].name, "v0.1.0");
        assert_eq!(
            tags[0].commit.sha,
            "c5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc"
        );
    }

    /// Verify list_tags returns empty vector for repo with no tags.
    ///
    /// From GitHub API: Empty array returned when repository has no tags.
    #[tokio::test]
    async fn test_list_tags_empty_repo() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Empty-Repo/tags"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
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

        let result = client.list_tags("octocat", "Empty-Repo").await;

        assert!(result.is_ok());
        let tags = result.unwrap();
        assert_eq!(tags.len(), 0);
    }
}

mod type_serialization_tests {
    use super::*;

    /// Verify Repository deserializes from GitHub API JSON.
    ///
    /// From GitHub API docs: GET /repos/{owner}/{repo} returns repository metadata.
    #[test]
    fn test_repository_deserialization() {
        let json = r#"{
            "id": 1296269,
            "name": "Hello-World",
            "full_name": "octocat/Hello-World",
            "owner": {
                "login": "octocat",
                "id": 1,
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "type": "User"
            },
            "description": "This your first repo!",
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/octocat/Hello-World",
            "clone_url": "https://github.com/octocat/Hello-World.git",
            "ssh_url": "git@github.com:octocat/Hello-World.git",
            "created_at": "2011-01-26T19:01:12Z",
            "updated_at": "2011-01-26T19:14:43Z"
        }"#;

        let repo: Repository = serde_json::from_str(json).unwrap();

        assert_eq!(repo.id, 1296269);
        assert_eq!(repo.name, "Hello-World");
        assert_eq!(repo.full_name, "octocat/Hello-World");
        assert_eq!(repo.owner.login, "octocat");
        assert_eq!(repo.owner.id, 1);
        assert_eq!(repo.description, Some("This your first repo!".to_string()));
        assert!(!repo.private);
        assert_eq!(repo.default_branch, "main");
    }

    /// Verify Repository deserializes with optional description as None.
    #[test]
    fn test_repository_deserialization_without_description() {
        let json = r#"{
            "id": 1296269,
            "name": "Hello-World",
            "full_name": "octocat/Hello-World",
            "owner": {
                "login": "octocat",
                "id": 1,
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "type": "User"
            },
            "description": null,
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/octocat/Hello-World",
            "clone_url": "https://github.com/octocat/Hello-World.git",
            "ssh_url": "git@github.com:octocat/Hello-World.git",
            "created_at": "2011-01-26T19:01:12Z",
            "updated_at": "2011-01-26T19:14:43Z"
        }"#;

        let repo: Repository = serde_json::from_str(json).unwrap();
        assert_eq!(repo.description, None);
    }

    /// Verify Branch deserializes from GitHub API JSON.
    ///
    /// From GitHub API docs: GET /repos/{owner}/{repo}/branches returns branch list.
    #[test]
    fn test_branch_deserialization() {
        let json = r#"{
            "name": "main",
            "commit": {
                "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
                "url": "https://api.github.com/repos/octocat/Hello-World/commits/6dcb09b"
            },
            "protected": true
        }"#;

        let branch: Branch = serde_json::from_str(json).unwrap();

        assert_eq!(branch.name, "main");
        assert_eq!(
            branch.commit.sha,
            "6dcb09b5b57875f334f61aebed695e2e4193db5e"
        );
        assert!(branch.protected);
    }

    /// Verify GitRef deserializes from GitHub API JSON.
    ///
    /// From GitHub API docs: GET /repos/{owner}/{repo}/git/refs/{ref} returns reference info.
    #[test]
    fn test_git_ref_deserialization() {
        let json = r#"{
            "ref": "refs/heads/feature-a",
            "node_id": "MDM6UmVmcmVmcy9oZWFkcy9mZWF0dXJlLWE=",
            "url": "https://api.github.com/repos/octocat/Hello-World/git/refs/heads/feature-a",
            "object": {
                "sha": "aa218f56b14c9653891f9e74264a383fa43fefbd",
                "type": "commit",
                "url": "https://api.github.com/repos/octocat/Hello-World/git/commits/aa218f5"
            }
        }"#;

        let git_ref: GitRef = serde_json::from_str(json).unwrap();

        assert_eq!(git_ref.ref_name, "refs/heads/feature-a");
        assert_eq!(git_ref.node_id, "MDM6UmVmcmVmcy9oZWFkcy9mZWF0dXJlLWE=");
        assert_eq!(
            git_ref.object.sha,
            "aa218f56b14c9653891f9e74264a383fa43fefbd"
        );
        assert!(matches!(git_ref.object.object_type, GitObjectType::Commit));
    }

    /// Verify Tag deserializes from GitHub API JSON.
    ///
    /// From GitHub API docs: GET /repos/{owner}/{repo}/tags returns tag list.
    #[test]
    fn test_tag_deserialization() {
        let json = r#"{
            "name": "v1.0.0",
            "commit": {
                "sha": "c5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc",
                "url": "https://api.github.com/repos/octocat/Hello-World/commits/c5b97d5"
            },
            "zipball_url": "https://github.com/octocat/Hello-World/zipball/v1.0.0",
            "tarball_url": "https://github.com/octocat/Hello-World/tarball/v1.0.0"
        }"#;

        let tag: Tag = serde_json::from_str(json).unwrap();

        assert_eq!(tag.name, "v1.0.0");
        assert_eq!(tag.commit.sha, "c5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc");
        assert!(tag.zipball_url.contains("zipball/v1.0.0"));
        assert!(tag.tarball_url.contains("tarball/v1.0.0"));
    }

    /// Verify GitObjectType serializes to lowercase strings.
    ///
    /// From GitHub API spec: object type field uses lowercase strings.
    #[test]
    fn test_git_object_type_serialization() {
        let commit_type = GitObjectType::Commit;
        let tree_type = GitObjectType::Tree;
        let blob_type = GitObjectType::Blob;
        let tag_type = GitObjectType::Tag;

        let commit_json = serde_json::to_string(&commit_type).unwrap();
        let tree_json = serde_json::to_string(&tree_type).unwrap();
        let blob_json = serde_json::to_string(&blob_type).unwrap();
        let tag_json = serde_json::to_string(&tag_type).unwrap();

        assert_eq!(commit_json, r#""commit""#);
        assert_eq!(tree_json, r#""tree""#);
        assert_eq!(blob_json, r#""blob""#);
        assert_eq!(tag_json, r#""tag""#);
    }

    /// Verify OwnerType serializes correctly for User and Organization.
    #[test]
    fn test_owner_type_serialization() {
        let user_type = OwnerType::User;
        let org_type = OwnerType::Organization;

        let user_json = serde_json::to_string(&user_type).unwrap();
        let org_json = serde_json::to_string(&org_type).unwrap();

        assert_eq!(user_json, r#""User""#);
        assert_eq!(org_json, r#""Organization""#);
    }
}
