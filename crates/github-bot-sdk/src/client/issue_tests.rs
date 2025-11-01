//! Tests for issue operations.

use super::*;
use crate::auth::{
    AuthenticationProvider, InstallationId, InstallationPermissions, InstallationToken,
    JsonWebToken,
};
use crate::client::{ClientConfig, GitHubClient};
use crate::error::{ApiError, AuthError};
use chrono::{Duration, Utc};
use std::sync::Arc;
use wiremock::matchers::{header, method, path, query_param};
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

mod construction {
    use super::*;

    /// Verify CreateIssueRequest with only required title field.
    #[test]
    fn test_create_issue_request_minimal() {
        let request = CreateIssueRequest {
            title: "Bug report".to_string(),
            body: None,
            assignees: None,
            milestone: None,
            labels: None,
        };

        assert_eq!(request.title, "Bug report");
        assert!(request.body.is_none());
        assert!(request.assignees.is_none());
    }

    /// Verify CreateIssueRequest with all optional fields populated.
    #[test]
    fn test_create_issue_request_full() {
        let request = CreateIssueRequest {
            title: "Bug report".to_string(),
            body: Some("Description of the bug".to_string()),
            assignees: Some(vec!["octocat".to_string()]),
            milestone: Some(1),
            labels: Some(vec!["bug".to_string()]),
        };

        assert_eq!(request.title, "Bug report");
        assert_eq!(request.body, Some("Description of the bug".to_string()));
        assert_eq!(request.assignees.as_ref().unwrap().len(), 1);
        assert_eq!(request.milestone, Some(1));
        assert_eq!(request.labels.as_ref().unwrap().len(), 1);
    }

    /// Verify UpdateIssueRequest with selective field updates.
    #[test]
    fn test_update_issue_request_partial() {
        let request = UpdateIssueRequest {
            title: Some("Updated title".to_string()),
            state: Some("closed".to_string()),
            ..Default::default()
        };

        assert_eq!(request.title, Some("Updated title".to_string()));
        assert_eq!(request.state, Some("closed".to_string()));
        assert!(request.body.is_none());
        assert!(request.assignees.is_none());
    }

    /// Verify CreateLabelRequest construction with all fields.
    #[test]
    fn test_create_label_request() {
        let request = CreateLabelRequest {
            name: "priority-high".to_string(),
            description: Some("High priority issue".to_string()),
            color: "ff0000".to_string(),
        };

        assert_eq!(request.name, "priority-high");
        assert_eq!(request.description, Some("High priority issue".to_string()));
        assert_eq!(request.color, "ff0000");
    }

    /// Verify CreateCommentRequest construction.
    #[test]
    fn test_create_comment_request() {
        let request = CreateCommentRequest {
            body: "This is a comment".to_string(),
        };

        assert_eq!(request.body, "This is a comment");
    }
}

mod issue_operations {
    use super::*;

    /// Verify list_issues returns all repository issues.
    ///
    /// Tests GET /repos/{owner}/{repo}/issues endpoint.
    #[tokio::test]
    async fn test_list_issues_all() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let issues_json = serde_json::json!([
            {
                "id": 1,
                "node_id": "MDU6SXNzdWUx",
                "number": 1347,
                "title": "Found a bug",
                "body": "Bug description",
                "state": "open",
                "user": {
                    "login": "octocat",
                    "id": 1,
                    "node_id": "MDQ6VXNlcjE=",
                    "type": "User"
                },
                "labels": [],
                "assignees": [],
                "milestone": null,
                "comments": 0,
                "created_at": "2011-04-22T13:33:48Z",
                "updated_at": "2011-04-22T13:33:48Z",
                "closed_at": null,
                "html_url": "https://github.com/octocat/Hello-World/issues/1347"
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/issues"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(200).set_body_json(issues_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client.list_issues("octocat", "Hello-World", None).await;

        assert!(result.is_ok());
        let issues = result.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].number, 1347);
        assert_eq!(issues[0].title, "Found a bug");
    }

