//! Tests for authentication module types and traits.

use super::*;

/// Verify GitHubAppId creation, conversion, and parsing.
///
/// Tests the GitHubAppId newtype to ensure:
/// - Creation via `new()` stores the correct value
/// - Conversion to u64 and string formats work correctly
/// - String parsing succeeds for valid numeric input
/// - String parsing fails for invalid non-numeric input
#[test]
fn test_github_app_id() {
    let app_id = GitHubAppId::new(12345);
    assert_eq!(app_id.as_u64(), 12345);
    assert_eq!(app_id.to_string(), "12345");

    let parsed: GitHubAppId = "67890".parse().unwrap();
    assert_eq!(parsed.as_u64(), 67890);

    let invalid = "not_a_number".parse::<GitHubAppId>();
    assert!(invalid.is_err());
}

/// Verify InstallationId creation, conversion, and parsing.
///
/// Tests the InstallationId newtype to ensure:
/// - Creation via `new()` stores the correct value
/// - Conversion to u64 and string formats work correctly
/// - String parsing succeeds for valid numeric input
#[test]
fn test_installation_id() {
    let installation = InstallationId::new(98765);
    assert_eq!(installation.as_u64(), 98765);
    assert_eq!(installation.to_string(), "98765");

    let parsed: InstallationId = "11111".parse().unwrap();
    assert_eq!(parsed.as_u64(), 11111);
}

/// Verify RepositoryId creation and conversion.
///
/// Tests the RepositoryId newtype to ensure:
/// - Creation via `new()` stores the correct value
/// - Conversion to u64 and string formats work correctly
#[test]
fn test_repository_id() {
    let repo = RepositoryId::new(54321);
    assert_eq!(repo.as_u64(), 54321);
    assert_eq!(repo.to_string(), "54321");
}

/// Verify UserId creation and conversion.
///
/// Tests the UserId newtype to ensure:
/// - Creation via `new()` stores the correct value
/// - Conversion to u64 and string formats work correctly
#[test]
fn test_user_id() {
    let user = UserId::new(99999);
    assert_eq!(user.as_u64(), 99999);
    assert_eq!(user.to_string(), "99999");
}

/// Verify JsonWebToken expiry detection and validation.
///
/// Tests JWT token expiry logic to ensure:
/// - Tokens with future expiry are not marked as expired
/// - `expires_soon()` correctly detects when expiry is within the margin
/// - `expires_soon()` returns false when sufficient time remains
/// - Accessor methods return correct app_id and token values
#[test]
fn test_jwt_token_expiry() {
    let app_id = GitHubAppId::new(1);
    let expires_at = Utc::now() + Duration::minutes(5);
    let jwt = JsonWebToken::new("test_token".to_string(), app_id, expires_at);

    assert!(!jwt.is_expired());
    assert!(jwt.expires_soon(Duration::minutes(10))); // Expires in 5 min, checking 10 min margin
    assert!(!jwt.expires_soon(Duration::minutes(2))); // Doesn't expire in 2 min
    assert_eq!(jwt.app_id(), app_id);
    assert_eq!(jwt.token(), "test_token");
}

/// Verify that JsonWebToken does not leak secrets in Debug output.
///
/// Tests the custom Debug implementation to ensure:
/// - Token value is redacted (not visible in debug output)
/// - Debug output contains "<REDACTED>" placeholder instead
/// - Prevents accidental logging of sensitive JWT tokens
#[test]
fn test_jwt_token_security() {
    let app_id = GitHubAppId::new(1);
    let jwt = JsonWebToken::new(
        "secret_token".to_string(),
        app_id,
        Utc::now() + Duration::minutes(10),
    );

    let debug_output = format!("{:?}", jwt);
    assert!(!debug_output.contains("secret_token"));
    assert!(debug_output.contains("<REDACTED>"));
}

