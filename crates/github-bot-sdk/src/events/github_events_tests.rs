//! Tests for GitHub event structures.

use super::*;
use serde_json::json;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_repository() -> Repository {
    Repository {
        id: 123456,
        name: "test-repo".to_string(),
        full_name: "owner/test-repo".to_string(),
        owner: crate::client::RepositoryOwner {
            login: "owner".to_string(),
            id: 999,
            avatar_url: "https://example.com/avatar.png".to_string(),
            owner_type: crate::client::OwnerType::User,
        },
        private: false,
        description: Some("Test repository".to_string()),
        default_branch: "main".to_string(),
        html_url: "https://github.com/owner/test-repo".to_string(),
        clone_url: "https://github.com/owner/test-repo.git".to_string(),
        ssh_url: "git@github.com:owner/test-repo.git".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn create_test_user() -> EventUser {
    EventUser {
        id: 54321,
        login: "testuser".to_string(),
        user_type: "User".to_string(),
    }
}

// ============================================================================
// Pull Request Event Tests
// ============================================================================

/// Verify PullRequestEvent structure can be serialized and deserialized.
#[test]
fn test_pull_request_event_serde() {
    let json = json!({
        "action": "opened",
        "number": 42,
        "pull_request": {
            "id": 1,
            "node_id": "PR_kwDOABCDEF==",
            "number": 42,
            "title": "Test PR",
            "body": "Test description",
            "state": "open",
            "user": {
                "login": "author",
                "id": 123,
                "node_id": "U_kwDOABC",
                "type": "User"
            },
            "head": {
                "ref": "feature-branch",
                "sha": "abc123",
                "repo": {
                    "id": 1,
                    "name": "test-repo",
                    "full_name": "owner/test-repo"
                }
            },
            "base": {
                "ref": "main",
                "sha": "def456",
                "repo": {
                    "id": 1,
                    "name": "test-repo",
                    "full_name": "owner/test-repo"
                }
            },
            "draft": false,
            "merged": false,
            "mergeable": true,
            "merge_commit_sha": null,
            "assignees": [],
            "requested_reviewers": [],
            "labels": [],
            "milestone": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "closed_at": null,
            "merged_at": null,
            "html_url": "https://github.com/owner/test-repo/pull/42"
        },
        "repository": {
            "id": 123456,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": {
                "login": "owner",
                "id": 999,
                "avatar_url": "https://example.com/avatar.png",
                "type": "User"
            },
            "description": "Test repository",
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/owner/test-repo",
            "clone_url": "https://github.com/owner/test-repo.git",
            "ssh_url": "git@github.com:owner/test-repo.git",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        },
        "sender": {
            "id": 54321,
            "login": "testuser",
            "type": "User"
        }
    });

    let event: PullRequestEvent =
        serde_json::from_value(json.clone()).expect("Failed to deserialize");

    assert_eq!(event.action, PullRequestAction::Opened);
    assert_eq!(event.number, 42);

    let serialized = serde_json::to_value(&event).expect("Failed to serialize");
    let deserialized: PullRequestEvent =
        serde_json::from_value(serialized).expect("Failed to re-deserialize");

    assert_eq!(deserialized.action, PullRequestAction::Opened);
    assert_eq!(deserialized.number, 42);
}

/// Verify PullRequestAction enum serialization with snake_case.
#[test]
fn test_pull_request_action_serialization() {
    let test_cases = vec![
        (PullRequestAction::Opened, "opened"),
        (PullRequestAction::Closed, "closed"),
        (PullRequestAction::Synchronize, "synchronize"),
        (PullRequestAction::ReviewRequested, "review_requested"),
        (PullRequestAction::ReadyForReview, "ready_for_review"),
    ];

    for (action, expected_str) in test_cases {
        let json = serde_json::to_value(&action).expect("Failed to serialize");
        assert_eq!(json, json!(expected_str));

        let deserialized: PullRequestAction =
            serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(deserialized, action);
    }
}

/// Verify PullRequestAction Display implementation.
#[test]
fn test_pull_request_action_display() {
    assert_eq!(format!("{}", PullRequestAction::Opened), "opened");
    assert_eq!(format!("{}", PullRequestAction::Synchronize), "synchronize");
    assert_eq!(
        format!("{}", PullRequestAction::ReviewRequested),
        "review_requested"
    );
}

// ============================================================================
// Issue Event Tests
// ============================================================================

/// Verify IssueEvent structure can be serialized and deserialized.
#[test]
fn test_issue_event_serde() {
    let json = json!({
        "action": "opened",
        "issue": {
            "id": 1,
            "node_id": "I_kwDOABCDEF==",
            "number": 10,
            "title": "Test Issue",
            "body": "Issue description",
            "state": "open",
            "user": {
                "login": "reporter",
                "id": 456,
                "node_id": "U_kwDOABC",
                "type": "User"
            },
            "assignees": [],
            "labels": [],
            "milestone": null,
            "comments": 0,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "closed_at": null,
            "html_url": "https://github.com/owner/test-repo/issues/10"
        },
        "repository": {
            "id": 123456,
            "name": "test-repo",
            "full_name": "owner/test-repo",
            "owner": {
                "login": "owner",
                "id": 999,
                "avatar_url": "https://example.com/avatar.png",
                "type": "User"
            },
            "description": "Test repository",
            "private": false,
            "default_branch": "main",
            "html_url": "https://github.com/owner/test-repo",
            "clone_url": "https://github.com/owner/test-repo.git",
            "ssh_url": "git@github.com:owner/test-repo.git",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        },
        "sender": {
            "id": 54321,
            "login": "testuser",
            "type": "User"
        }
    });

    let event: IssueEvent = serde_json::from_value(json.clone()).expect("Failed to deserialize");

    assert_eq!(event.action, IssueAction::Opened);
    assert_eq!(event.issue.number, 10);

    let serialized = serde_json::to_value(&event).expect("Failed to serialize");
    let deserialized: IssueEvent =
        serde_json::from_value(serialized).expect("Failed to re-deserialize");

    assert_eq!(deserialized.action, IssueAction::Opened);
    assert_eq!(deserialized.issue.number, 10);
}

/// Verify IssueAction enum serialization.
#[test]
fn test_issue_action_serialization() {
    let test_cases = vec![
        (IssueAction::Opened, "opened"),
        (IssueAction::Closed, "closed"),
        (IssueAction::Labeled, "labeled"),
        (IssueAction::Transferred, "transferred"),
    ];

    for (action, expected_str) in test_cases {
        let json = serde_json::to_value(&action).expect("Failed to serialize");
        assert_eq!(json, json!(expected_str));

        let deserialized: IssueAction =
            serde_json::from_value(json).expect("Failed to deserialize");
        assert_eq!(deserialized, action);
    }
}

/// Verify IssueAction Display implementation.
#[test]
fn test_issue_action_display() {
    assert_eq!(format!("{}", IssueAction::Opened), "opened");
    assert_eq!(format!("{}", IssueAction::Labeled), "labeled");
}

// ============================================================================
// Push Event Tests
// ============================================================================

/// Verify PushEvent structure serialization.
#[test]
fn test_push_event_serde() {
    let commit = Commit {
        id: "abc123def456".to_string(),
        message: "Initial commit".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        author: CommitAuthor {
            name: "Test Author".to_string(),
            email: "author@example.com".to_string(),
            username: Some("testauthor".to_string()),
        },
        committer: CommitAuthor {
            name: "Test Committer".to_string(),
            email: "committer@example.com".to_string(),
            username: None,
        },
    };

    let event = PushEvent {
        ref_name: "refs/heads/main".to_string(),
        before: "0000000000000000000000000000000000000000".to_string(),
        after: "abc123def456".to_string(),
        created: true,
        deleted: false,
        forced: false,
        commits: vec![commit.clone()],
        head_commit: Some(commit),
        repository: create_test_repository(),
        sender: create_test_user(),
    };

    let json = serde_json::to_value(&event).expect("Failed to serialize");
    let deserialized: PushEvent = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized.ref_name, "refs/heads/main");
    assert_eq!(deserialized.commits.len(), 1);
    assert_eq!(deserialized.commits[0].id, "abc123def456");
}

