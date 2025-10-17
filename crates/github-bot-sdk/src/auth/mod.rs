//! GitHub App authentication types and interfaces.
//!
//! This module provides core authentication types for GitHub Apps including:
//! - ID types (GitHubAppId, InstallationId, RepositoryId, UserId)
//! - Token types (JsonWebToken, InstallationToken)
//! - Permission and installation metadata
//! - Authentication trait interfaces
//!
//! See `github-bot-sdk-specs/modules/auth.md` for complete specification.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::error::{ApiError, AuthError, CacheError, SecretError, SigningError, ValidationError};

// ============================================================================
// Core ID Types
// ============================================================================

/// GitHub App identifier assigned during app registration.
///
/// This is a globally unique identifier for your GitHub App, found in the
/// app settings page. It's used for JWT generation and app identification.
///
/// # Examples
///
/// ```
/// use github_bot_sdk::auth::GitHubAppId;
///
/// let app_id = GitHubAppId::new(123456);
/// assert_eq!(app_id.as_u64(), 123456);
/// assert_eq!(app_id.to_string(), "123456");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GitHubAppId(u64);

impl GitHubAppId {
    /// Create a new GitHub App ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use github_bot_sdk::auth::GitHubAppId;
    ///
    /// let app_id = GitHubAppId::new(123456);
    /// ```
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value.
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

/// GitHub App installation identifier for specific accounts.
///
/// When a GitHub App is installed on an organization or user account, GitHub
/// assigns an installation ID. This ID is used to obtain installation tokens
/// and perform operations on behalf of that installation.
///
/// # Examples
///
/// ```
/// use github_bot_sdk::auth::InstallationId;
///
/// let installation = InstallationId::new(98765);
/// assert_eq!(installation.as_u64(), 98765);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallationId(u64);

impl InstallationId {
    /// Create a new installation ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value.
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

/// Repository identifier used by GitHub API.
///
/// This numeric ID uniquely identifies a repository and remains stable even
/// if the repository is renamed or transferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(u64);

impl RepositoryId {
    /// Create a new repository ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value.
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

/// User identifier used by GitHub API.
///
/// This numeric ID uniquely identifies a user or organization and remains
/// stable even if the username changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(u64);

impl UserId {
    /// Create a new user ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value.
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

// ============================================================================
// Token Types
// ============================================================================

/// JWT token for GitHub App authentication.
///
/// JSON Web Tokens (JWTs) are used to authenticate as a GitHub App. They have
/// a maximum lifetime of 10 minutes and are used to obtain installation tokens.
///
/// The token string is never exposed in Debug output for security.
///
/// # Examples
///
/// ```
/// use github_bot_sdk::auth::{JsonWebToken, GitHubAppId};
/// use chrono::{Utc, Duration};
///
/// let app_id = GitHubAppId::new(123);
/// let expires_at = Utc::now() + Duration::minutes(10);
/// let jwt = JsonWebToken::new("encoded.jwt.token".to_string(), app_id, expires_at);
///
/// assert!(!jwt.is_expired());
/// assert_eq!(jwt.app_id(), app_id);
/// ```
#[derive(Clone)]
pub struct JsonWebToken {
    token: String,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    app_id: GitHubAppId,
}

impl JsonWebToken {
    /// Create a new JWT token.
    ///
    /// # Arguments
    ///
    /// * `token` - The encoded JWT string
    /// * `app_id` - The GitHub App ID this token represents
    /// * `expires_at` - When the token expires (max 10 minutes from creation)
    pub fn new(token: String, app_id: GitHubAppId, expires_at: DateTime<Utc>) -> Self {
        let issued_at = Utc::now();
        Self {
            token,
            issued_at,
            expires_at,
            app_id,
        }
    }

    /// Get the token string for use in API requests.
    ///
    /// This should be included in the Authorization header as:
    /// `Authorization: Bearer <token>`
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get the GitHub App ID this token represents.
    pub fn app_id(&self) -> GitHubAppId {
        self.app_id
    }

    /// Get when this token was issued.
    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    /// Get when this token expires.
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Check if the token is currently expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Check if the token will expire soon.
    ///
    /// # Arguments
    ///
    /// * `margin` - How far in the future to check (e.g., 5 minutes)
    ///
    /// Returns true if the token will expire within the margin period.
    pub fn expires_soon(&self, margin: Duration) -> bool {
        Utc::now() + margin >= self.expires_at
    }

