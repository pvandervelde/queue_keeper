//! Tests for the queue-keeper-core library module.

use super::*;

#[test]
fn test_event_id_generation() {
    let id1 = EventId::new();
    let id2 = EventId::new();

    assert_ne!(id1, id2);
    assert!(!id1.as_str().is_empty());
}

#[test]
fn test_session_id_validation() {
    // Valid session ID
    let valid = SessionId::new("owner/repo/pull_request/123".to_string());
    assert!(valid.is_ok());

    // Too long
    let too_long = "a".repeat(129);
    let invalid = SessionId::new(too_long);
    assert!(matches!(invalid, Err(ValidationError::TooLong { .. })));

    // Invalid characters
    let invalid = SessionId::new("owner/repo with spaces/issue/456".to_string());
    assert!(matches!(
        invalid,
        Err(ValidationError::InvalidCharacters { .. })
    ));
}

#[test]
fn test_session_id_from_parts() {
    let session_id = SessionId::from_parts("microsoft", "vscode", "pull_request", "1234");
    assert_eq!(session_id.as_str(), "microsoft/vscode/pull_request/1234");
}

#[test]
fn test_retry_policy_delay_calculation() {
    let policy = RetryPolicy::exponential();

    let delay1 = policy.calculate_delay(1);
    let delay2 = policy.calculate_delay(2);
    let delay3 = policy.calculate_delay(3);

    assert!(delay1 > Duration::ZERO);
    assert!(delay2 > delay1);
    assert!(delay3 > delay2);
    assert!(delay3 <= policy.max_delay);
}
