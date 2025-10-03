# GitHub Client Module

The client module provides a high-level, authenticated GitHub API client built on top of the authentication module. It handles rate limiting, retries, pagination, and provides both REST and GraphQL capabilities.

## Overview

The GitHub client abstracts the complexity of making authenticated API requests to GitHub, providing a clean interface for common operations while maintaining flexibility for custom requests.

## Core Types

### GitHubClient

The main client interface that handles all GitHub API interactions.

```rust
pub struct GitHubClient {
    auth: Arc<dyn AuthProvider>,
    http_client: reqwest::Client,
    rate_limiter: Arc<RateLimiter>,
    config: ClientConfig,
}

impl GitHubClient {
    pub fn new(auth: impl AuthProvider + 'static) -> GitHubClientBuilder { ... }

    /// Get an installation-specific client
    pub async fn installation(&self, repo: &Repository) -> Result<InstallationClient, ClientError> { ... }

    /// Make a raw authenticated GET request
    pub async fn get(&self, installation_id: u64, path: &str) -> Result<Response, ClientError> { ... }

    /// Make a raw authenticated POST request
    pub async fn post(&self, installation_id: u64, path: &str, body: &impl Serialize) -> Result<Response, ClientError> { ... }

    /// Execute a GraphQL query
    pub async fn graphql(&self, installation_id: u64, query: &str, variables: Value) -> Result<GraphQLResponse, ClientError> { ... }
}
```

### InstallationClient

A client bound to a specific GitHub App installation, providing convenient methods for common operations.

```rust
pub struct InstallationClient {
    client: Arc<GitHubClient>,
    installation_id: u64,
    repository: Repository,
}

impl InstallationClient {
    // Repository operations
    pub async fn get_repository(&self) -> Result<RepositoryInfo, ClientError> { ... }
    pub async fn list_branches(&self) -> Result<Vec<Branch>, ClientError> { ... }

    // Issue operations
    pub async fn get_issue(&self, number: u32) -> Result<Issue, ClientError> { ... }
    pub async fn create_issue(&self, issue: &CreateIssue) -> Result<Issue, ClientError> { ... }
    pub async fn update_issue(&self, number: u32, update: &UpdateIssue) -> Result<Issue, ClientError> { ... }
    pub async fn list_issues(&self, params: &IssueListParams) -> Result<PagedResponse<Issue>, ClientError> { ... }

    // Pull Request operations
    pub async fn get_pull_request(&self, number: u32) -> Result<PullRequest, ClientError> { ... }
    pub async fn list_pull_requests(&self, params: &PrListParams) -> Result<PagedResponse<PullRequest>, ClientError> { ... }
    pub async fn create_review(&self, pr_number: u32, review: &CreateReview) -> Result<Review, ClientError> { ... }

    // Comment operations
    pub async fn create_issue_comment(&self, issue_number: u32, body: &str) -> Result<Comment, ClientError> { ... }
    pub async fn create_pr_comment(&self, pr_number: u32, comment: &CreatePrComment) -> Result<Comment, ClientError> { ... }

    // Status and Check operations
    pub async fn create_status(&self, sha: &str, status: &CreateStatus) -> Result<Status, ClientError> { ... }
    pub async fn create_check_run(&self, check: &CreateCheckRun) -> Result<CheckRun, ClientError> { ... }
    pub async fn update_check_run(&self, check_run_id: u64, update: &UpdateCheckRun) -> Result<CheckRun, ClientError> { ... }

    // Label operations
    pub async fn add_labels(&self, issue_number: u32, labels: &[String]) -> Result<Vec<Label>, ClientError> { ... }
    pub async fn remove_label(&self, issue_number: u32, label: &str) -> Result<(), ClientError> { ... }
}
```

### ClientConfig

Configuration for client behavior, rate limiting, and retry policies.