/// Verify InstallationToken permission checking and repository access control.
///
/// Tests permission and repository access logic to ensure:
/// - `has_permission()` correctly evaluates permissions based on level hierarchy
/// - Read permission allows read operations but not write
/// - Write permission allows both read and write operations
/// - Admin permission allows all operations
/// - `can_access_repository()` correctly checks repository list
#[test]
fn test_installation_token_permissions() {
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

/// Verify that InstallationToken does not leak secrets in Debug output.
///
/// Tests the custom Debug implementation to ensure:
/// - Token value is redacted (not visible in debug output)
/// - Debug output contains "<REDACTED>" placeholder instead
/// - Prevents accidental logging of sensitive installation tokens
#[test]
fn test_installation_token_security() {
    let token = InstallationToken::new(
        "secret_installation_token".to_string(),
        InstallationId::new(1),
        Utc::now() + Duration::hours(1),
        InstallationPermissions::default(),
        vec![],
    );

    let debug_output = format!("{:?}", token);
    assert!(!debug_output.contains("secret_installation_token"));
    assert!(debug_output.contains("<REDACTED>"));
}

/// Verify that PrivateKey does not leak key material in Debug output.
///
/// Tests the custom Debug implementation to ensure:
/// - Private key bytes are redacted (not visible in debug output)
/// - Debug output contains "<REDACTED>" placeholder instead
/// - Algorithm information is safely accessible
/// - Prevents accidental logging of sensitive cryptographic material
#[test]
fn test_private_key_security() {
    let key = PrivateKey::new(b"super_secret_key_material".to_vec(), KeyAlgorithm::RS256);

    let debug_output = format!("{:?}", key);
    assert!(!debug_output.contains("super_secret_key_material"));
    assert!(debug_output.contains("<REDACTED>"));
    assert_eq!(key.algorithm(), &KeyAlgorithm::RS256);
}

/// Verify PermissionLevel enum variants are distinct.
///
/// Tests that different PermissionLevel values are properly distinguished
/// through inequality checks (None ≠ Read ≠ Write ≠ Admin).
#[test]
fn test_permission_level_ordering() {
    assert!(PermissionLevel::Read != PermissionLevel::None);
    assert!(PermissionLevel::Write != PermissionLevel::Read);
    assert!(PermissionLevel::Admin != PermissionLevel::Write);
}

/// Verify Repository helper methods for name extraction.
///
/// Tests Repository utility methods to ensure:
/// - `owner_name()` correctly extracts owner from full_name
/// - `repo_name()` correctly extracts repository name from full_name
/// - `full_name()` returns the complete "owner/repo" format
/// - `html_url` is correctly constructed from full_name
#[test]
fn test_repository_helpers() {
    let owner = User {
        id: UserId::new(1),
        login: "octocat".to_string(),
        user_type: UserType::User,
        avatar_url: Some("https://github.com/octocat.png".to_string()),
        html_url: "https://github.com/octocat".to_string(),
    };

    let repo = Repository::new(
        RepositoryId::new(123),
        "hello-world".to_string(),
        "octocat/hello-world".to_string(),
        owner,
        false,
    );

    assert_eq!(repo.owner_name(), "octocat");
    assert_eq!(repo.repo_name(), "hello-world");
    assert_eq!(repo.full_name(), "octocat/hello-world");
    assert_eq!(repo.html_url, "https://github.com/octocat/hello-world");
}

/// Verify InstallationPermissions default values.
///
/// Tests the Default trait implementation to ensure:
/// - All permissions default to None
/// - This aligns with GitHub API where missing permissions mean no access
#[test]
fn test_default_permissions() {
    let perms = InstallationPermissions::default();
    assert_eq!(perms.issues, PermissionLevel::None);
    assert_eq!(perms.pull_requests, PermissionLevel::None);
    assert_eq!(perms.contents, PermissionLevel::None);
    assert_eq!(perms.metadata, PermissionLevel::None);
    assert_eq!(perms.checks, PermissionLevel::None);
    assert_eq!(perms.actions, PermissionLevel::None);
}

/// Verify JsonWebToken time-until-expiry calculation.
///
/// Tests the `time_until_expiry()` method to ensure:
/// - Correctly calculates remaining time until token expires
/// - Returns a Duration that accounts for current time
/// - Provides accurate minute-level precision
#[test]
fn test_jwt_token_time_until_expiry() {
    let app_id = GitHubAppId::new(1);
    let expires_at = Utc::now() + Duration::minutes(5);
    let jwt = JsonWebToken::new("test".to_string(), app_id, expires_at);

    let remaining = jwt.time_until_expiry();
    assert!(remaining.num_minutes() >= 4 && remaining.num_minutes() <= 5);
}

/// Verify InstallationToken expiry detection and soon-to-expire warnings.

/// Verify TargetType enum variants and serialization.
///
/// Tests that TargetType variants (Organization, User) are correctly
/// defined and serialize to PascalCase as expected by GitHub API.
#[test]
fn test_target_type() {
    use serde_json;

    let org = TargetType::Organization;
    let user = TargetType::User;

    assert_eq!(format!("{:?}", org), "Organization");
    assert_eq!(format!("{:?}", user), "User");

    // Verify serialization format (PascalCase)
    let org_json = serde_json::to_string(&org).unwrap();
    assert_eq!(org_json, "\"Organization\"");

    let user_json = serde_json::to_string(&user).unwrap();
    assert_eq!(user_json, "\"User\"");
}

/// Verify Account struct creation and field access.
///
/// Tests that Account type correctly represents installation account
/// information with proper field access and type safety.
#[test]
fn test_account_type() {
    let account = Account {
        id: UserId::new(12345),
        login: "octocat".to_string(),
        account_type: TargetType::Organization,
        avatar_url: Some("https://github.com/octocat.png".to_string()),
        html_url: "https://github.com/octocat".to_string(),
    };

    assert_eq!(account.id, UserId::new(12345));
    assert_eq!(account.login, "octocat");
    assert_eq!(account.account_type, TargetType::Organization);
}

/// Verify Installation struct includes all required fields.
///
/// Tests that Installation type correctly represents all fields from
/// GitHub API including URLs, permissions, and metadata.
#[test]
fn test_installation_structure() {
    let account = Account {
        id: UserId::new(1),
        login: "octocat".to_string(),
        account_type: TargetType::Organization,
        avatar_url: Some("https://github.com/octocat.png".to_string()),
        html_url: "https://github.com/octocat".to_string(),
    };

    let installation = Installation {
        id: InstallationId::new(123),
        account,
        access_tokens_url: "https://api.github.com/app/installations/123/access_tokens".to_string(),
        repositories_url: "https://api.github.com/installation/repositories".to_string(),
        html_url: "https://github.com/settings/installations/123".to_string(),
        app_id: GitHubAppId::new(456),
        target_type: TargetType::Organization,
        repository_selection: RepositorySelection::All,
        permissions: InstallationPermissions::default(),
        events: vec!["push".to_string(), "pull_request".to_string()],
        created_at: Utc::now(),
        updated_at: Utc::now(),
        single_file_name: None,
        has_multiple_single_files: false,
        suspended_at: None,
        suspended_by: None,
    };

    assert_eq!(installation.id, InstallationId::new(123));
    assert_eq!(installation.app_id, GitHubAppId::new(456));
    assert_eq!(installation.target_type, TargetType::Organization);
    assert_eq!(installation.repository_selection, RepositorySelection::All);
    assert_eq!(installation.events.len(), 2);
}

/// Verify InstallationToken expiry detection and soon-to-expire warnings.
///
/// Tests token expiry logic to ensure:
/// - Tokens with future expiry are not marked as expired
/// - `expires_soon()` returns false when margin is less than remaining time
/// - `expires_soon()` returns true when margin exceeds remaining time
#[test]
fn test_installation_token_expiry() {
    let token = InstallationToken::new(
        "test".to_string(),
        InstallationId::new(1),
        Utc::now() + Duration::minutes(30),
        InstallationPermissions::default(),
        vec![],
    );

    assert!(!token.is_expired());
    assert!(!token.expires_soon(Duration::minutes(10)));
    assert!(token.expires_soon(Duration::minutes(40)));
}

/// Verify UserType enum variants and Debug output.
///
/// Tests that UserType variants (User, Bot, Organization) are correctly
/// defined and produce expected Debug string representations.
#[test]
fn test_user_type_variants() {
    let user_type = UserType::User;
    assert_eq!(format!("{:?}", user_type), "User");

    let bot_type = UserType::Bot;
    assert_eq!(format!("{:?}", bot_type), "Bot");

    let org_type = UserType::Organization;
    assert_eq!(format!("{:?}", org_type), "Organization");
}

/// Verify RepositorySelection enum variants and equality.
///
/// Tests that RepositorySelection variants (All, Selected) are correctly
/// defined and properly compared for equality and inequality.
#[test]
fn test_repository_selection_variants() {
    let all = RepositorySelection::All;
    let selected = RepositorySelection::Selected;

    assert_eq!(all, RepositorySelection::All);
    assert_eq!(selected, RepositorySelection::Selected);
    assert_ne!(all, selected);
}

/// Verify all Permission enum variants are distinct.
///
/// Tests that all permission variants (read/write for issues, PRs, contents, checks)
/// are properly defined and each variant is unique (equal to itself, unequal to others).
#[test]
fn test_permission_enum_all_variants() {
    let permissions = [
        Permission::ReadIssues,
        Permission::WriteIssues,
        Permission::ReadPullRequests,
        Permission::WritePullRequests,
        Permission::ReadContents,
        Permission::WriteContents,
        Permission::ReadChecks,
        Permission::WriteChecks,
    ];

    // Verify all permissions are distinct
    for (i, p1) in permissions.iter().enumerate() {
        for (j, p2) in permissions.iter().enumerate() {
            if i == j {
                assert_eq!(p1, p2);
            } else {
                assert_ne!(p1, p2);
            }
        }
    }
}

/// Verify KeyAlgorithm enum and Debug output.
///
/// Tests that KeyAlgorithm::RS256 produces the expected Debug string representation.
#[test]
fn test_key_algorithm() {
    let algo = KeyAlgorithm::RS256;
    assert_eq!(format!("{:?}", algo), "RS256");
}

/// Verify JwtClaims serialization to JSON.
///
/// Tests the Serialize trait implementation to ensure:
/// - GitHubAppId is serialized as a numeric value
/// - Issued-at (iat) and expiry (exp) timestamps are correctly serialized
/// - JSON output contains all expected claim fields
#[test]
fn test_jwt_claims_serialization() {
    use serde_json;

    let claims = JwtClaims {
        iss: GitHubAppId::new(12345),
        iat: 1234567890,
        exp: 1234568490,
    };

    let json = serde_json::to_string(&claims).unwrap();
    assert!(json.contains("12345"));
    assert!(json.contains("1234567890"));
    assert!(json.contains("1234568490"));
}
