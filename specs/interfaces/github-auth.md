# GitHub Authentication Specification

**Module Path**: `crates/github-bot-sdk/src/auth/mod.rs`

**Architectural Layer**: Core Domain (GitHub Integration)

**Responsibilities**: Manages GitHub App authentication lifecycle including JWT generation, installation token exchange, and secure token caching

**Related Documentation**: See `github-bot-sdk-specs/architecture/app-level-authentication.md` for comprehensive guide on authentication levels and usage patterns.

## Authentication Levels

GitHub App authentication operates at two distinct levels:

### App-Level Authentication (JWT)

**Purpose**: Authenticate AS the GitHub App itself for app-wide operations.

**Token Type**: JSON Web Token (JWT)

- **Lifetime**: Maximum 10 minutes (GitHub requirement)
- **Signing**: RS256 with private key
- **Scope**: App-wide operations

**Use Cases**:

- Listing all installations (`GET /app/installations`)
- Getting app information (`GET /app`)
- Managing specific installation (`GET /app/installations/{id}`)
- Converting webhook events to installation IDs

**API Method**: `AuthenticationProvider::app_token()`

### Installation-Level Authentication (Installation Token)

**Purpose**: Authenticate as a specific installation for repository/org operations.

**Token Type**: Installation Access Token

- **Lifetime**: 1 hour (GitHub managed)
- **Obtained**: JWT exchanged for installation token via GitHub API
- **Scope**: Limited to installation's permissions and repositories

**Use Cases**:

- Repository operations (create issues/PRs, read files, webhooks)
- Organization operations (team management, org settings)
- Any operation within the installation's granted permissions

**API Method**: `AuthenticationProvider::installation_token(installation_id)`

### Choosing the Right Authentication Level

| Operation Type | Auth Level | Method |
|----------------|------------|--------|
| List installations | App-level | `app_token()` |
| Get app metadata | App-level | `app_token()` |
| Discover installation for webhook | App-level | `app_token()` |
| Create issue/PR | Installation-level | `installation_token(id)` |
| Read repository files | Installation-level | `installation_token(id)` |
| Manage webhooks | Installation-level | `installation_token(id)` |
| Update check runs | Installation-level | `installation_token(id)` |

**Hybrid Pattern**: Many bots use app-level authentication to discover the appropriate installation, then switch to installation-level authentication for actual operations.

## Dependencies

- Shared Types: `UserId`, `RepositoryId`, `Timestamp`, `ValidationError`
- External Traits: `SecretProvider`, `TokenCache`, `JwtSigner`
- Cryptography: `jsonwebtoken`, `hmac`, `sha2`
- Time: `chrono::{DateTime, Utc, Duration}`

## Core Types

### GitHubAppId

GitHub App identifier assigned during app registration.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GitHubAppId(u64);

impl GitHubAppId {
    pub fn new(id: u64) -> Self;
    pub fn as_u64(&self) -> u64;
}
```

### InstallationId

GitHub App installation identifier for specific org/user accounts.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallationId(u64);

impl InstallationId {
    pub fn new(id: u64) -> Self;
    pub fn as_u64(&self) -> u64;
}
```

### JsonWebToken

JWT token for GitHub App authentication (short-lived, 10 minutes max).

```rust
#[derive(Debug, Clone)]
pub struct JsonWebToken {
    token: String,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    app_id: GitHubAppId,
}

impl JsonWebToken {
    pub fn new(token: String, app_id: GitHubAppId, expires_at: DateTime<Utc>) -> Self;
    pub fn token(&self) -> &str;
    pub fn is_expired(&self) -> bool;
    pub fn expires_soon(&self, margin: Duration) -> bool;
    pub fn time_until_expiry(&self) -> Duration;
}
```

**Security Requirements**:

- Token string never logged or included in Debug output
- Maximum lifetime: 10 minutes (GitHub requirement)
- Should be refreshed 2 minutes before expiry

### InstallationToken

Installation-scoped access token for GitHub API operations.

