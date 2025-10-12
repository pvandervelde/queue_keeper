//! # GitHub Bot SDK
//!
//! Software Development Kit for GitHub Bot integration with App authentication,
//! API client abstractions, and webhook processing.
//!
//! This SDK provides:
//! - GitHub App authentication with JWT and installation tokens
//! - API client with rate limiting and retry logic
//! - Webhook signature validation
//! - Repository and installation management
//!
//! See specs/interfaces/github-auth.md for complete specification.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

// ============================================================================
// Core Types
// ============================================================================

/// GitHub App identifier assigned during app registration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GitHubAppId(u64);

impl GitHubAppId {
    /// Create new GitHub App ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get raw ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for GitHubAppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for GitHubAppId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .parse::<u64>()
            .map_err(|_| ValidationError::InvalidFormat {
                field: "github_app_id".to_string(),
                message: "must be a positive integer".to_string(),
            })?;
        Ok(Self::new(id))
    }
}

/// GitHub App installation identifier for specific org/user accounts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallationId(u64);

impl InstallationId {
    /// Create new installation ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get raw ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for InstallationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for InstallationId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .parse::<u64>()
            .map_err(|_| ValidationError::InvalidFormat {
                field: "installation_id".to_string(),
                message: "must be a positive integer".to_string(),
            })?;
        Ok(Self::new(id))
    }
}

/// Repository identifier used by GitHub API
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(u64);

impl RepositoryId {
    /// Create new repository ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get raw ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for RepositoryId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .parse::<u64>()
            .map_err(|_| ValidationError::InvalidFormat {
                field: "repository_id".to_string(),
                message: "must be a positive integer".to_string(),
            })?;
        Ok(Self::new(id))
    }
}

/// User identifier used by GitHub API
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(u64);

impl UserId {
    /// Create new user ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get raw ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for UserId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .parse::<u64>()
            .map_err(|_| ValidationError::InvalidFormat {
                field: "user_id".to_string(),
                message: "must be a positive integer".to_string(),
            })?;
        Ok(Self::new(id))
    }
}

/// User type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserType {
    User,
    Bot,
    Organization,
}

/// User information from GitHub API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub login: String,
    pub user_type: UserType,
    pub avatar_url: Option<String>,
    pub html_url: String,
}

/// Repository information from GitHub API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: RepositoryId,
    pub name: String,
    pub full_name: String,
    pub owner: User,
    pub private: bool,
    pub html_url: String,
    pub default_branch: String,
}

impl Repository {
    /// Create new repository
    pub fn new(
        id: RepositoryId,
        name: String,
        full_name: String,
        owner: User,
        private: bool,
    ) -> Self {
        Self {
            id,
            name: name.clone(),
            full_name: full_name.clone(),
            owner,
            private,
            html_url: format!("https://github.com/{}", full_name),
            default_branch: "main".to_string(), // Default assumption
        }
    }

    /// Get repository owner name
    pub fn owner_name(&self) -> &str {
        &self.owner.login
    }

    /// Get repository name without owner
    pub fn repo_name(&self) -> &str {
        &self.name
    }

    /// Get full repository name (owner/name)
    pub fn full_name(&self) -> &str {
        &self.full_name
    }
}

/// Installation information from GitHub API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installation {
    pub id: InstallationId,
    pub account: User,
    pub repository_selection: RepositorySelection,
    pub permissions: InstallationPermissions,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub suspended_at: Option<DateTime<Utc>>,
}

/// Repository selection for installation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepositorySelection {
    All,
    Selected,
}

// ============================================================================
// Authentication Types
// ============================================================================

/// JWT token for GitHub App authentication (short-lived, 10 minutes max)
#[derive(Clone)]
pub struct JsonWebToken {
    token: String,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    app_id: GitHubAppId,
}