    /// Get the time remaining until expiry.
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

/// Installation-scoped access token for GitHub API operations.
///
/// Installation tokens provide access to perform operations on behalf of a
/// specific installation. They have a 1-hour lifetime and include permission
/// and repository scope information.
///
/// The token string is never exposed in Debug output for security.
///
/// # Examples
///
/// ```
/// use github_bot_sdk::auth::{InstallationToken, InstallationId, InstallationPermissions, Permission, RepositoryId};
/// use chrono::{Utc, Duration};
///
/// let installation_id = InstallationId::new(456);
/// let expires_at = Utc::now() + Duration::hours(1);
/// let permissions = InstallationPermissions::default();
/// let repositories = vec![RepositoryId::new(789)];
///
/// let token = InstallationToken::new(
///     "ghs_token".to_string(),
///     installation_id,
///     expires_at,
///     permissions,
///     repositories,
/// );
///
/// assert_eq!(token.installation_id(), installation_id);
/// assert!(!token.is_expired());
/// ```
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
    /// Create a new installation token.
    ///
    /// # Arguments
    ///
    /// * `token` - The token string from GitHub API
    /// * `installation_id` - The installation this token is for
    /// * `expires_at` - When the token expires (typically 1 hour)
    /// * `permissions` - The permissions granted to this token
    /// * `repositories` - The repositories this token can access
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

    /// Get the token string for use in API requests.
    ///
    /// This should be included in the Authorization header as:
    /// `Authorization: Bearer <token>`
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Get the installation ID this token is for.
    pub fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Get when this token was issued.
    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    /// Get when this token expires.
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Get the permissions granted to this token.
    pub fn permissions(&self) -> &InstallationPermissions {
        &self.permissions
    }

    /// Get the repositories this token can access.
    pub fn repositories(&self) -> &[RepositoryId] {
        &self.repositories
    }

    /// Check if the token is currently expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Check if the token will expire soon.
    ///
    /// # Arguments
    ///
    /// * `margin` - How far in the future to check (e.g., 5 minutes)
    ///
    /// Returns true if the token will expire within the margin period.
    pub fn expires_soon(&self, margin: Duration) -> bool {
        Utc::now() + margin >= self.expires_at
    }

    /// Check if the token has a specific permission.
    ///
    /// # Examples
    ///
    /// ```
    /// # use github_bot_sdk::auth::{InstallationToken, InstallationId, InstallationPermissions, Permission, PermissionLevel, RepositoryId};
    /// # use chrono::{Utc, Duration};
    /// let mut permissions = InstallationPermissions::default();
    /// permissions.issues = PermissionLevel::Write;
    ///
    /// let token = InstallationToken::new(
    ///     "token".to_string(),
    ///     InstallationId::new(1),
    ///     Utc::now() + Duration::hours(1),
    ///     permissions,
    ///     vec![],
    /// );
    ///
    /// assert!(token.has_permission(Permission::ReadIssues));
    /// assert!(token.has_permission(Permission::WriteIssues));
    /// assert!(!token.has_permission(Permission::WriteContents));
    /// ```
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

    /// Check if the token can access a specific repository.
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

// ============================================================================
// Permission Types
// ============================================================================

/// Permissions granted to a GitHub App installation.
///
/// Each permission can be set to None, Read, Write, or Admin level.
/// See GitHub's documentation for details on what each permission allows.
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

/// Permission level for GitHub resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    None,
    Read,
    Write,
    Admin,
}

/// Specific permissions that can be checked on tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

// ============================================================================
// Supporting Types
// ============================================================================

/// User type classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum UserType {
    User,
    Bot,
    Organization,
}

/// User information from GitHub API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub login: String,
    #[serde(rename = "type")]
    pub user_type: UserType,
    pub avatar_url: Option<String>,
    pub html_url: String,
}

/// Repository information from GitHub API.
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
    /// Create a new repository.
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

    /// Get repository owner name.
    pub fn owner_name(&self) -> &str {
        &self.owner.login
    }

    /// Get repository name without owner.
    pub fn repo_name(&self) -> &str {
        &self.name
    }

    /// Get full repository name (owner/name).
    pub fn full_name(&self) -> &str {
        &self.full_name
    }
}

/// Installation information from GitHub API.
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

/// Repository selection for an installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepositorySelection {
    All,
    Selected,
}

/// Private key for JWT signing.
///
/// Stores the cryptographic key material for signing JWTs. The key data
/// is never exposed in Debug output for security.
#[derive(Clone)]
pub struct PrivateKey {
    key_data: Vec<u8>,
    algorithm: KeyAlgorithm,
}

