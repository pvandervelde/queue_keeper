# App-Level Authentication

## Overview

This document clarifies the distinction between app-level and installation-level authentication in the GitHub Bot SDK, addressing use cases where bots need to authenticate as the GitHub App itself rather than as a specific installation.

## Authentication Levels

### App-Level Authentication (JWT)

Authenticate as the GitHub App itself using JWT tokens signed with the app's private key.

**Authorization Header**: `Bearer <JWT>`

**Use Cases**:

1. **Installation Discovery**: List all installations to discover where the app is installed
2. **App Management**: Get app information, update app configuration
3. **Installation Management**: Get installation details, suspend/unsuspend installations
4. **Token Exchange**: Create installation access tokens (the initial exchange)
5. **Marketplace Operations**: Access app marketplace and billing information
6. **Webhook Configuration**: Manage app-level webhook settings

**GitHub API Endpoints**:

- `GET /app` - Get the authenticated app
- `GET /app/installations` - List installations for the authenticated app
- `GET /app/installations/{installation_id}` - Get a specific installation
- `POST /app/installations/{installation_id}/access_tokens` - Create installation token
- `PUT /app/installations/{installation_id}/suspended` - Suspend installation
- `DELETE /app/installations/{installation_id}/suspended` - Unsuspend installation
- `GET /app/hook/config` - Get webhook configuration
- `PATCH /app/hook/config` - Update webhook configuration

**Token Lifetime**: Maximum 10 minutes (GitHub requirement)

**Caching**: Generally not cached; generated on-demand for each operation

### Installation-Level Authentication (Installation Token)

Authenticate as a specific installation using installation access tokens.

**Authorization Header**: `Bearer <installation_token>` or `token <installation_token>`

**Use Cases**:

1. **Repository Operations**: All operations on repositories (issues, PRs, commits, etc.)
2. **Organization Operations**: Organization-level operations within installation scope
3. **User Operations**: User-specific operations within installation scope
4. **Webhook Processing**: Operations triggered by webhook events
5. **Scheduled Tasks**: Background jobs operating on installed repositories

**GitHub API Endpoints**:

- `GET /repos/{owner}/{repo}/*` - All repository endpoints
- `GET /orgs/{org}/*` - Organization endpoints (within installation scope)
- `GET /user/*` - User endpoints (within installation scope)
- Any operation requiring repository or organization access

**Token Lifetime**: 1 hour from creation (GitHub default)

**Caching**: Should be cached with installation ID as key; refresh before expiration

## Common Bot Patterns

### Pattern 1: Installation Discovery Bot

A bot that needs to discover all installations and then operate on each:

```rust
// 1. Authenticate as app to discover installations
let installations = client.list_installations().await?;

// 2. For each installation, authenticate and operate
for installation in installations {
    let inst_client = client.installation_by_id(installation.id).await?;

    // Now perform installation-scoped operations
    let repos = inst_client.list_repositories().await?;
    for repo in repos {
        process_repository(&inst_client, &repo).await?;
    }
}
```

### Pattern 2: Webhook-Triggered Bot

A bot that responds to webhooks and needs to authenticate for the specific installation:

```rust
// Webhook contains installation_id in payload
let event = parse_webhook(payload)?;

// Authenticate as the specific installation
let inst_client = client.installation_by_id(event.installation.id).await?;

// Perform operations on the repository
inst_client.create_issue_comment(
    &event.repository,
    event.issue.number,
    "Thank you for the issue!"
).await?;
```

### Pattern 3: Repository-Scoped Bot

A bot configured with a specific repository that doesn't need installation discovery:

```rust
// If you know the repository, SDK discovers installation automatically
let repo = Repository {
    owner: "acme".to_string(),
    name: "project".to_string(),
    full_name: "acme/project".to_string(),
};

let inst_client = client.installation(&repo).await?;

// Now perform operations on this repository
let issues = inst_client.list_issues(&Default::default()).await?;
```

### Pattern 4: App Management Bot

A bot that manages the GitHub App itself:

```rust
// Authenticate as app to manage installations
let app = client.get_app().await?;
println!("Managing app: {}", app.name);

// Get specific installation details
let installation = client.get_installation(12345).await?;

// Suspend installation if needed
if installation.suspended_at.is_none() {
    client.post_as_app(
        &format!("/app/installations/{}/suspended", installation.id),
        &json!({})
    ).await?;
}
```

## Implementation Requirements

### AuthenticationProvider Trait

The `AuthenticationProvider` trait must support both authentication levels:

```rust
#[async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Get JWT token for app-level GitHub API operations.
    ///
    /// Use this for operations that require authentication as the GitHub App itself, such as:
    /// - Listing installations (`GET /app/installations`)
    /// - Getting app information (`GET /app`)
    /// - Managing installations (`GET /app/installations/{installation_id}`)
    ///
    /// This method handles caching and automatic refresh of JWTs.
    async fn app_token(&self) -> Result<JsonWebToken, AuthError>;

    /// Get installation token for installation-level API operations.
    ///
    /// Use this for operations within a specific installation context, such as:
    /// - Repository operations (reading files, creating issues/PRs)
    /// - Organization operations (team management, webhooks)
    /// - Any operation scoped to the installation's permissions
    ///
    /// This method handles caching and automatic refresh of installation tokens.
    ///
    /// # Arguments
    ///
    /// * `installation_id` - The installation to get a token for
    async fn installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// Refresh installation token (force new token generation).
    ///
    /// Bypasses cache and requests a new installation token from GitHub.
    /// Use sparingly as it counts against rate limits.
    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// List all installations for this GitHub App.
    ///
    /// Requires app-level authentication. This is a convenience method that combines
    /// app_token() with the list installations API call.
    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError>;

    /// Get repositories accessible by installation.
    ///
    /// Requires installation-level authentication. This is a convenience method that combines
    /// installation_token() with the list repositories API call.
    async fn get_installation_repositories(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<Repository>, AuthError>;
}
```

### GitHubClient Interface

The `GitHubClient` must provide methods for both authentication levels:

```rust
impl GitHubClient {
    // App-level operations (use JWT)
    pub async fn list_installations(&self) -> Result<Vec<Installation>, ClientError>;
    pub async fn get_app(&self) -> Result<App, ClientError>;
    pub async fn get_installation(&self, installation_id: u64) -> Result<Installation, ClientError>;
    pub async fn get_as_app(&self, path: &str) -> Result<Response, ClientError>;
    pub async fn post_as_app(&self, path: &str, body: &impl Serialize) -> Result<Response, ClientError>;

    // Installation-level operations (use installation token)
    pub async fn installation(&self, repo: &Repository) -> Result<InstallationClient, ClientError>;
    pub async fn installation_by_id(&self, installation_id: u64) -> Result<InstallationClient, ClientError>;
    pub async fn get(&self, installation_id: u64, path: &str) -> Result<Response, ClientError>;
    pub async fn post(&self, installation_id: u64, path: &str, body: &impl Serialize) -> Result<Response, ClientError>;
}
```

## Security Considerations

### JWT Security

- JWTs authenticate the app itself and have powerful permissions
- Always use HTTPS for API requests
- Never log JWTs or include them in error messages
- Limit JWT expiration to minimum required (max 10 minutes per GitHub)
- Generate fresh JWTs for each app-level operation if possible

### Installation Token Security

- Installation tokens are scoped to specific repositories/organizations
- Use installation tokens for all repository operations (not JWTs)
- Cache installation tokens to avoid excessive token generation
- Refresh tokens before expiration to avoid operation failures
- Tokens are automatically scoped by GitHub to installation permissions

### Principle of Least Privilege

- Use installation tokens whenever possible (more restrictive scope)
- Only use app-level authentication when truly needed
- Configure minimal required permissions for each installation
- Avoid storing tokens longer than necessary

## Testing Strategies

### App-Level Authentication Tests

```rust
#[tokio::test]
async fn test_list_installations() {
    let auth = MockAuthProvider::new()
        .with_jwt("mock-jwt-token");

    let client = GitHubClient::new(auth);

    // Should use JWT authentication
    let installations = client.list_installations().await.unwrap();

    assert!(!installations.is_empty());
}
```

### Installation-Level Authentication Tests

```rust
#[tokio::test]
async fn test_create_issue_with_installation_token() {
    let auth = MockAuthProvider::new()
        .with_installation_token(12345, "mock-installation-token");

    let client = GitHubClient::new(auth);
    let inst_client = client.installation_by_id(12345).await.unwrap();

    // Should use installation token authentication
    let issue = inst_client.create_issue(&repo, &issue_data).await.unwrap();

    assert_eq!(issue.title, "Test Issue");
}
```

### Authentication Context Tests

```rust
#[tokio::test]
async fn test_automatic_authentication_context() {
    let client = setup_client();

    // App-level call uses JWT
    let app = client.get_app().await.unwrap();
    assert_uses_jwt_auth(&app);

    // Installation-level call uses installation token
    let inst_client = client.installation_by_id(12345).await.unwrap();
    let repos = inst_client.list_repositories().await.unwrap();
    assert_uses_installation_token_auth(&repos);
}
```

## Migration Guide

For existing code that only implements installation-level authentication:

1. **Identify app-level operations**: Review your code for operations that might benefit from app-level authentication
2. **Add app-level methods**: Implement `list_installations()`, `get_app()`, and other app-level operations
3. **Update client interface**: Ensure `GitHubClient` exposes both `get_as_app()` and `get()` methods
4. **Update authentication provider**: Ensure `AuthProvider` implements `app_token()` method
5. **Update tests**: Add tests for both authentication levels

## Summary

**Use App-Level Authentication (JWT) For**:

- Listing installations
- Managing installations
- Creating installation tokens
- App-level configuration
- Marketplace operations

**Use Installation-Level Authentication (Installation Token) For**:

- All repository operations
- Organization operations
- Webhook processing
- User operations
- Any operation on specific repositories

This two-level authentication model enables GitHub Apps to both manage their own installation lifecycle and perform repository-specific operations within appropriate security scopes.