impl JsonWebToken {
    /// Create new JWT token
    pub fn new(token: String, app_id: GitHubAppId, expires_at: DateTime<Utc>) -> Self {
        let issued_at = Utc::now();
        Self {
            token,
            issued_at,
            expires_at,
            app_id,
        }
    }

    /// Get token string
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get app ID
    pub fn app_id(&self) -> GitHubAppId {
        self.app_id
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Check if token expires soon
    pub fn expires_soon(&self, margin: Duration) -> bool {
        Utc::now() + margin >= self.expires_at
    }

    /// Get time until expiry
    pub fn time_until_expiry(&self) -> Duration {
        self.expires_at - Utc::now()
    }
}

// Security: Don't expose token in debug output
impl std::fmt::Debug for JsonWebToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonWebToken")
            .field("app_id", &self.app_id)
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("token", &"<REDACTED>")
            .finish()
    }
}

/// Installation-scoped access token for GitHub API operations
#[derive(Clone)]
pub struct InstallationToken {
    token: String,
    installation_id: InstallationId,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    permissions: InstallationPermissions,
    repositories: Vec<RepositoryId>,
}

impl InstallationToken {
    /// Create new installation token
    pub fn new(
        token: String,
        installation_id: InstallationId,
        expires_at: DateTime<Utc>,
        permissions: InstallationPermissions,
        repositories: Vec<RepositoryId>,
    ) -> Self {
        let issued_at = Utc::now();
        Self {
            token,
            installation_id,
            issued_at,
            expires_at,
            permissions,
            repositories,
        }
    }

    /// Get token string
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get installation ID
    pub fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Get expiration time
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Check if token expires soon
    pub fn expires_soon(&self, margin: Duration) -> bool {
        Utc::now() + margin >= self.expires_at
    }

    /// Check if token has specific permission
    pub fn has_permission(&self, permission: Permission) -> bool {
        match permission {
            Permission::ReadIssues => matches!(
                self.permissions.issues,
                PermissionLevel::Read | PermissionLevel::Write | PermissionLevel::Admin
            ),
            Permission::WriteIssues => matches!(
                self.permissions.issues,
                PermissionLevel::Write | PermissionLevel::Admin
            ),
            Permission::ReadPullRequests => matches!(
                self.permissions.pull_requests,
                PermissionLevel::Read | PermissionLevel::Write | PermissionLevel::Admin
            ),
            Permission::WritePullRequests => matches!(
                self.permissions.pull_requests,
                PermissionLevel::Write | PermissionLevel::Admin
            ),
            Permission::ReadContents => matches!(
                self.permissions.contents,
                PermissionLevel::Read | PermissionLevel::Write | PermissionLevel::Admin
            ),
            Permission::WriteContents => matches!(
                self.permissions.contents,
                PermissionLevel::Write | PermissionLevel::Admin
            ),
            Permission::ReadChecks => matches!(
                self.permissions.checks,
                PermissionLevel::Read | PermissionLevel::Write | PermissionLevel::Admin
            ),
            Permission::WriteChecks => matches!(
                self.permissions.checks,
                PermissionLevel::Write | PermissionLevel::Admin
            ),
        }
    }

    /// Check if token can access specific repository
    pub fn can_access_repository(&self, repo_id: RepositoryId) -> bool {
        self.repositories.contains(&repo_id)
    }
}

// Security: Redact token in debug output
impl std::fmt::Debug for InstallationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallationToken")
            .field("installation_id", &self.installation_id)
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("permissions", &self.permissions)
            .field("repositories", &self.repositories)
            .field("token", &"<REDACTED>")
            .finish()
    }
}

/// Permissions granted to the GitHub App installation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallationPermissions {
    pub issues: PermissionLevel,
    pub pull_requests: PermissionLevel,
    pub contents: PermissionLevel,
    pub metadata: PermissionLevel,
    pub checks: PermissionLevel,
    pub actions: PermissionLevel,
}

