# Installation Client Interface Specification

**Module**: `github-bot-sdk::client::installation`
**File**: `crates/github-bot-sdk/src/client/installation.rs`
**Dependencies**: `GitHubClient`, `AuthProvider`, `InstallationId`, `ApiError`

## Overview

The `InstallationClient` provides installation-scoped access to GitHub API operations. It is bound to a specific installation ID and uses installation tokens (not JWTs) for authentication. All operations respect GitHub's rate limiting and installation permissions.

## Architectural Location

**Layer**: Infrastructure adapter
**Purpose**: GitHub API client for installation-level operations
**Authentication**: Installation tokens via `AuthProvider::installation_token()`

## Core Type

### InstallationClient

Client struct bound to a specific GitHub App installation.

```rust
pub struct InstallationClient {
    /// Parent GitHub client (shared HTTP client, auth provider, rate limiter)
    client: Arc<GitHubClient>,
    /// Installation ID this client is bound to
    installation_id: InstallationId,
}
```

**Design Notes**:

- Holds `Arc<GitHubClient>` to share HTTP client and connection pool
- Installation ID is fixed at construction time
- All operations delegate to parent client for HTTP and token management
- Thread-safe via `Arc` - can be cloned cheaply

## Factory Methods

### GitHubClient::installation_by_id

Create an installation client for a specific installation ID.

```rust
impl GitHubClient {
    /// Create an installation-scoped client.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The GitHub App installation ID
    ///
    /// # Returns
    ///
    /// Returns an `InstallationClient` bound to the specified installation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use github_bot_sdk::client::GitHubClient;
    /// # use github_bot_sdk::auth::InstallationId;
    /// # async fn example(github_client: &GitHubClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let installation_id = InstallationId::new(12345);
    /// let client = github_client.installation_by_id(installation_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if the installation ID is invalid or inaccessible.
    pub async fn installation_by_id(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationClient, ApiError>;
}
```

**Behavior**:

1. Validate installation ID exists (optional - can defer to first API call)
2. Create `InstallationClient` with reference to parent
3. Return immediately (no API call required)

**Error Conditions**:

- None at construction time (validation happens on first request)

## Generic Request Methods

All installation operations use these generic helpers that handle authentication and error mapping.

### GET Request