```rust
#[derive(Debug, Clone)]
pub struct InstallationToken {
    token: String,
    installation_id: InstallationId,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    permissions: InstallationPermissions,
    repositories: Vec<RepositoryId>,
}

impl InstallationToken {
    pub fn new(
        token: String,
        installation_id: InstallationId,
        expires_at: DateTime<Utc>,
        permissions: InstallationPermissions,
        repositories: Vec<RepositoryId>,
    ) -> Self;

    pub fn token(&self) -> &str;
    pub fn installation_id(&self) -> InstallationId;
    pub fn is_expired(&self) -> bool;
    pub fn expires_soon(&self, margin: Duration) -> bool;
    pub fn has_permission(&self, permission: Permission) -> bool;
    pub fn can_access_repository(&self, repo_id: RepositoryId) -> bool;
}
```

**Security Requirements**:

- Token string redacted in Debug output
- Lifetime: 1 hour (GitHub managed)
- Should be refreshed 5 minutes before expiry
- Scope limited to specific installation permissions

### InstallationPermissions

Permissions granted to the GitHub App installation.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallationPermissions {
    pub issues: PermissionLevel,
    pub pull_requests: PermissionLevel,
    pub contents: PermissionLevel,
    pub metadata: PermissionLevel,
    pub checks: PermissionLevel,
    pub actions: PermissionLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    None,
    Read,
    Write,
    Admin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    ReadIssues,
    WriteIssues,
    ReadPullRequests,
    WritePullRequests,
    ReadContents,
    WriteContents,
    ReadChecks,
    WriteChecks,
}
```

## Core Operations

### AuthenticationProvider

Main interface for GitHub App authentication operations with support for both app-level and installation-level authentication.

```rust
#[async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Get JWT token for app-level API operations.
    ///
    /// Returns a cached JWT if available and not expiring soon (within 2 minutes).
    /// Automatically generates new JWT when needed.
    ///
    /// # Use Cases
    /// - Listing installations
    /// - Getting app information
    /// - Managing installations
    ///
    /// # Errors
    /// * `AuthError::SecretError` - Failed to retrieve private key or app ID
    /// * `AuthError::SigningError` - JWT signing failed
    async fn app_token(&self) -> Result<JsonWebToken, AuthError>;

    /// Get installation token for installation-level API operations.
    ///
    /// Returns a cached token if available and not expiring soon (within 5 minutes).
    /// Automatically exchanges JWT for installation token when needed.
    ///
    /// # Use Cases
    /// - Repository operations
    /// - Issue/PR management
    /// - Any operation within installation scope
    ///
    /// # Errors
    /// * `AuthError::InstallationNotFound` - Installation doesn't exist or access denied
    /// * `AuthError::GitHubApiError` - GitHub API request failed
    async fn installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// Refresh installation token (bypass cache, force new token).
    ///
    /// Use sparingly as it counts against rate limits.
    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// List all installations for this GitHub App.
    ///
    /// Convenience method combining app_token() with list installations API.
    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError>;

    /// Get repositories accessible by installation.
    ///
    /// Convenience method combining installation_token() with list repositories API.
    async fn get_installation_repositories(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<Repository>, AuthError>;
}
```

**Authentication Flow**:

```
app_token() flow:
1. Check TokenCache for valid JWT
2. If expired/missing, get private key from SecretProvider
3. Generate new JWT via JwtSigner
4. Cache JWT (valid for ~8 minutes)
5. Return JWT

installation_token() flow:
1. Check TokenCache for valid installation token
2. If expired/missing:
   a. Get JWT via app_token()
   b. Call GitHub API to exchange JWT for installation token
   c. Cache installation token (valid for ~55 minutes)
3. Return installation token
```

### SecretProvider (External Trait)

Interface for retrieving GitHub App secrets from secure storage.

```rust
#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get_private_key(&self) -> Result<PrivateKey, SecretError>;
    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError>;
    async fn get_webhook_secret(&self) -> Result<String, SecretError>;

    fn cache_duration(&self) -> Duration;
}

#[derive(Debug, Clone)]
pub struct PrivateKey {
    key_data: Vec<u8>,
    algorithm: KeyAlgorithm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAlgorithm {
    RS256,
}
```

**Contract Requirements**:

- Private keys must never be logged or exposed in errors
- Secrets should be cached for performance (max 5 minutes)
- Must support secret rotation without service restart
- Integration with Azure Key Vault or AWS Secrets Manager

### TokenCache (External Trait)

Interface for caching authentication tokens securely.

```rust
#[async_trait]
pub trait TokenCache: Send + Sync {
    async fn get_jwt(&self, app_id: GitHubAppId) -> Result<Option<JsonWebToken>, CacheError>;