impl PrivateKey {
    /// Create a new private key.
    ///
    /// # Arguments
    ///
    /// * `key_data` - The raw key bytes (PEM or DER format)
    /// * `algorithm` - The signing algorithm (typically RS256)
    pub fn new(key_data: Vec<u8>, algorithm: KeyAlgorithm) -> Self {
        Self {
            key_data,
            algorithm,
        }
    }

    /// Get the key data.
    pub fn key_data(&self) -> &[u8] {
        &self.key_data
    }

    /// Get the signing algorithm.
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

/// Key algorithm for JWT signing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAlgorithm {
    RS256,
}

/// JWT claims structure for GitHub App authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Issuer (GitHub App ID)
    pub iss: GitHubAppId,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Expiration (Unix timestamp, max 10 minutes from iat)
    pub exp: i64,
}

/// Rate limit information from GitHub API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: DateTime<Utc>,
    pub used: u32,
}

// ============================================================================
// Trait Definitions (Interfaces for later tasks)
// ============================================================================

/// Main interface for GitHub App authentication operations.
#[async_trait::async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Generate JWT token for GitHub App authentication.
    async fn generate_jwt(&self) -> Result<JsonWebToken, AuthError>;

    /// Get installation token for API operations.
    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// Refresh installation token (force new token).
    async fn refresh_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<InstallationToken, AuthError>;

    /// List all installations for this GitHub App.
    async fn list_installations(&self) -> Result<Vec<Installation>, AuthError>;

    /// Get repositories accessible by installation.
    async fn get_installation_repositories(
        &self,
        installation_id: InstallationId,
    ) -> Result<Vec<Repository>, AuthError>;
}

/// Interface for retrieving GitHub App secrets from secure storage.
#[async_trait::async_trait]
pub trait SecretProvider: Send + Sync {
    /// Get private key for JWT signing.
    async fn get_private_key(&self) -> Result<PrivateKey, SecretError>;

    /// Get GitHub App ID.
    async fn get_app_id(&self) -> Result<GitHubAppId, SecretError>;

    /// Get webhook secret for signature validation.
    async fn get_webhook_secret(&self) -> Result<String, SecretError>;

    /// Get cache duration for secrets.
    fn cache_duration(&self) -> Duration;
}

/// Interface for caching authentication tokens securely.
#[async_trait::async_trait]
pub trait TokenCache: Send + Sync {
    /// Get cached JWT token.
    async fn get_jwt(&self, app_id: GitHubAppId) -> Result<Option<JsonWebToken>, CacheError>;

    /// Store JWT token in cache.
    async fn store_jwt(&self, jwt: JsonWebToken) -> Result<(), CacheError>;

    /// Get cached installation token.
    async fn get_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<Option<InstallationToken>, CacheError>;

    /// Store installation token in cache.
    async fn store_installation_token(&self, token: InstallationToken) -> Result<(), CacheError>;

    /// Invalidate installation token.
    async fn invalidate_installation_token(
        &self,
        installation_id: InstallationId,
    ) -> Result<(), CacheError>;

    /// Cleanup expired tokens.
    fn cleanup_expired_tokens(&self);
}

/// Interface for JWT token generation and signing.
#[async_trait::async_trait]
pub trait JwtSigner: Send + Sync {
    /// Sign JWT with private key.
    async fn sign_jwt(
        &self,
        claims: JwtClaims,
        private_key: &PrivateKey,
    ) -> Result<JsonWebToken, SigningError>;

    /// Validate private key format.
    fn validate_private_key(&self, key: &PrivateKey) -> Result<(), ValidationError>;
}

/// Interface for GitHub API client operations.
#[async_trait::async_trait]
pub trait GitHubApiClient: Send + Sync {
    /// Create installation access token via GitHub API.
    async fn create_installation_access_token(
        &self,
        installation_id: InstallationId,
        jwt: &JsonWebToken,
    ) -> Result<InstallationToken, ApiError>;

    /// List installations for the GitHub App.
    async fn list_app_installations(
        &self,
        jwt: &JsonWebToken,
    ) -> Result<Vec<Installation>, ApiError>;

    /// Get repositories for installation.
    async fn list_installation_repositories(
        &self,
        installation_id: InstallationId,
        token: &InstallationToken,
    ) -> Result<Vec<Repository>, ApiError>;

    /// Get repository information.
    async fn get_repository(
        &self,
        repo_id: RepositoryId,
        token: &InstallationToken,
    ) -> Result<Repository, ApiError>;

    /// Check API rate limits.
    async fn get_rate_limit(&self, token: &InstallationToken) -> Result<RateLimitInfo, ApiError>;
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