```rust
pub struct ClientConfig {
    pub user_agent: String,
    pub timeout: Duration,
    pub max_retries: u32,
    pub retry_backoff: RetryBackoff,
    pub rate_limit_margin: f64,
    pub github_api_url: String,
    pub graphql_url: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            user_agent: "github-bot-sdk/1.0".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_backoff: RetryBackoff::exponential(Duration::from_millis(100)),
            rate_limit_margin: 0.1, // Keep 10% buffer
            github_api_url: "https://api.github.com".to_string(),
            graphql_url: "https://api.github.com/graphql".to_string(),
        }
    }
}
```

## Rate Limiting

The client implements intelligent rate limiting to respect GitHub's API limits and avoid abuse detection.

### RateLimiter

```rust
pub struct RateLimiter {
    limits: Arc<RwLock<HashMap<String, RateLimit>>>,
    config: RateLimitConfig,
}

pub struct RateLimit {
    pub limit: u32,
    pub remaining: u32,
    pub reset: DateTime<Utc>,
    pub resource: String,
}

impl RateLimiter {
    pub async fn acquire(&self, resource: &str, installation_id: u64) -> Result<(), ClientError> { ... }

    pub async fn update_from_headers(&self, resource: &str, headers: &HeaderMap) { ... }

    pub fn get_limit(&self, resource: &str) -> Option<RateLimit> { ... }
}
```

### Rate Limit Strategies

1. **Proactive Limiting**: Check rate limits before making requests
2. **Header Parsing**: Update limits from GitHub response headers
3. **Exponential Backoff**: Automatic retry with increasing delays
4. **Per-Installation Tracking**: Separate limits for each installation
5. **Circuit Breaking**: Temporary halt when limits exceeded

## Pagination Support

The client provides automatic pagination handling for list operations.

### PagedResponse

```rust
pub struct PagedResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub total_count: Option<u32>,
    pub has_next: bool,
    pub next_page: Option<u32>,
}

impl<T> PagedResponse<T> {
    pub async fn next(&self, client: &InstallationClient) -> Result<Option<PagedResponse<T>>, ClientError> { ... }

    pub fn into_stream(self, client: &InstallationClient) -> impl Stream<Item = Result<T, ClientError>> { ... }
}
```

### Pagination Parameters

```rust
pub struct PaginationParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub sort: Option<String>,
    pub direction: Option<SortDirection>,
}

pub enum SortDirection {
    Asc,
    Desc,
}
```

## GraphQL Support

The client provides GraphQL query capabilities for complex data fetching.

### GraphQL Types

```rust
pub struct GraphQLQuery {
    pub query: String,
    pub variables: serde_json::Value,
    pub operation_name: Option<String>,
}

pub struct GraphQLResponse {
    pub data: Option<serde_json::Value>,
    pub errors: Option<Vec<GraphQLError>>,
}

pub struct GraphQLError {
    pub message: String,
    pub locations: Option<Vec<Location>>,
    pub path: Option<Vec<serde_json::Value>>,
}
```

### GraphQL Usage

```rust
let query = r#"
    query GetRepository($owner: String!, $name: String!) {
        repository(owner: $owner, name: $name) {
            id
            name
            description
            stargazerCount
            forkCount
            pullRequests(first: 10, states: OPEN) {
                nodes {
                    id
                    number
                    title
                    author {
                        login
                    }
                }
            }
        }
    }
"#;

let variables = json!({
    "owner": "octocat",
    "name": "Hello-World"
});

let response = client.graphql(installation_id, query, variables).await?;
```

## Request/Response Types

The client uses strongly-typed structs for all GitHub API operations.