impl Default for InstallationPermissions {
    fn default() -> Self {
        Self {
            issues: PermissionLevel::None,
            pull_requests: PermissionLevel::None,
            contents: PermissionLevel::None,
            metadata: PermissionLevel::Read, // Usually granted by default
            checks: PermissionLevel::None,
            actions: PermissionLevel::None,
        }
    }
}

/// Permission level for specific GitHub resources
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    None,
    Read,
    Write,
    Admin,
}

/// Specific permissions that can be checked
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

/// Private key for JWT signing
#[derive(Clone)]
pub struct PrivateKey {
    key_data: Vec<u8>,
    algorithm: KeyAlgorithm,
}

impl PrivateKey {
    /// Create new private key
    pub fn new(key_data: Vec<u8>, algorithm: KeyAlgorithm) -> Self {
        Self {
            key_data,
            algorithm,
        }
    }

    /// Get key data
    pub fn key_data(&self) -> &[u8] {
        &self.key_data
    }

    /// Get algorithm
    pub fn algorithm(&self) -> &KeyAlgorithm {
        &self.algorithm
    }
}

// Security: Don't expose key data in debug output
impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateKey")
            .field("algorithm", &self.algorithm)
            .field("key_data", &"<REDACTED>")
            .finish()
    }
}

/// Key algorithm for JWT signing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAlgorithm {
    RS256,
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub iss: GitHubAppId, // Issuer (GitHub App ID)
    pub iat: i64,         // Issued at (Unix timestamp)
    pub exp: i64,         // Expiration (Unix timestamp, max 10 min)
}

// ============================================================================
// Core Trait Definitions
// ============================================================================

/// Main interface for GitHub App authentication operations
#[async_trait::async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Generate JWT token for GitHub App authentication
    async fn generate_jwt(&self) -> Result<JsonWebToken, AuthError>;

    /// Get installation token for API operations
    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// Refresh installation token (force new token)
    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// List all installations for this GitHub App
    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError>;

    /// Get repositories accessible by installation
    async fn get_installation_repositories(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<Repository>, AuthError>;
}

/// Interface for retrieving GitHub App secrets from secure storage
#[async_trait::async_trait]
pub trait SecretProvider: Send + Sync {
    /// Get private key for JWT signing
    async fn get_private_key(&self) -> Result<PrivateKey, SecretError>;

    /// Get GitHub App ID
    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError>;

    /// Get webhook secret for signature validation
    async fn get_webhook_secret(&self) -> Result<String, SecretError>;

    /// Get cache duration for secrets
    fn cache_duration(&self) -> Duration;
}

/// Interface for caching authentication tokens securely
#[async_trait::async_trait]
pub trait TokenCache: Send + Sync {
    /// Get cached JWT token
    async fn get_jwt(&self, app_id: GitHubAppId) -> Result<Option<JsonWebToken>, CacheError>;

    /// Store JWT token in cache
    async fn store_jwt(&self, jwt: JsonWebToken) -> Result<(), CacheError>;

    /// Get cached installation token
    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<Option<InstallationToken>, CacheError>;

    /// Store installation token in cache
    async fn store_installation_token(&self, token: InstallationToken) -> Result<(), CacheError>;

    /// Invalidate installation token
    async fn invalidate_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<(), CacheError>;

    /// Cleanup expired tokens
    fn cleanup_expired_tokens(&self);
}

/// Interface for JWT token generation and signing
#[async_trait::async_trait]
pub trait JwtSigner: Send + Sync {
    /// Sign JWT with private key
    async fn sign_jwt(
        &self,
        claims: JwtClaims,
        private_key: &PrivateKey,
    ) -> Result<JsonWebToken, SigningError>;

    /// Validate private key format
    fn validate_private_key(&self, key: &PrivateKey) -> Result<(), ValidationError>;
}

