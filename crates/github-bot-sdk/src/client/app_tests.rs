//! Tests for GitHub App metadata types.

use super::*;
use crate::auth::{User, UserType};

#[test]
fn test_app_construction() {
    let owner = User {
        id: crate::auth::UserId::new(1),
        login: "octocat".to_string(),
        user_type: UserType::User,
        avatar_url: Some("https://github.com/images/error/octocat_happy.gif".to_string()),
        html_url: "https://github.com/octocat".to_string(),
    };

    let app = App {
        id: 12345,
        slug: "my-app".to_string(),
        name: "My App".to_string(),
        owner: owner.clone(),
        description: Some("A test app".to_string()),
        external_url: "https://example.com".to_string(),
        html_url: "https://github.com/apps/my-app".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    assert_eq!(app.id, 12345);
    assert_eq!(app.slug, "my-app");
    assert_eq!(app.name, "My App");
    assert_eq!(app.owner.login, "octocat");
    assert_eq!(app.description, Some("A test app".to_string()));
}

#[test]
fn test_app_serialization() {
    let owner = User {
        id: crate::auth::UserId::new(1),
        login: "octocat".to_string(),
        user_type: UserType::User,
        avatar_url: Some("https://github.com/images/error/octocat_happy.gif".to_string()),
        html_url: "https://github.com/octocat".to_string(),
    };

    let app = App {
        id: 12345,
        slug: "my-app".to_string(),
        name: "My App".to_string(),
        owner,
        description: Some("A test app".to_string()),
        external_url: "https://example.com".to_string(),
        html_url: "https://github.com/apps/my-app".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let json = serde_json::to_string(&app).unwrap();
    assert!(json.contains("\"id\":12345"));
    assert!(json.contains("\"slug\":\"my-app\""));
    assert!(json.contains("\"name\":\"My App\""));
}

#[test]
fn test_app_deserialization() {
    let json = r#"{
        "id": 12345,
        "slug": "my-app",
        "name": "My App",
        "owner": {
            "id": 1,
            "login": "octocat",
            "type": "User",
            "avatar_url": "https://github.com/images/error/octocat_happy.gif",
            "html_url": "https://github.com/octocat"
        },
        "description": "A test app",
        "external_url": "https://example.com",
        "html_url": "https://github.com/apps/my-app",
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-02T00:00:00Z"
    }"#;

    let app: App = serde_json::from_str(json).unwrap();

    assert_eq!(app.id, 12345);
    assert_eq!(app.slug, "my-app");
    assert_eq!(app.name, "My App");
    assert_eq!(app.owner.login, "octocat");
    assert_eq!(app.description, Some("A test app".to_string()));
    assert_eq!(app.external_url, "https://example.com");
}

#[test]
fn test_app_with_optional_description() {
    let json = r#"{
        "id": 12345,
        "slug": "my-app",
        "name": "My App",
        "owner": {
            "id": 1,
            "login": "octocat",
            "type": "User",
            "avatar_url": null,
            "html_url": "https://github.com/octocat"
        },
        "description": null,
        "external_url": "https://example.com",
        "html_url": "https://github.com/apps/my-app",
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-02T00:00:00Z"
    }"#;

    let app: App = serde_json::from_str(json).unwrap();

    assert_eq!(app.description, None);
}

#[test]
fn test_app_clone() {
    let owner = User {
        id: crate::auth::UserId::new(1),
        login: "octocat".to_string(),
        user_type: UserType::User,
        avatar_url: Some("https://github.com/images/error/octocat_happy.gif".to_string()),
        html_url: "https://github.com/octocat".to_string(),
    };

    let app = App {
        id: 12345,
        slug: "my-app".to_string(),
        name: "My App".to_string(),
        owner,
        description: Some("A test app".to_string()),
        external_url: "https://example.com".to_string(),
        html_url: "https://github.com/apps/my-app".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let cloned = app.clone();

    assert_eq!(app, cloned);
}
