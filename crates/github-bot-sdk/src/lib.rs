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
//! See `github-bot-sdk-specs/modules/auth.md` for complete specification.
//!
//! # Examples
//!
//! ## Basic Authentication
//!
//! ```rust,no_run
//! use github_bot_sdk::auth::{GitHubAppId, InstallationId};
//!
//! // Create identifiers
//! let app_id = GitHubAppId::new(123456);
//! let installation_id = InstallationId::new(789012);
//!
//! // Use with authentication provider (implementation in later tasks)
//! ```
//!
//! ## Working with Tokens
//!
//! ```rust
//! use github_bot_sdk::auth::{JsonWebToken, GitHubAppId};
//! use chrono::{Utc, Duration};
//!
//! let app_id = GitHubAppId::new(123);
//! let expires_at = Utc::now() + Duration::minutes(10);
//! let jwt = JsonWebToken::new("token".to_string(), app_id, expires_at);
//!
//! // Check expiration
//! if jwt.is_expired() {
//!     println!("Token expired!");
//! }
//!
//! if jwt.expires_soon(Duration::minutes(5)) {
//!     println!("Token expires soon, should refresh");
//! }
//! ```

// Public modules
pub mod auth;
pub mod error;

// Re-export commonly used types at crate root for convenience
pub use error::{ApiError, AuthError, CacheError, SecretError, SigningError, ValidationError};

pub use auth::{
    AuthenticationProvider, GitHubApiClient, GitHubAppId, Installation, InstallationId,
    InstallationPermissions, InstallationToken, JsonWebToken, JwtClaims, JwtSigner, KeyAlgorithm,
    Permission, PermissionLevel, PrivateKey, RateLimitInfo, Repository, RepositoryId,
    RepositorySelection, SecretProvider, TokenCache, User, UserId, UserType,
};