/// Interface for GitHub API client operations
#[async_trait::async_trait]
pub trait GitHubApiClient: Send + Sync {
    /// Create installation access token via GitHub API
    async fn create_installation_access_token(
        &self,
        installation_id: InstallationId,
        jwt: &JsonWebToken,
    ) -> Result<InstallationToken, ApiError>;

    /// List installations for the GitHub App
    async fn list_app_installations(
        &self,
        jwt: &JsonWebToken,
    ) -> Result<Vec<Installation>, ApiError>;

    /// Get repositories for installation
    async fn list_installation_repositories(
        &self,
        installation_id: InstallationId,
        token: &InstallationToken,
    ) -> Result<Vec<Repository>, ApiError>;

    /// Get repository information
    async fn get_repository(
        &self,
        repo_id: RepositoryId,
        token: &InstallationToken,
    ) -> Result<Repository, ApiError>;

    /// Check API rate limits
    async fn get_rate_limit(&self, token: &InstallationToken) -> Result<RateLimitInfo, ApiError>;
}

// ============================================================================
// Error Types
// ============================================================================

/// Authentication-related errors with retry classification
#[derive(Debug, Error)]
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

    #[error("API error: {0}")]
    ApiError(#[from] ApiError),
}

impl AuthError {
    /// Check if error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        match self {
            Self::InvalidCredentials => false,
            Self::InstallationNotFound { .. } => false,
            Self::TokenExpired => true, // Can refresh token
            Self::InsufficientPermissions { .. } => false,
            Self::GitHubApiError { status, .. } => *status >= 500 || *status == 429,
            Self::SigningError(_) => false,
            Self::SecretError(e) => e.is_transient(),
            Self::CacheError(_) => true, // Can fallback to fresh generation
            Self::NetworkError(_) => true,
            Self::ApiError(e) => e.is_transient(),
        }
    }

    /// Check if error should be retried
    pub fn should_retry(&self) -> bool {
        self.is_transient()
    }

    /// Get retry delay duration
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::GitHubApiError { status, .. } if *status == 429 => Some(Duration::minutes(1)),
            Self::NetworkError(_) => Some(Duration::seconds(5)),
            _ => None,
        }
    }
}

/// Errors during secret retrieval from secure storage
#[derive(Debug, Error)]
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

impl SecretError {
    /// Check if error is transient
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::ProviderUnavailable(_))
    }
}

/// Errors during token caching operations
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Cache operation failed: {message}")]
    OperationFailed { message: String },

    #[error("Cache unavailable: {message}")]
    Unavailable { message: String },

    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Errors during JWT signing
#[derive(Debug, Error)]
pub enum SigningError {
    #[error("Invalid private key: {message}")]
    InvalidKey { message: String },

    #[error("Signing operation failed: {message}")]
    SigningFailed { message: String },

    #[error("Token encoding failed: {message}")]
    EncodingFailed { message: String },
}

/// Errors during GitHub API operations
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP error: {status} - {message}")]
    HttpError { status: u16, message: String },

    #[error("Rate limit exceeded. Reset at: {reset_at}")]
    RateLimitExceeded { reset_at: DateTime<Utc> },

    #[error("Request timeout")]
    Timeout,

    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Authorization failed")]
    AuthorizationFailed,

    #[error("Resource not found")]
    NotFound,

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    HttpClientError(#[from] reqwest::Error),
}

impl ApiError {
    /// Check if error is transient
    pub fn is_transient(&self) -> bool {
        match self {
            Self::HttpError { status, .. } => *status >= 500 || *status == 429,
            Self::RateLimitExceeded { .. } => true,
            Self::Timeout => true,
            Self::InvalidRequest { .. } => false,
            Self::AuthenticationFailed => false,
            Self::AuthorizationFailed => false,
            Self::NotFound => false,
            Self::JsonError(_) => false,
            Self::HttpClientError(_) => true, // Network issues are transient
        }
    }
}

/// Validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Required field missing: {field}")]
    Required { field: String },

    #[error("Invalid format for {field}: {message}")]
    InvalidFormat { field: String, message: String },

    #[error("Value out of range for {field}: {message}")]
    OutOfRange { field: String, message: String },
}

/// Rate limit information from GitHub API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: DateTime<Utc>,
    pub used: u32,
}

// ============================================================================
// Default Implementations (Stubs)
// ============================================================================

/// Default authentication provider implementation
pub struct DefaultAuthProvider;

#[async_trait::async_trait]
impl AuthenticationProvider for DefaultAuthProvider {
    async fn generate_jwt(&self) -> Result<JsonWebToken, AuthError> {
        // TODO: Implement JWT generation
        // See specs/interfaces/github-auth.md
        unimplemented!("JWT generation not yet implemented")
    }

    async fn get_installation_token(
        &self,
        _installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        // TODO: Implement installation token retrieval
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation token retrieval not yet implemented")
    }

    async fn refresh_installation_token(
        &self,
        _installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError> {
        // TODO: Implement installation token refresh
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation token refresh not yet implemented")
    }

    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError> {
        // TODO: Implement installation listing
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation listing not yet implemented")
    }

    async fn get_installation_repositories(
        &self,
        _installation_id: InstallationId,
    ) -> Result<Vec<Repository>, AuthError> {
        // TODO: Implement repository listing
        // See specs/interfaces/github-auth.md
        unimplemented!("Repository listing not yet implemented")
    }
}

/// Default JWT signer implementation
pub struct DefaultJwtSigner;

#[async_trait::async_trait]
impl JwtSigner for DefaultJwtSigner {
    async fn sign_jwt(
        &self,
        _claims: JwtClaims,
        _private_key: &PrivateKey,
    ) -> Result<JsonWebToken, SigningError> {
        // TODO: Implement JWT signing with RS256
        // See specs/interfaces/github-auth.md
        unimplemented!("JWT signing not yet implemented")
    }

    fn validate_private_key(&self, _key: &PrivateKey) -> Result<(), ValidationError> {
        // TODO: Implement private key validation
        // See specs/interfaces/github-auth.md
        unimplemented!("Private key validation not yet implemented")
    }
}

/// Default GitHub API client implementation
pub struct DefaultGitHubApiClient;

#[async_trait::async_trait]
impl GitHubApiClient for DefaultGitHubApiClient {
    async fn create_installation_access_token(
        &self,
        _installation_id: InstallationId,
        _jwt: &JsonWebToken,
    ) -> Result<InstallationToken, ApiError> {
        // TODO: Implement installation token creation via GitHub API
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation token creation not yet implemented")
    }

    async fn list_app_installations(
        &self,
        _jwt: &JsonWebToken,
    ) -> Result<Vec<Installation>, ApiError> {
        // TODO: Implement installation listing via GitHub API
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation listing not yet implemented")
    }

    async fn list_installation_repositories(
        &self,
        _installation_id: InstallationId,
        _token: &InstallationToken,
    ) -> Result<Vec<Repository>, ApiError> {
        // TODO: Implement repository listing via GitHub API
        // See specs/interfaces/github-auth.md
        unimplemented!("Repository listing not yet implemented")
    }

    async fn get_repository(
        &self,
        _repo_id: RepositoryId,
        _token: &InstallationToken,
    ) -> Result<Repository, ApiError> {
        // TODO: Implement repository retrieval via GitHub API
        // See specs/interfaces/github-auth.md
        unimplemented!("Repository retrieval not yet implemented")
    }

    async fn get_rate_limit(&self, _token: &InstallationToken) -> Result<RateLimitInfo, ApiError> {
        // TODO: Implement rate limit checking via GitHub API
        // See specs/interfaces/github-auth.md
        unimplemented!("Rate limit checking not yet implemented")
    }
}

/// In-memory token cache implementation
pub struct InMemoryTokenCache;

#[async_trait::async_trait]
impl TokenCache for InMemoryTokenCache {
    async fn get_jwt(&self, _app_id: GitHubAppId) -> Result<Option<JsonWebToken>, CacheError> {
        // TODO: Implement in-memory JWT caching
        // See specs/interfaces/github-auth.md
        unimplemented!("JWT caching not yet implemented")
    }