    async fn store_jwt(&self, jwt: JsonWebToken) -> Result<(), CacheError>;

    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<Option<InstallationToken>, CacheError>;

    async fn store_installation_token(
        &self,
        token: InstallationToken,
    ) -> Result<(), CacheError>;

    async fn invalidate_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<(), CacheError>;

    fn cleanup_expired_tokens(&self);
}
```

**Cache Requirements**:

- JWT tokens cached until 2 minutes before expiry
- Installation tokens cached until 5 minutes before expiry
- Automatic cleanup of expired tokens
- Thread-safe concurrent access
- Optional persistence across service restarts

### JwtSigner (External Trait)

Interface for JWT token generation and signing.

```rust
#[async_trait]
pub trait JwtSigner: Send + Sync {
    async fn sign_jwt(
        &self,
        claims: JwtClaims,
        private_key: &PrivateKey,
    ) -> Result<JsonWebToken, SigningError>;

    fn validate_private_key(&self, key: &PrivateKey) -> Result<(), ValidationError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub iss: GitHubAppId,        // Issuer (GitHub App ID)
    pub iat: i64,                // Issued at (Unix timestamp)
    pub exp: i64,                // Expiration (Unix timestamp, max 10 min)
}
```

**Signing Requirements**:

- Must use RS256 algorithm (RSA with SHA-256)
- JWT expiration maximum 10 minutes (GitHub requirement)
- Claims must include iss, iat, exp fields
- Private key validation before signing

## Error Types

### AuthError

Authentication-related errors with retry classification.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid GitHub App credentials")]
    InvalidCredentials,

    #[error("Installation {installation_id} not found or access denied")]
    InstallationNotFound { installation_id: InstallationId },

    #[error("Installation token expired")]
    TokenExpired,

    #[error("Insufficient permissions for operation: {permission}")]
    InsufficientPermissions { permission: String },

    #[error("GitHub API error: {status} - {message}")]
    GitHubApiError { status: u16, message: String },

    #[error("JWT signing failed: {0}")]
    SigningError(#[from] SigningError),

    #[error("Secret retrieval failed: {0}")]
    SecretError(#[from] SecretError),

    #[error("Token cache error: {0}")]
    CacheError(#[from] CacheError),

    #[error("Network error: {0}")]
    NetworkError(String),
}

impl AuthError {
    pub fn is_transient(&self) -> bool;
    pub fn should_retry(&self) -> bool;
    pub fn retry_after(&self) -> Option<Duration>;
}
```

### SecretError

Errors during secret retrieval from secure storage.

```rust
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("Secret not found: {key}")]
    NotFound { key: String },

    #[error("Access denied to secret: {key}")]
    AccessDenied { key: String },

    #[error("Secret provider unavailable: {0}")]
    ProviderUnavailable(String),

    #[error("Invalid secret format: {key}")]
    InvalidFormat { key: String },
}
```

## Authentication Flow Implementation

### JWT Generation Flow

```rust
impl AuthenticationProvider for DefaultAuthProvider {
    async fn generate_jwt(&self) -> Result<JsonWebToken, AuthError> {
        // Check cache first
        let app_id = self.secret_provider.get_app_id().await?;
        if let Some(cached_jwt) = self.token_cache.get_jwt(app_id).await? {
            if !cached_jwt.expires_soon(Duration::minutes(2)) {
                return Ok(cached_jwt);
            }
        }

        // Generate new JWT
        let private_key = self.secret_provider.get_private_key().await?;
        let now = Utc::now();
        let exp = now + Duration::minutes(10); // GitHub max limit

        let claims = JwtClaims {
            iss: app_id,
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };

        let jwt = self.jwt_signer.sign_jwt(claims, &private_key).await?;

        // Cache the new JWT
        self.token_cache.store_jwt(jwt.clone()).await?;

        Ok(jwt)
    }
}
```

### Installation Token Flow

```rust
impl AuthenticationProvider for DefaultAuthProvider {
    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        // Check cache first
        if let Some(cached_token) = self.token_cache
            .get_installation_token(installation_id).await?
        {
            if !cached_token.expires_soon(Duration::minutes(5)) {
                return Ok(cached_token);
            }
        }

        // Generate JWT for API authentication
        let jwt = self.generate_jwt().await?;

        // Exchange JWT for installation token via GitHub API
        let installation_token = self.github_client
            .create_installation_access_token(installation_id, &jwt)
            .await?;

        // Cache the new installation token
        self.token_cache
            .store_installation_token(installation_token.clone())
            .await?;

        Ok(installation_token)
    }
}
```

## Usage Examples

### Basic Authentication Setup

```rust
use github_bot_sdk::auth::{AuthenticationProvider, GitHubAppId, InstallationId};

async fn setup_auth() -> Result<Box<dyn AuthenticationProvider>, AuthError> {
    // Configure secret provider (Azure Key Vault)
    let secret_provider = AzureKeyVaultSecretProvider::new(
        "https://my-keyvault.vault.azure.net"
    ).await?;

    // Configure token cache (in-memory with optional persistence)
    let token_cache = InMemoryTokenCache::new();

    // Configure JWT signer
    let jwt_signer = RS256JwtSigner::new();

    // Create authentication provider
    let auth_provider = DefaultAuthProvider::new(
        secret_provider,
        token_cache,
        jwt_signer,
    );

    Ok(Box::new(auth_provider))
}
```

### Token Usage in API Calls

```rust
async fn make_authenticated_request(
    auth_provider: &dyn AuthenticationProvider,
    installation_id: InstallationId,
) -> Result<(), AuthError> {
    // Get installation token
    let token = auth_provider
        .get_installation_token(installation_id)
        .await?;

    // Verify permissions
    if !token.has_permission(Permission::ReadIssues) {
        return Err(AuthError::InsufficientPermissions {
            permission: "issues:read".to_string(),
        });
    }

    // Use token in API request
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/installation/repositories")
        .header("Authorization", format!("Bearer {}", token.token()))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "MyBot/1.0")
        .send()
        .await?;

    tracing::info!(
        installation_id = %installation_id.as_u64(),
        status = %response.status(),
        "GitHub API request completed"
    );

    Ok(())
}
```

### Token Refresh Strategy

```rust
async fn ensure_valid_token(
    auth_provider: &dyn AuthenticationProvider,
    installation_id: InstallationId,
) -> Result<InstallationToken, AuthError> {
    let token = auth_provider
        .get_installation_token(installation_id)
        .await?;

    // Proactively refresh if expiring soon
    if token.expires_soon(Duration::minutes(10)) {
        tracing::info!(
            installation_id = %installation_id.as_u64(),
            expires_at = %token.expires_at,
            "Refreshing installation token proactively"
        );

        return auth_provider
            .refresh_installation_token(installation_id)
            .await;
    }

    Ok(token)
}
```

## Performance Characteristics

### Token Generation

- JWT generation: < 50ms (including signing)
- Installation token exchange: < 200ms (including GitHub API call)
- Cache lookup: < 1ms
- **Total authentication**: < 250ms (cold) / < 1ms (cached)

### Caching Strategy

- JWT cache TTL: 8 minutes (2 minutes before GitHub expiry)
- Installation token cache TTL: 55 minutes (5 minutes before expiry)
- Cache cleanup interval: 5 minutes
- Memory usage: ~1KB per cached token

### Error Recovery

- GitHub API failures: Retry with exponential backoff
- Secret retrieval failures: Use cached values if available
- Cache failures: Fallback to fresh token generation
- Network timeouts: 30-second timeout with retry

## Security Considerations

### Private Key Protection

- Private keys never logged or included in error messages
- In-memory private keys cleared after use
- Secret rotation supported without service restart
- Access controlled via Azure Key Vault or AWS Secrets Manager

### Token Security

- Installation tokens scoped to specific permissions and repositories
- Tokens automatically refreshed before expiry
- Expired tokens automatically removed from cache
- No token persistence in logs or error messages

### Rate Limiting

- GitHub API rate limits respected (5000 requests/hour)
- JWT generation limited to prevent unnecessary API calls
- Installation token refresh batched when possible
- Circuit breaker protection for repeated failures

This authentication specification provides secure, efficient GitHub App authentication with proper token lifecycle management and comprehensive error handling.