### Common GitHub Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: IssueState,
    pub user: User,
    pub labels: Vec<Label>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: PrState,
    pub user: User,
    pub head: PrBranch,
    pub base: PrBranch,
    pub mergeable: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub login: String,
    pub avatar_url: String,
    pub html_url: String,
}
```

### Request Types

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CreateIssue {
    pub title: String,
    pub body: Option<String>,
    pub labels: Option<Vec<String>>,
    pub assignees: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateReview {
    pub body: Option<String>,
    pub event: ReviewEvent,
    pub comments: Option<Vec<ReviewComment>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateStatus {
    pub state: StatusState,
    pub target_url: Option<String>,
    pub description: Option<String>,
    pub context: String,
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Authentication failed: {source}")]
    Auth { source: AuthError },

    #[error("Rate limit exceeded for {resource}. Resets at {reset}")]
    RateLimit { resource: String, reset: DateTime<Utc> },

    #[error("GitHub API error: {status} - {message}")]
    GitHub { status: u16, message: String, response_body: String },

    #[error("Request timeout after {timeout:?}")]
    Timeout { timeout: Duration },

    #[error("Network error: {source}")]
    Network { source: reqwest::Error },

    #[error("Serialization error: {source}")]
    Serialization { source: serde_json::Error },

    #[error("Invalid response format: {message}")]
    InvalidResponse { message: String },

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Permission denied for {operation} on {resource}")]
    PermissionDenied { operation: String, resource: String },
}
```

## Usage Examples

### Basic Client Setup

```rust
use github_bot_sdk::{GitHubAppAuth, GitHubClient};

let auth = GitHubAppAuth::new()
    .app_id(12345)
    .private_key_from_env("GITHUB_PRIVATE_KEY")?
    .build()?;

let client = GitHubClient::new(auth)
    .user_agent("my-bot/1.0")
    .timeout(Duration::from_secs(30))
    .max_retries(3)
    .build();
```

### Working with Issues

```rust
let repo = Repository {
    owner: "octocat".to_string(),
    name: "Hello-World".to_string(),
    full_name: "octocat/Hello-World".to_string(),
};

let installation = client.installation(&repo).await?;

// Create a new issue
let new_issue = CreateIssue {
    title: "Bug report".to_string(),
    body: Some("Found a bug in the application".to_string()),
    labels: Some(vec!["bug".to_string()]),
    assignees: None,
};

let issue = installation.create_issue(&new_issue).await?;

// Add a comment
installation.create_issue_comment(
    issue.number,
    "Thank you for reporting this issue!"
).await?;

// Add labels
installation.add_labels(issue.number, &["needs-triage".to_string()]).await?;
```

### Handling Pull Requests

```rust
// List open pull requests
let pr_params = PrListParams {
    state: Some(PrState::Open),
    sort: Some("updated".to_string()),
    direction: Some(SortDirection::Desc),
    ..Default::default()
};

let prs = installation.list_pull_requests(&pr_params).await?;

for pr in prs.items {
    // Create a review
    let review = CreateReview {
        body: Some("LGTM! 🚀".to_string()),
        event: ReviewEvent::Approve,
        comments: None,
    };

    installation.create_review(pr.number, &review).await?;
}
```

### Custom API Requests

```rust
// Make a custom API request
let response = client.get(installation_id, "/repos/octocat/Hello-World/contributors").await?;
let contributors: Vec<Contributor> = response.json().await?;

// POST with custom payload
let payload = json!({
    "name": "feature-branch",
    "sha": "main"
});

let response = client.post(
    installation_id,
    "/repos/octocat/Hello-World/git/refs",
    &payload
).await?;
```

## Testing Support

```rust
#[cfg(test)]
pub mod testing {
    use super::*;

    pub struct MockGitHubClient {
        responses: HashMap<String, MockResponse>,
    }

    impl MockGitHubClient {
        pub fn new() -> Self { ... }

        pub fn expect_get(mut self, path: &str, response: MockResponse) -> Self { ... }

        pub fn expect_post(mut self, path: &str, response: MockResponse) -> Self { ... }
    }

    pub struct MockResponse {
        pub status: u16,
        pub body: serde_json::Value,
        pub headers: HeaderMap,
    }
}
```

## Performance Characteristics

- **Request Overhead**: ~2-5ms per request (auth + serialization)
- **Rate Limit Check**: ~0.1ms per request
- **Retry Logic**: Exponential backoff up to 30 seconds
- **Connection Pooling**: Reuses HTTP connections for efficiency
- **Memory Usage**: ~500KB base overhead, ~1KB per cached rate limit