/// Verify Commit structure handles all fields correctly.
#[test]
fn test_commit_structure() {
    let commit = Commit {
        id: "sha123".to_string(),
        message: "Fix bug".to_string(),
        timestamp: "2024-01-01T12:00:00Z".to_string(),
        author: CommitAuthor {
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            username: Some("alice-dev".to_string()),
        },
        committer: CommitAuthor {
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
            username: None,
        },
    };

    assert_eq!(commit.id, "sha123");
    assert_eq!(commit.author.username, Some("alice-dev".to_string()));
    assert_eq!(commit.committer.username, None);
}

/// Verify ref field is correctly renamed in JSON.
#[test]
fn test_push_event_ref_field_rename() {
    let event = PushEvent {
        ref_name: "refs/heads/feature".to_string(),
        before: "abc".to_string(),
        after: "def".to_string(),
        created: false,
        deleted: false,
        forced: false,
        commits: vec![],
        head_commit: None,
        repository: create_test_repository(),
        sender: create_test_user(),
    };

    let json = serde_json::to_value(&event).expect("Failed to serialize");

    // The `ref_name` field should serialize as `ref`
    assert!(json.get("ref").is_some());
    assert_eq!(json["ref"], "refs/heads/feature");
}

// ============================================================================
// Check Run Event Tests
// ============================================================================