    /// Verify list_issues can filter by state parameter.
    ///
    /// Tests GET /repos/{owner}/{repo}/issues?state=open endpoint.
    #[tokio::test]
    async fn test_list_issues_filtered_by_state() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/issues"))
            .and(query_param("state", "open"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .list_issues("octocat", "Hello-World", Some("open"))
            .await;

        assert!(result.is_ok());
    }

    /// Verify get_issue returns a specific issue by number.
    ///
    /// Tests GET /repos/{owner}/{repo}/issues/{number} endpoint.
    #[tokio::test]
    async fn test_get_issue_found() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let issue_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6SXNzdWUx",
            "number": 1347,
            "title": "Found a bug",
            "body": "Bug description",
            "state": "open",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [],
            "assignees": [],
            "milestone": null,
            "comments": 0,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-22T13:33:48Z",
            "closed_at": null,
            "html_url": "https://github.com/octocat/Hello-World/issues/1347"
        });

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/issues/1347"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client.get_issue("octocat", "Hello-World", 1347).await;

        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.number, 1347);
        assert_eq!(issue.title, "Found a bug");
    }

    /// Verify get_issue returns NotFound for non-existent issue.
    #[tokio::test]
    async fn test_get_issue_not_found() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/issues/9999"))
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

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client.get_issue("octocat", "Hello-World", 9999).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }

    /// Verify create_issue with minimal required fields (title only).
    ///
    /// Tests POST /repos/{owner}/{repo}/issues endpoint.
    #[tokio::test]
    async fn test_create_issue_minimal() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let created_issue_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6SXNzdWUx",
            "number": 1348,
            "title": "New bug",
            "body": null,
            "state": "open",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [],
            "assignees": [],
            "milestone": null,
            "comments": 0,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-22T13:33:48Z",
            "closed_at": null,
            "html_url": "https://github.com/octocat/Hello-World/issues/1348"
        });

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/issues"))
            .respond_with(ResponseTemplate::new(201).set_body_json(created_issue_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let request = CreateIssueRequest {
            title: "New bug".to_string(),
            body: None,
            assignees: None,
            milestone: None,
            labels: None,
        };

        let result = client.create_issue("octocat", "Hello-World", request).await;

        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.number, 1348);
        assert_eq!(issue.title, "New bug");
    }

    /// Verify create_issue with all optional fields populated.
    #[tokio::test]
    async fn test_create_issue_full() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let created_issue_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6SXNzdWUx",
            "number": 1349,
            "title": "Full bug report",
            "body": "Detailed description",
            "state": "open",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [{
                "id": 1,
                "node_id": "MDU6TGFiZWwx",
                "name": "bug",
                "description": null,
                "color": "ff0000",
                "default": true
            }],
            "assignees": [{
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            }],
            "milestone": null,
            "comments": 0,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-22T13:33:48Z",
            "closed_at": null,
            "html_url": "https://github.com/octocat/Hello-World/issues/1349"
        });

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/issues"))
            .respond_with(ResponseTemplate::new(201).set_body_json(created_issue_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let request = CreateIssueRequest {
            title: "Full bug report".to_string(),
            body: Some("Detailed description".to_string()),
            assignees: Some(vec!["octocat".to_string()]),
            milestone: Some(1),
            labels: Some(vec!["bug".to_string()]),
        };

        let result = client.create_issue("octocat", "Hello-World", request).await;

        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.number, 1349);
        assert_eq!(issue.labels.len(), 1);
        assert_eq!(issue.assignees.len(), 1);
    }

    /// Verify update_issue modifies issue fields.
    ///
    /// Tests PATCH /repos/{owner}/{repo}/issues/{number} endpoint.
    #[tokio::test]
    async fn test_update_issue() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_issue_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6SXNzdWUx",
            "number": 1347,
            "title": "Updated title",
            "body": "Updated body",
            "state": "closed",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [],
            "assignees": [],
            "milestone": null,
            "comments": 0,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-22T13:34:00Z",
            "closed_at": "2011-04-22T13:34:00Z",
            "html_url": "https://github.com/octocat/Hello-World/issues/1347"
        });

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/issues/1347"))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_issue_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let request = UpdateIssueRequest {
            title: Some("Updated title".to_string()),
            body: Some("Updated body".to_string()),
            state: Some("closed".to_string()),
            ..Default::default()
        };

        let result = client
            .update_issue("octocat", "Hello-World", 1347, request)
            .await;

        assert!(result.is_ok());
        let issue = result.unwrap();
        assert_eq!(issue.title, "Updated title");
        assert_eq!(issue.state, "closed");
    }

    /// Verify set_issue_milestone sets a milestone on an issue.
    ///
    /// Tests PATCH /repos/{owner}/{repo}/issues/{number} with milestone field.
    #[tokio::test]
    async fn test_set_issue_milestone() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_issue_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6SXNzdWUx",
            "number": 1347,
            "title": "Bug with milestone",
            "body": null,
            "state": "open",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [],
            "assignees": [],
            "milestone": {
                "id": 1,
                "node_id": "MDk6TWlsZXN0b25lMQ==",
                "number": 1,
                "title": "v1.0",
                "description": null,
                "state": "open",
                "open_issues": 1,
                "closed_issues": 0,
                "due_on": null,
                "created_at": "2011-04-10T20:09:31Z",
                "updated_at": "2011-04-10T20:09:31Z",
                "closed_at": null
            },
            "comments": 0,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-22T13:33:48Z",
            "closed_at": null,
            "html_url": "https://github.com/octocat/Hello-World/issues/1347"
        });

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/issues/1347"))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_issue_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .set_issue_milestone("octocat", "Hello-World", 1347, Some(1))
            .await;

        assert!(result.is_ok());
        let issue = result.unwrap();
        assert!(issue.milestone.is_some());
        assert_eq!(issue.milestone.unwrap().number, 1);
    }

    /// Verify set_issue_milestone with None clears the milestone.
    ///
    /// Tests PATCH /repos/{owner}/{repo}/issues/{number} with milestone=null.
    #[tokio::test]
    async fn test_clear_issue_milestone() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_issue_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6SXNzdWUx",
            "number": 1347,
            "title": "Bug without milestone",
            "body": null,
            "state": "open",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [],
            "assignees": [],
            "milestone": null,
            "comments": 0,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-22T13:33:48Z",
            "closed_at": null,
            "html_url": "https://github.com/octocat/Hello-World/issues/1347"
        });

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/issues/1347"))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_issue_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .set_issue_milestone("octocat", "Hello-World", 1347, None)
            .await;

        assert!(result.is_ok());
        let issue = result.unwrap();
        assert!(issue.milestone.is_none());
    }
}