```rust
impl InstallationClient {
    /// Make an authenticated GET request to the GitHub API.
    ///
    /// Uses installation token for authentication.
    ///
    /// # Arguments
    ///
    /// * `path` - API path (e.g., "/repos/owner/repo" or "repos/owner/repo")
    ///
    /// # Returns
    ///
    /// Returns the raw `reqwest::Response` for flexible handling.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` for HTTP errors, authentication failures, or network issues.
    pub async fn get(&self, path: &str) -> Result<reqwest::Response, ApiError>;
}
```

### POST Request

```rust
impl InstallationClient {
    /// Make an authenticated POST request to the GitHub API.
    ///
    /// # Arguments
    ///
    /// * `path` - API path
    /// * `body` - Request body (will be serialized to JSON)
    ///
    /// # Errors
    ///
    /// Returns `ApiError` for HTTP errors, serialization failures, or network issues.
    pub async fn post<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError>;
}
```

### PUT Request

```rust
impl InstallationClient {
    /// Make an authenticated PUT request to the GitHub API.
    pub async fn put<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError>;
}
```

### DELETE Request

```rust
impl InstallationClient {
    /// Make an authenticated DELETE request to the GitHub API.
    pub async fn delete(&self, path: &str) -> Result<reqwest::Response, ApiError>;
}
```

### PATCH Request

```rust
impl InstallationClient {
    /// Make an authenticated PATCH request to the GitHub API.
    pub async fn patch<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError>;
}
```

## Request Method Behavior

All generic request methods follow this pattern:

1. **Get Installation Token**: Call `self.client.auth_provider().installation_token(self.installation_id)`
2. **Build Request**: Create HTTP request with:
   - URL: `{github_api_url}/{normalized_path}`
   - Headers:
     - `Authorization: Bearer {installation_token}`
     - `Accept: application/vnd.github+json`
     - `User-Agent: {from client config}`
3. **Send Request**: Execute via `self.client.http_client()`
4. **Return Response**: Return raw response (caller handles status checking and parsing)

**Path Normalization**:

- Remove leading `/` if present
- Example: `/repos/owner/repo` → `repos/owner/repo`

## Token Management Integration

Installation client delegates token management to the parent `GitHubClient`:

```rust
// Internal implementation detail (not public API)
async fn get_installation_token(&self) -> Result<InstallationToken, ApiError> {
    self.client
        .auth_provider()
        .installation_token(self.installation_id)
        .await
        .map_err(|e| ApiError::TokenGenerationFailed {
            message: format!("Failed to get installation token: {}", e),
        })
}
```

**Token Caching**:

- Handled by `AuthProvider` implementation
- InstallationClient doesn't cache tokens directly
- Fresh token obtained for each request (cache is transparent)

## Error Handling

### Error Mapping

Generic request methods may return these errors:

| Error Variant | Condition | HTTP Status |
|--------------|-----------|-------------|
| `ApiError::TokenGenerationFailed` | Can't get installation token | N/A |
| `ApiError::HttpClientError` | Network failure | N/A |
| `ApiError::Timeout` | Request timeout | N/A |
| `ApiError::HttpError` | HTTP error response | Any non-2xx |
| `ApiError::PermissionDenied` | Insufficient permissions | 403 |
| `ApiError::NotFound` | Resource not found | 404 |
| `ApiError::RateLimitExceeded` | Rate limit hit | 429 |

**Note**: Specific operations (defined in other interface specs) map HTTP errors to appropriate `ApiError` variants.

## Usage Examples

### Basic GET Request

```rust
use github_bot_sdk::client::{GitHubClient, InstallationClient};
use github_bot_sdk::auth::InstallationId;

async fn example(github_client: &GitHubClient) -> Result<(), Box<dyn std::error::Error>> {
    let installation_id = InstallationId::new(12345);
    let client = github_client.installation_by_id(installation_id).await?;

    let response = client.get("repos/octocat/Hello-World").await?;

    if response.status().is_success() {
        let repo_data: serde_json::Value = response.json().await?;
        println!("Repository: {:?}", repo_data);
    }

    Ok(())
}
```

### POST Request with Body

```rust
use serde_json::json;

async fn create_issue_example(client: &InstallationClient) -> Result<(), Box<dyn std::error::Error>> {
    let issue_data = json!({
        "title": "Bug found",
        "body": "Description of the bug"
    });

    let response = client.post("repos/octocat/Hello-World/issues", &issue_data).await?;

    if response.status().is_success() {
        println!("Issue created successfully");
    }

    Ok(())
}
```

## Implementation Notes

### Thread Safety

- `InstallationClient` is `Send + Sync` (via `Arc<GitHubClient>`)
- Can be cloned cheaply (increments `Arc` reference count)
- Safe to use across async tasks

### Performance Characteristics

- Construction: O(1) - just creates struct
- Request overhead: ~2-5ms for token retrieval (cached)
- Network latency: Variable (depends on GitHub API)

### Testing Strategy

- Mock `GitHubClient` in tests
- Verify correct Authorization header usage
- Test token error propagation
- Test path normalization

## Assertions

This interface supports:

- **Assertion #3a**: Installation token usage (not JWT)
- **Assertion #5**: Installation-level operations use installation tokens

## Next Steps

After implementing this foundation, add domain-specific operations:

1. Repository operations (task 5a.0)
2. Issue operations (task 5b.0)
3. Pull request operations (task 5c.0)
4. Additional operations (milestones, workflows, releases)

## References

- `github-bot-sdk-specs/modules/client.md` - Client module specification
- `github-bot-sdk-specs/architecture/app-level-authentication.md` - Authentication patterns
- `crates/github-bot-sdk/src/client/mod.rs` - GitHubClient implementation