/// Verify CheckRunEvent structure serialization.
#[test]
fn test_check_run_event_serde() {
    let check_run = CheckRun {
        id: 987654,
        name: "CI Tests".to_string(),
        status: "completed".to_string(),
        conclusion: Some("success".to_string()),
    };

    let event = CheckRunEvent {
        action: CheckRunAction::Completed,
        check_run,
        repository: create_test_repository(),
        sender: create_test_user(),
    };

    let json = serde_json::to_value(&event).expect("Failed to serialize");
    let deserialized: CheckRunEvent = serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized.action, CheckRunAction::Completed);
    assert_eq!(deserialized.check_run.name, "CI Tests");
    assert_eq!(
        deserialized.check_run.conclusion,
        Some("success".to_string())
    );
}

/// Verify CheckRunAction enum serialization.
#[test]
fn test_check_run_action_serialization() {
    let test_cases = vec![
        (CheckRunAction::Created, "created"),
        (CheckRunAction::Completed, "completed"),
        (CheckRunAction::Rerequested, "rerequested"),
    ];

    for (action, expected_str) in test_cases {
        let json = serde_json::to_value(&action).expect("Failed to serialize");
        assert_eq!(json, json!(expected_str));
    }
}

// ============================================================================
// Check Suite Event Tests
// ============================================================================

/// Verify CheckSuiteEvent structure serialization.
#[test]
fn test_check_suite_event_serde() {
    let check_suite = CheckSuite {
        id: 111222,
        status: "completed".to_string(),
        conclusion: Some("failure".to_string()),
    };

    let event = CheckSuiteEvent {
        action: CheckSuiteAction::Completed,
        check_suite,
        repository: create_test_repository(),
        sender: create_test_user(),
    };

    let json = serde_json::to_value(&event).expect("Failed to serialize");
    let deserialized: CheckSuiteEvent =
        serde_json::from_value(json).expect("Failed to deserialize");

    assert_eq!(deserialized.action, CheckSuiteAction::Completed);
    assert_eq!(
        deserialized.check_suite.conclusion,
        Some("failure".to_string())
    );
}

/// Verify CheckSuiteAction enum serialization.
#[test]
fn test_check_suite_action_serialization() {
    let test_cases = vec![
        (CheckSuiteAction::Completed, "completed"),
        (CheckSuiteAction::Requested, "requested"),
        (CheckSuiteAction::Rerequested, "rerequested"),
    ];

    for (action, expected_str) in test_cases {
        let json = serde_json::to_value(&action).expect("Failed to serialize");
        assert_eq!(json, json!(expected_str));
    }
}

// ============================================================================
// EventUser Tests
// ============================================================================

/// Verify EventUser serialization with type field rename.
#[test]
fn test_event_user_type_field() {
    let user = EventUser {
        id: 999,
        login: "botuser".to_string(),
        user_type: "Bot".to_string(),
    };

    let json = serde_json::to_value(&user).expect("Failed to serialize");
    assert_eq!(json["type"], "Bot");
    assert_eq!(json["login"], "botuser");

    let deserialized: EventUser = serde_json::from_value(json).expect("Failed to deserialize");
    assert_eq!(deserialized.user_type, "Bot");
}