mod label_operations {
    use super::*;

    /// Verify list_labels returns all repository labels.
    ///
    /// Tests GET /repos/{owner}/{repo}/labels endpoint.
    #[tokio::test]
    async fn test_list_labels() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let labels_json = serde_json::json!([
            {
                "id": 1,
                "node_id": "MDU6TGFiZWwx",
                "name": "bug",
                "description": "Something isn't working",
                "color": "d73a4a",
                "default": true
            },
            {
                "id": 2,
                "node_id": "MDU6TGFiZWwy",
                "name": "enhancement",
                "description": "New feature or request",
                "color": "a2eeef",
                "default": true
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/labels"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(200).set_body_json(labels_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client.list_labels("octocat", "Hello-World").await;

        assert!(result.is_ok());
        let labels = result.unwrap();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].name, "bug");
        assert_eq!(labels[1].name, "enhancement");
    }

    /// Verify get_label returns a specific label by name.
    ///
    /// Tests GET /repos/{owner}/{repo}/labels/{name} endpoint.
    #[tokio::test]
    async fn test_get_label() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let label_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6TGFiZWwx",
            "name": "bug",
            "description": "Something isn't working",
            "color": "d73a4a",
            "default": true
        });

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/labels/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(label_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client.get_label("octocat", "Hello-World", "bug").await;

        assert!(result.is_ok());
        let label = result.unwrap();
        assert_eq!(label.name, "bug");
        assert_eq!(label.color, "d73a4a");
    }

    /// Verify get_label returns NotFound for non-existent label.
    #[tokio::test]
    async fn test_get_label_not_found() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/labels/nonexistent"))
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

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .get_label("octocat", "Hello-World", "nonexistent")
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }

    /// Verify create_label creates a new label.
    ///
    /// Tests POST /repos/{owner}/{repo}/labels endpoint.
    #[tokio::test]
    async fn test_create_label() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let created_label_json = serde_json::json!({
            "id": 3,
            "node_id": "MDU6TGFiZWwz",
            "name": "priority-high",
            "description": "High priority issue",
            "color": "ff0000",
            "default": false
        });

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/labels"))
            .respond_with(ResponseTemplate::new(201).set_body_json(created_label_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let request = CreateLabelRequest {
            name: "priority-high".to_string(),
            description: Some("High priority issue".to_string()),
            color: "ff0000".to_string(),
        };

        let result = client.create_label("octocat", "Hello-World", request).await;

        assert!(result.is_ok());
        let label = result.unwrap();
        assert_eq!(label.name, "priority-high");
        assert_eq!(label.color, "ff0000");
    }

    /// Verify update_label modifies an existing label.
    ///
    /// Tests PATCH /repos/{owner}/{repo}/labels/{name} endpoint.
    #[tokio::test]
    async fn test_update_label() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_label_json = serde_json::json!({
            "id": 1,
            "node_id": "MDU6TGFiZWwx",
            "name": "bug-critical",
            "description": "Critical bug requiring immediate attention",
            "color": "ff0000",
            "default": false
        });

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/labels/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_label_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let request = UpdateLabelRequest {
            new_name: Some("bug-critical".to_string()),
            description: Some("Critical bug requiring immediate attention".to_string()),
            color: Some("ff0000".to_string()),
        };

        let result = client
            .update_label("octocat", "Hello-World", "bug", request)
            .await;

        assert!(result.is_ok());
        let label = result.unwrap();
        assert_eq!(label.name, "bug-critical");
        assert_eq!(label.color, "ff0000");
    }

    /// Verify delete_label removes a label.
    ///
    /// Tests DELETE /repos/{owner}/{repo}/labels/{name} endpoint.
    #[tokio::test]
    async fn test_delete_label() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("DELETE"))
            .and(path("/repos/octocat/Hello-World/labels/deprecated"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .delete_label("octocat", "Hello-World", "deprecated")
            .await;

        assert!(result.is_ok());
    }

    /// Verify add_labels_to_issue adds labels to an issue.
    ///
    /// Tests POST /repos/{owner}/{repo}/issues/{number}/labels endpoint.
    #[tokio::test]
    async fn test_add_labels_to_issue() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_labels_json = serde_json::json!([
            {
                "id": 1,
                "node_id": "MDU6TGFiZWwx",
                "name": "bug",
                "description": "Something isn't working",
                "color": "d73a4a",
                "default": true
            },
            {
                "id": 2,
                "node_id": "MDU6TGFiZWwy",
                "name": "high-priority",
                "description": "High priority",
                "color": "ff0000",
                "default": false
            }
        ]);

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/issues/1347/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_labels_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let labels = vec!["bug".to_string(), "high-priority".to_string()];

        let result = client
            .add_labels_to_issue("octocat", "Hello-World", 1347, labels)
            .await;

        assert!(result.is_ok());
        let updated_labels = result.unwrap();
        assert_eq!(updated_labels.len(), 2);
        assert_eq!(updated_labels[0].name, "bug");
        assert_eq!(updated_labels[1].name, "high-priority");
    }

    /// Verify remove_label_from_issue removes a specific label from an issue.
    ///
    /// Tests DELETE /repos/{owner}/{repo}/issues/{number}/labels/{name} endpoint.
    #[tokio::test]
    async fn test_remove_label_from_issue() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let remaining_labels_json = serde_json::json!([
            {
                "id": 1,
                "node_id": "MDU6TGFiZWwx",
                "name": "bug",
                "description": "Something isn't working",
                "color": "d73a4a",
                "default": true
            }
        ]);

        Mock::given(method("DELETE"))
            .and(path(
                "/repos/octocat/Hello-World/issues/1347/labels/high-priority",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(remaining_labels_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .remove_label_from_issue("octocat", "Hello-World", 1347, "high-priority")
            .await;

        assert!(result.is_ok());
        let remaining_labels = result.unwrap();
        assert_eq!(remaining_labels.len(), 1);
        assert_eq!(remaining_labels[0].name, "bug");
    }
}

mod comment_operations {
    use super::*;

    /// Verify list_issue_comments returns all comments on an issue.
    ///
    /// Tests GET /repos/{owner}/{repo}/issues/{number}/comments endpoint.
    #[tokio::test]
    async fn test_list_issue_comments() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let comments_json = serde_json::json!([
            {
                "id": 1,
                "node_id": "MDEyOklzc3VlQ29tbWVudDE=",
                "body": "Great idea!",
                "user": {
                    "login": "octocat",
                    "id": 1,
                    "node_id": "MDQ6VXNlcjE=",
                    "type": "User"
                },
                "created_at": "2011-04-14T16:00:49Z",
                "updated_at": "2011-04-14T16:00:49Z",
                "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-1"
            },
            {
                "id": 2,
                "node_id": "MDEyOklzc3VlQ29tbWVudDI=",
                "body": "I agree!",
                "user": {
                    "login": "hubot",
                    "id": 2,
                    "node_id": "MDQ6VXNlcjI=",
                    "type": "Bot"
                },
                "created_at": "2011-04-14T17:00:49Z",
                "updated_at": "2011-04-14T17:00:49Z",
                "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-2"
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/issues/1347/comments"))
            .and(header("Authorization", format!("Bearer {}", test_token)))
            .respond_with(ResponseTemplate::new(200).set_body_json(comments_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .list_issue_comments("octocat", "Hello-World", 1347)
            .await;

        assert!(result.is_ok());
        let comments = result.unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].id, 1);
        assert_eq!(comments[0].body, "Great idea!");
        assert_eq!(comments[1].id, 2);
    }

    /// Verify get_issue_comment returns a specific comment by ID.
    ///
    /// Tests GET /repos/{owner}/{repo}/issues/comments/{id} endpoint.
    #[tokio::test]
    async fn test_get_issue_comment() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let comment_json = serde_json::json!({
            "id": 1,
            "node_id": "MDEyOklzc3VlQ29tbWVudDE=",
            "body": "Great idea!",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "created_at": "2011-04-14T16:00:49Z",
            "updated_at": "2011-04-14T16:00:49Z",
            "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-1"
        });

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/issues/comments/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(comment_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client.get_issue_comment("octocat", "Hello-World", 1).await;

        assert!(result.is_ok());
        let comment = result.unwrap();
        assert_eq!(comment.id, 1);
        assert_eq!(comment.body, "Great idea!");
    }

    /// Verify get_issue_comment returns NotFound for non-existent comment.
    #[tokio::test]
    async fn test_get_issue_comment_not_found() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("GET"))
            .and(path("/repos/octocat/Hello-World/issues/comments/9999"))
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

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .get_issue_comment("octocat", "Hello-World", 9999)
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApiError::NotFound));
    }

    /// Verify create_issue_comment creates a new comment.
    ///
    /// Tests POST /repos/{owner}/{repo}/issues/{number}/comments endpoint.
    #[tokio::test]
    async fn test_create_issue_comment() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let created_comment_json = serde_json::json!({
            "id": 3,
            "node_id": "MDEyOklzc3VlQ29tbWVudDM=",
            "body": "This is a new comment",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "created_at": "2011-04-14T18:00:49Z",
            "updated_at": "2011-04-14T18:00:49Z",
            "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-3"
        });

        Mock::given(method("POST"))
            .and(path("/repos/octocat/Hello-World/issues/1347/comments"))
            .respond_with(ResponseTemplate::new(201).set_body_json(created_comment_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let request = CreateCommentRequest {
            body: "This is a new comment".to_string(),
        };

        let result = client
            .create_issue_comment("octocat", "Hello-World", 1347, request)
            .await;

        assert!(result.is_ok());
        let comment = result.unwrap();
        assert_eq!(comment.id, 3);
        assert_eq!(comment.body, "This is a new comment");
    }

    /// Verify update_issue_comment modifies an existing comment.
    ///
    /// Tests PATCH /repos/{owner}/{repo}/issues/comments/{id} endpoint.
    #[tokio::test]
    async fn test_update_issue_comment() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        let updated_comment_json = serde_json::json!({
            "id": 1,
            "node_id": "MDEyOklzc3VlQ29tbWVudDE=",
            "body": "Updated comment text",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "created_at": "2011-04-14T16:00:49Z",
            "updated_at": "2011-04-14T19:00:49Z",
            "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-1"
        });

        Mock::given(method("PATCH"))
            .and(path("/repos/octocat/Hello-World/issues/comments/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(updated_comment_json))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let request = UpdateCommentRequest {
            body: "Updated comment text".to_string(),
        };

        let result = client
            .update_issue_comment("octocat", "Hello-World", 1, request)
            .await;

        assert!(result.is_ok());
        let comment = result.unwrap();
        assert_eq!(comment.body, "Updated comment text");
    }

    /// Verify delete_issue_comment removes a comment.
    ///
    /// Tests DELETE /repos/{owner}/{repo}/issues/comments/{id} endpoint.
    #[tokio::test]
    async fn test_delete_issue_comment() {
        let mock_server = MockServer::start().await;
        let test_token = "ghs_test_token";

        Mock::given(method("DELETE"))
            .and(path("/repos/octocat/Hello-World/issues/comments/1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let auth = MockAuthProvider::new_with_token(test_token);
        let github_client = GitHubClient::builder(auth)
            .config(ClientConfig::default().with_github_api_url(mock_server.uri()))
            .build()
            .unwrap();

        let client = github_client
            .installation_by_id(InstallationId::new(12345))
            .await
            .unwrap();

        let result = client
            .delete_issue_comment("octocat", "Hello-World", 1)
            .await;

        assert!(result.is_ok());
    }
}

mod serialization {
    use super::*;

    /// Verify Issue can be deserialized from GitHub API response.
    ///
    /// Tests that the Issue type correctly deserializes from GitHub's API JSON format
    /// including all fields like id, number, title, state, labels, etc.
    #[test]
    fn test_issue_deserialize() {
        let json = r#"{
            "id": 1,
            "node_id": "MDU6SXNzdWUx",
            "number": 1347,
            "title": "Found a bug",
            "body": "I'm having a problem with this.",
            "state": "open",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [
                {
                    "id": 208045946,
                    "node_id": "MDU6TGFiZWwyMDgwNDU5NDY=",
                    "name": "bug",
                    "description": "Something isn't working",
                    "color": "d73a4a",
                    "default": true
                }
            ],
            "assignees": [],
            "milestone": null,
            "comments": 0,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-22T13:33:48Z",
            "closed_at": null,
            "html_url": "https://github.com/octocat/Hello-World/issues/1347"
        }"#;

        let issue: Issue = serde_json::from_str(json).unwrap();

        assert_eq!(issue.id, 1);
        assert_eq!(issue.node_id, "MDU6SXNzdWUx");
        assert_eq!(issue.number, 1347);
        assert_eq!(issue.title, "Found a bug");
        assert_eq!(
            issue.body,
            Some("I'm having a problem with this.".to_string())
        );
        assert_eq!(issue.state, "open");
        assert_eq!(issue.user.login, "octocat");
        assert_eq!(issue.labels.len(), 1);
        assert_eq!(issue.labels[0].name, "bug");
        assert_eq!(issue.comments, 0);
        assert!(issue.closed_at.is_none());
    }

    /// Verify Issue deserializes with closed state and timestamp.
    #[test]
    fn test_issue_deserialize_closed() {
        let json = r#"{
            "id": 2,
            "node_id": "MDU6SXNzdWUy",
            "number": 1348,
            "title": "Fixed bug",
            "body": null,
            "state": "closed",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "labels": [],
            "assignees": [],
            "milestone": null,
            "comments": 5,
            "created_at": "2011-04-22T13:33:48Z",
            "updated_at": "2011-04-23T13:33:48Z",
            "closed_at": "2011-04-23T13:33:48Z",
            "html_url": "https://github.com/octocat/Hello-World/issues/1348"
        }"#;

        let issue: Issue = serde_json::from_str(json).unwrap();

        assert_eq!(issue.state, "closed");
        assert_eq!(issue.body, None);
        assert!(issue.closed_at.is_some());
        assert_eq!(issue.comments, 5);
    }

    /// Verify Label can be deserialized from GitHub API response.
    #[test]
    fn test_label_deserialize() {
        let json = r#"{
            "id": 208045946,
            "node_id": "MDU6TGFiZWwyMDgwNDU5NDY=",
            "name": "bug",
            "description": "Something isn't working",
            "color": "d73a4a",
            "default": true
        }"#;

        let label: Label = serde_json::from_str(json).unwrap();

        assert_eq!(label.id, 208045946);
        assert_eq!(label.node_id, "MDU6TGFiZWwyMDgwNDU5NDY=");
        assert_eq!(label.name, "bug");
        assert_eq!(
            label.description,
            Some("Something isn't working".to_string())
        );
        assert_eq!(label.color, "d73a4a");
        assert_eq!(label.default, true);
    }

    /// Verify Comment can be deserialized from GitHub API response.
    #[test]
    fn test_comment_deserialize() {
        let json = r#"{
            "id": 1,
            "node_id": "MDEyOklzc3VlQ29tbWVudDE=",
            "body": "Great idea!",
            "user": {
                "login": "octocat",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "type": "User"
            },
            "created_at": "2011-04-14T16:00:49Z",
            "updated_at": "2011-04-14T16:00:49Z",
            "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-1"
        }"#;

        let comment: Comment = serde_json::from_str(json).unwrap();

        assert_eq!(comment.id, 1);
        assert_eq!(comment.node_id, "MDEyOklzc3VlQ29tbWVudDE=");
        assert_eq!(comment.body, "Great idea!");
        assert_eq!(comment.user.login, "octocat");
        assert_eq!(
            comment.html_url,
            "https://github.com/octocat/Hello-World/issues/1347#issuecomment-1"
        );
    }

    /// Verify Milestone can be deserialized from GitHub API response.
    #[test]
    fn test_milestone_deserialize() {
        let json = r#"{
            "id": 1002604,
            "node_id": "MDk6TWlsZXN0b25lMTAwMjYwNA==",
            "number": 1,
            "title": "v1.0",
            "description": "Tracking milestone for version 1.0",
            "state": "open",
            "open_issues": 4,
            "closed_issues": 8,
            "due_on": "2012-10-09T23:39:01Z",
            "created_at": "2011-04-10T20:09:31Z",
            "updated_at": "2014-03-03T18:58:10Z",
            "closed_at": null
        }"#;

        let milestone: Milestone = serde_json::from_str(json).unwrap();

        assert_eq!(milestone.id, 1002604);
        assert_eq!(milestone.node_id, "MDk6TWlsZXN0b25lMTAwMjYwNA==");
        assert_eq!(milestone.number, 1);
        assert_eq!(milestone.title, "v1.0");
        assert_eq!(
            milestone.description,
            Some("Tracking milestone for version 1.0".to_string())
        );
        assert_eq!(milestone.state, "open");
        assert_eq!(milestone.open_issues, 4);
        assert_eq!(milestone.closed_issues, 8);
        assert!(milestone.due_on.is_some());
        assert!(milestone.closed_at.is_none());
    }

    /// Verify CreateIssueRequest serializes correctly.
    #[test]
    fn test_create_issue_request_serialize() {
        let request = CreateIssueRequest {
            title: "Found a bug".to_string(),
            body: Some("I'm having a problem".to_string()),
            assignees: Some(vec!["octocat".to_string()]),
            milestone: Some(1),
            labels: Some(vec!["bug".to_string(), "high-priority".to_string()]),
        };

        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["title"], "Found a bug");
        assert_eq!(json["body"], "I'm having a problem");
        assert_eq!(json["assignees"][0], "octocat");
        assert_eq!(json["milestone"], 1);
        assert_eq!(json["labels"].as_array().unwrap().len(), 2);
    }

    /// Verify UpdateIssueRequest skips None fields.
    #[test]
    fn test_update_issue_request_serialize_partial() {
        let request = UpdateIssueRequest {
            title: Some("Updated title".to_string()),
            state: Some("closed".to_string()),
            body: None,
            assignees: None,
            milestone: None,
            labels: None,
        };

        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["title"], "Updated title");
        assert_eq!(json["state"], "closed");
        // None fields should not be present
        assert!(json.get("body").is_none());
        assert!(json.get("assignees").is_none());
        assert!(json.get("milestone").is_none());
        assert!(json.get("labels").is_none());
    }
}

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn test_issue_not_found() {
        todo!("Mock: 404 response returns ApiError::NotFound")
    }

    #[tokio::test]
    async fn test_forbidden_access() {
        todo!("Mock: 403 response returns ApiError::Forbidden")
    }

    #[tokio::test]
    async fn test_validation_error() {
        todo!("Mock: 422 response for invalid input")
    }
}
