//! Tests for authentication module types and traits.

use super::*;

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

#[test]
fn test_installation_id() {
    let installation = InstallationId::new(98765);
    assert_eq!(installation.as_u64(), 98765);
    assert_eq!(installation.to_string(), "98765");

    let parsed: InstallationId = "11111".parse().unwrap();
    assert_eq!(parsed.as_u64(), 11111);
}

#[test]
fn test_repository_id() {
    let repo = RepositoryId::new(54321);
    assert_eq!(repo.as_u64(), 54321);
    assert_eq!(repo.to_string(), "54321");
}

#[test]
fn test_user_id() {
    let user = UserId::new(99999);
    assert_eq!(user.as_u64(), 99999);
    assert_eq!(user.to_string(), "99999");
}

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

#[test]
fn test_private_key_security() {
    let key = PrivateKey::new(b"super_secret_key_material".to_vec(), KeyAlgorithm::RS256);

    let debug_output = format!("{:?}", key);
    assert!(!debug_output.contains("super_secret_key_material"));
    assert!(debug_output.contains("<REDACTED>"));
    assert_eq!(key.algorithm(), &KeyAlgorithm::RS256);
}

#[test]
fn test_permission_level_ordering() {
    assert!(PermissionLevel::Read != PermissionLevel::None);
    assert!(PermissionLevel::Write != PermissionLevel::Read);
    assert!(PermissionLevel::Admin != PermissionLevel::Write);
}

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

#[test]
fn test_default_permissions() {
    let perms = InstallationPermissions::default();
    assert_eq!(perms.issues, PermissionLevel::None);
    assert_eq!(perms.pull_requests, PermissionLevel::None);
    assert_eq!(perms.contents, PermissionLevel::None);
    assert_eq!(perms.metadata, PermissionLevel::Read); // Default for metadata
    assert_eq!(perms.checks, PermissionLevel::None);
    assert_eq!(perms.actions, PermissionLevel::None);
}

#[test]
fn test_jwt_token_time_until_expiry() {
    let app_id = GitHubAppId::new(1);
    let expires_at = Utc::now() + Duration::minutes(5);
    let jwt = JsonWebToken::new("test".to_string(), app_id, expires_at);

    let remaining = jwt.time_until_expiry();
    assert!(remaining.num_minutes() >= 4 && remaining.num_minutes() <= 5);
}

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

#[test]
fn test_user_type_variants() {
    let user_type = UserType::User;
    assert_eq!(format!("{:?}", user_type), "User");

    let bot_type = UserType::Bot;
    assert_eq!(format!("{:?}", bot_type), "Bot");

    let org_type = UserType::Organization;
    assert_eq!(format!("{:?}", org_type), "Organization");
}

#[test]
fn test_repository_selection_variants() {
    let all = RepositorySelection::All;
    let selected = RepositorySelection::Selected;

    assert_eq!(all, RepositorySelection::All);
    assert_eq!(selected, RepositorySelection::Selected);
    assert_ne!(all, selected);
}

#[test]
fn test_permission_enum_all_variants() {
    let permissions = vec![
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

#[test]
fn test_key_algorithm() {
    let algo = KeyAlgorithm::RS256;
    assert_eq!(format!("{:?}", algo), "RS256");
}

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
