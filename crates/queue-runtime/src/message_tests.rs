//! Tests for message types.

use super::*;
use chrono::Utc;

#[test]
fn test_message_builder() {
    let session_id = SessionId::new("test-session".to_string()).unwrap();
    let message = Message::new("test body".into())
        .with_session_id(session_id.clone())
        .with_attribute("key".to_string(), "value".to_string())
        .with_correlation_id("corr-123".to_string())
        .with_ttl(Duration::minutes(30));

    assert_eq!(message.session_id, Some(session_id));
    assert_eq!(message.attributes.get("key"), Some(&"value".to_string()));
    assert_eq!(message.correlation_id, Some("corr-123".to_string()));
    assert_eq!(message.time_to_live, Some(Duration::minutes(30)));
}

#[test]
fn test_receipt_handle_expiry() {
    let expires_at = Timestamp::from_datetime(Utc::now() + Duration::minutes(5));
    let receipt = ReceiptHandle::new(
        "test-receipt".to_string(),
        expires_at,
        ProviderType::InMemory,
    );

    assert!(!receipt.is_expired());
    assert!(receipt.time_until_expiry() > Duration::minutes(4));
}

#[test]
fn test_received_message_to_message() {
    let session_id = SessionId::new("test".to_string()).unwrap();
    let received = ReceivedMessage {
        message_id: MessageId::new(),
        body: "test".into(),
        attributes: HashMap::new(),
        session_id: Some(session_id.clone()),
        correlation_id: Some("corr-123".to_string()),
        receipt_handle: ReceiptHandle::new(
            "receipt".to_string(),
            Timestamp::now(),
            ProviderType::InMemory,
        ),
        delivery_count: 1,
        first_delivered_at: Timestamp::now(),
        delivered_at: Timestamp::now(),
    };

    let message = received.message();
    assert_eq!(message.session_id, Some(session_id));
    assert_eq!(message.correlation_id, Some("corr-123".to_string()));
    assert_eq!(message.time_to_live, None); // TTL not preserved
}
