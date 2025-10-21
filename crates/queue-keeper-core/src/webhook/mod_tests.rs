//! Tests for webhook processing module.

use super::*;
use serde_json::json;

#[test]
fn test_webhook_headers_validation() {
    let mut headers = HashMap::new();
    headers.insert("X-GitHub-Event".to_string(), "push".to_string());
    headers.insert(
        "X-GitHub-Delivery".to_string(),
        "12345678-1234-1234-1234-123456789abc".to_string(),
    );
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    // Signature is required for non-ping events
    headers.insert(
        "X-Hub-Signature-256".to_string(),
        "sha256=test-signature".to_string(),
    );

    let webhook_headers = WebhookHeaders::from_http_headers(&headers);
    assert!(webhook_headers.is_ok());
}

#[test]
fn test_event_entity_extraction() {
    // Pull request event
    let pr_payload = json!({
        "action": "opened",
        "pull_request": {
            "number": 123
        }
    });
    let entity = EventEntity::from_payload("pull_request", &pr_payload);
    assert_eq!(entity, EventEntity::PullRequest { number: 123 });

    // Push event
    let push_payload = json!({
        "ref": "refs/heads/main",
        "commits": []
    });
    let entity = EventEntity::from_payload("push", &push_payload);
    assert_eq!(
        entity,
        EventEntity::Branch {
            name: "main".to_string()
        }
    );
}

#[test]
fn test_session_id_generation() {
    let repository = Repository::new(
        RepositoryId::new(12345),
        "test-repo".to_string(),
        "owner/test-repo".to_string(),
        User {
            id: UserId::new(1),
            login: "owner".to_string(),
            user_type: UserType::User,
        },
        false,
    );

    let entity = EventEntity::PullRequest { number: 123 };
    let session_id = EventEnvelope::generate_session_id(&repository, &entity);

    assert_eq!(session_id.as_str(), "owner/test-repo/pull_request/123");
}