    async fn store_jwt(&self, _jwt: JsonWebToken) -> Result<(), CacheError> {
        // TODO: Implement JWT storage
        // See specs/interfaces/github-auth.md
        unimplemented!("JWT storage not yet implemented")
    }

    async fn get_installation_token(
        &self,
        _installation_id: InstallationId,
    ) -> Result<Option<InstallationToken>, CacheError> {
        // TODO: Implement installation token caching
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation token caching not yet implemented")
    }

    async fn store_installation_token(&self, _token: InstallationToken) -> Result<(), CacheError> {
        // TODO: Implement installation token storage
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation token storage not yet implemented")
    }

    async fn invalidate_installation_token(
        &self,
        _installation_id: InstallationId,
    ) -> Result<(), CacheError> {
        // TODO: Implement installation token invalidation
        // See specs/interfaces/github-auth.md
        unimplemented!("Installation token invalidation not yet implemented")
    }

    fn cleanup_expired_tokens(&self) {
        // TODO: Implement expired token cleanup
        // See specs/interfaces/github-auth.md
        unimplemented!("Token cleanup not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_app_id() {
        let app_id = GitHubAppId::new(12345);
        assert_eq!(app_id.as_u64(), 12345);
        assert_eq!(app_id.to_string(), "12345");
    }

    #[test]
    fn test_installation_id() {
        let installation_id = InstallationId::new(67890);
        assert_eq!(installation_id.as_u64(), 67890);
        assert_eq!(installation_id.to_string(), "67890");
    }

    #[test]
    fn test_jwt_expiry() {
        let app_id = GitHubAppId::new(1);
        let expires_at = Utc::now() + Duration::minutes(5);
        let jwt = JsonWebToken::new("test_token".to_string(), app_id, expires_at);

        assert!(!jwt.is_expired());
        assert!(jwt.expires_soon(Duration::minutes(10))); // Expires in 5 min, checking 10 min margin
        assert!(!jwt.expires_soon(Duration::minutes(2))); // Doesn't expire in 2 min
    }

    #[test]
    fn test_permission_checking() {
        let permissions = InstallationPermissions {
            issues: PermissionLevel::Read,
            pull_requests: PermissionLevel::Write,
            contents: PermissionLevel::None,
            metadata: PermissionLevel::Read,
            checks: PermissionLevel::Admin,
            actions: PermissionLevel::None,
        };

        let token = InstallationToken::new(
            "test_token".to_string(),
            InstallationId::new(1),
            Utc::now() + Duration::hours(1),
            permissions,
            vec![RepositoryId::new(123)],
        );

        assert!(token.has_permission(Permission::ReadIssues));
        assert!(!token.has_permission(Permission::WriteIssues));
        assert!(token.has_permission(Permission::ReadPullRequests));
        assert!(token.has_permission(Permission::WritePullRequests));
        assert!(!token.has_permission(Permission::ReadContents));
        assert!(token.has_permission(Permission::WriteChecks));

        assert!(token.can_access_repository(RepositoryId::new(123)));
        assert!(!token.can_access_repository(RepositoryId::new(456)));
    }

    #[test]
    fn test_error_transience() {
        assert!(AuthError::NetworkError("timeout".to_string()).is_transient());
        assert!(!AuthError::InvalidCredentials.is_transient());
        assert!(AuthError::GitHubApiError {
            status: 500,
            message: "server error".to_string()
        }
        .is_transient());
        assert!(!AuthError::GitHubApiError {
            status: 400,
            message: "bad request".to_string()
        }
        .is_transient());
    }
}
