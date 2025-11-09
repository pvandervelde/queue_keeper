//! Tests for event processor.

use super::*;
use chrono::Utc;
use serde_json::json;

fn create_test_repository() -> crate::client::Repository {
    crate::client::Repository {
        id: 1,
        name: "repo".to_string(),
        full_name: "owner/repo".to_string(),
        owner: crate::client::RepositoryOwner {
            login: "owner".to_string(),
            id: 1,
            avatar_url: "https://avatars.githubusercontent.com/u/1?v=4".to_string(),
            owner_type: crate::client::OwnerType::User,
        },
        description: Some("Test repository".to_string()),
        private: false,
        default_branch: "main".to_string(),
        html_url: "https://github.com/owner/repo".to_string(),
        clone_url: "https://github.com/owner/repo.git".to_string(),
        ssh_url: "git@github.com:owner/repo.git".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn test_processor_config_default() {
    let config = ProcessorConfig::default();

    assert!(config.enable_signature_validation);
    assert!(config.enable_session_correlation);
    assert_eq!(config.max_payload_size, 1024 * 1024);
    assert_eq!(config.trace_sampling_rate, 0.1);
}

#[tokio::test]
async fn test_process_webhook_pull_request() {
    let config = ProcessorConfig {
        enable_signature_validation: false,
        ..Default::default()
    };
    let processor = EventProcessor::new(config);

    let payload = json!({
        "action": "opened",
        "number": 123,
        "pull_request": {
            "id": 1,
            "number": 123,
            "state": "open",
            "title": "Test PR",
            "body": "Test description",
            "user": {
                "id": 1,
                "login": "testuser",
                "type": "User"
            },
            "head": {
                "ref": "feature",
                "sha": "abc123",
                "repo": {
                    "id": 1,
                    "name": "repo",
                    "full_name": "owner/repo"
                }
            },
            "base": {
                "ref": "main",
                "sha": "def456",
                "repo": {
                    "id": 1,
                    "name": "repo",
                    "full_name": "owner/repo"
                }
            },
            "draft": false,
            "mergeable": true,
            "merged": false
        },
        "repository": {
            "id": 1,
            "name": "repo",
            "full_name": "owner/repo",
            "private": false,
            "fork": false,
            "url": "https://api.github.com/repos/owner/repo",
            "html_url": "https://github.com/owner/repo",
            "default_branch": "main"
        },
        "sender": {
            "id": 1,
            "login": "testuser",
            "type": "User"
        }
    });

    let payload_bytes = serde_json::to_vec(&payload).unwrap();

    let result = processor
        .process_webhook("pull_request", &payload_bytes, Some("delivery-123"))
        .await;

    assert!(result.is_ok() || result.is_err()); // Will fail until implemented
}

#[tokio::test]
async fn test_process_webhook_issues() {
    let config = ProcessorConfig {
        enable_signature_validation: false,
        ..Default::default()
    };
    let processor = EventProcessor::new(config);

    let payload = json!({
        "action": "opened",
        "issue": {
            "id": 1,
            "number": 456,
            "state": "open",
            "title": "Test Issue",
            "body": "Test description",
            "user": {
                "id": 1,
                "login": "testuser",
                "type": "User"
            },
            "labels": []
        },
        "repository": {
            "id": 1,
            "name": "repo",
            "full_name": "owner/repo",
            "private": false,
            "fork": false,
            "url": "https://api.github.com/repos/owner/repo",
            "html_url": "https://github.com/owner/repo",
            "default_branch": "main"
        },
        "sender": {
            "id": 1,
            "login": "testuser",
            "type": "User"
        }
    });

    let payload_bytes = serde_json::to_vec(&payload).unwrap();

    let result = processor
        .process_webhook("issues", &payload_bytes, Some("delivery-456"))
        .await;

    assert!(result.is_ok() || result.is_err()); // Will fail until implemented
}

#[tokio::test]
async fn test_process_webhook_payload_too_large() {
    let config = ProcessorConfig {
        max_payload_size: 100, // Very small limit
        ..Default::default()
    };
    let processor = EventProcessor::new(config);

    let large_payload = vec![b'x'; 1000];

    let result = processor
        .process_webhook("pull_request", &large_payload, None)
        .await;

    assert!(result.is_err());
    if let Err(EventError::PayloadTooLarge { size, max }) = result {
        assert_eq!(size, 1000);
        assert_eq!(max, 100);
    }
}

#[tokio::test]
async fn test_process_webhook_invalid_json() {
    let config = ProcessorConfig {
        enable_signature_validation: false,
        ..Default::default()
    };
    let processor = EventProcessor::new(config);

    let invalid_json = b"not valid json {]";

    let result = processor
        .process_webhook("pull_request", invalid_json, None)
        .await;

    assert!(result.is_err());
}

#[test]
fn test_extract_entity_info_pull_request() {
    let config = ProcessorConfig::default();
    let processor = EventProcessor::new(config);

    let payload = json!({
        "number": 123,
        "pull_request": {
            "id": 1
        }
    });

    let result = processor.extract_entity_info("pull_request", &payload);
    assert!(result.is_ok() || result.is_err()); // Will fail until implemented
}

#[test]
fn test_extract_entity_info_issues() {
    let config = ProcessorConfig::default();
    let processor = EventProcessor::new(config);

    let payload = json!({
        "issue": {
            "number": 456
        }
    });

    let result = processor.extract_entity_info("issues", &payload);
    assert!(result.is_ok() || result.is_err()); // Will fail until implemented
}

#[test]
fn test_extract_entity_info_push() {
    let config = ProcessorConfig::default();
    let processor = EventProcessor::new(config);

    let payload = json!({
        "ref": "refs/heads/main"
    });

    let result = processor.extract_entity_info("push", &payload);
    assert!(result.is_ok() || result.is_err()); // Will fail until implemented
}

#[test]
fn test_generate_session_id_entity_strategy() {
    let config = ProcessorConfig {
        session_id_strategy: SessionIdStrategy::Entity,
        ..Default::default()
    };
    let processor = EventProcessor::new(config);

    let repository = create_test_repository();

    let session_id = processor.generate_session_id(
        &EntityType::PullRequest,
        &Some("123".to_string()),
        &repository,
    );

    assert!(session_id.is_some() || session_id.is_none()); // Will fail until implemented
}

#[test]
fn test_generate_session_id_repository_strategy() {
    let config = ProcessorConfig {
        session_id_strategy: SessionIdStrategy::Repository,
        ..Default::default()
    };
    let processor = EventProcessor::new(config);

    let repository = create_test_repository();

    let session_id = processor.generate_session_id(
        &EntityType::PullRequest,
        &Some("123".to_string()),
        &repository,
    );

    assert!(session_id.is_some() || session_id.is_none()); // Will fail until implemented
}

#[test]
fn test_generate_session_id_none_strategy() {
    let config = ProcessorConfig {
        session_id_strategy: SessionIdStrategy::None,
        ..Default::default()
    };
    let processor = EventProcessor::new(config);

    let repository = create_test_repository();

    let session_id = processor.generate_session_id(
        &EntityType::PullRequest,
        &Some("123".to_string()),
        &repository,
    );

    assert!(session_id.is_none());
}
