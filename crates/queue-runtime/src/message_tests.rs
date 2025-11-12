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

// ============================================================================
// SendOptions Tests
// ============================================================================

#[test]
fn test_send_options_default() {
    let options = SendOptions::new();
    assert!(options.session_id.is_none());
    assert!(options.correlation_id.is_none());
    assert!(options.scheduled_enqueue_time.is_none());
    assert!(options.time_to_live.is_none());
    assert!(options.properties.is_empty());
    assert!(options.content_type.is_none());
    assert!(options.duplicate_detection_id.is_none());
}

#[test]
fn test_send_options_builder() {
    let session_id = SessionId::new("session-123".to_string()).unwrap();
    let options = SendOptions::new()
        .with_session_id(session_id.clone())
        .with_correlation_id("corr-456".to_string())
        .with_time_to_live(Duration::hours(1))
        .with_property("priority".to_string(), "high".to_string())
        .with_content_type("application/json".to_string())
        .with_duplicate_detection_id("dedup-789".to_string());

    assert_eq!(options.session_id, Some(session_id));
    assert_eq!(options.correlation_id, Some("corr-456".to_string()));
    assert_eq!(options.time_to_live, Some(Duration::hours(1)));
    assert_eq!(
        options.properties.get("priority"),
        Some(&"high".to_string())
    );
    assert_eq!(options.content_type, Some("application/json".to_string()));
    assert_eq!(
        options.duplicate_detection_id,
        Some("dedup-789".to_string())
    );
}

#[test]
fn test_send_options_with_scheduled_time() {
    let scheduled_time = Timestamp::from_datetime(Utc::now() + Duration::hours(2));
    let options = SendOptions::new().with_scheduled_enqueue_time(scheduled_time.clone());

    assert_eq!(options.scheduled_enqueue_time, Some(scheduled_time));
}

#[test]
fn test_send_options_with_delay() {
    let before = Utc::now();
    let options = SendOptions::new().with_delay(Duration::minutes(30));
    let after = Utc::now();

    assert!(options.scheduled_enqueue_time.is_some());
    let scheduled = options.scheduled_enqueue_time.unwrap().as_datetime();
    let expected_min = before + Duration::minutes(30);
    let expected_max = after + Duration::minutes(30);

    assert!(scheduled >= expected_min);
    assert!(scheduled <= expected_max);
}

#[test]
fn test_send_options_multiple_properties() {
    let options = SendOptions::new()
        .with_property("key1".to_string(), "value1".to_string())
        .with_property("key2".to_string(), "value2".to_string())
        .with_property("key3".to_string(), "value3".to_string());

    assert_eq!(options.properties.len(), 3);
    assert_eq!(options.properties.get("key1"), Some(&"value1".to_string()));
    assert_eq!(options.properties.get("key2"), Some(&"value2".to_string()));
    assert_eq!(options.properties.get("key3"), Some(&"value3".to_string()));
}

// ============================================================================
// ReceiveOptions Tests
// ============================================================================

#[test]
fn test_receive_options_default() {
    let options = ReceiveOptions::new();
    assert_eq!(options.max_messages, 1);
    assert_eq!(options.timeout, Duration::seconds(30));
    assert!(options.session_id.is_none());
    assert!(!options.accept_any_session);
    assert!(options.lock_duration.is_none());
    assert!(!options.peek_only);
    assert!(options.from_sequence_number.is_none());
}

#[test]
fn test_receive_options_builder() {
    let session_id = SessionId::new("session-abc".to_string()).unwrap();
    let options = ReceiveOptions::new()
        .with_max_messages(10)
        .with_timeout(Duration::seconds(60))
        .with_session_id(session_id.clone())
        .with_lock_duration(Duration::minutes(5));

    assert_eq!(options.max_messages, 10);
    assert_eq!(options.timeout, Duration::seconds(60));
    assert_eq!(options.session_id, Some(session_id));
    assert!(!options.accept_any_session);
    assert_eq!(options.lock_duration, Some(Duration::minutes(5)));
}

#[test]
fn test_receive_options_accept_any_session() {
    let options = ReceiveOptions::new().accept_any_session();

    assert!(options.accept_any_session);
    assert!(options.session_id.is_none());
}

#[test]
fn test_receive_options_session_id_overrides_accept_any() {
    let session_id = SessionId::new("specific-session".to_string()).unwrap();
    let options = ReceiveOptions::new()
        .accept_any_session()
        .with_session_id(session_id.clone());

    assert_eq!(options.session_id, Some(session_id));
    assert!(!options.accept_any_session);
}

#[test]
fn test_receive_options_accept_any_clears_session_id() {
    let session_id = SessionId::new("session-xyz".to_string()).unwrap();
    let options = ReceiveOptions::new()
        .with_session_id(session_id)
        .accept_any_session();

    assert!(options.session_id.is_none());
    assert!(options.accept_any_session);
}

#[test]
fn test_receive_options_peek_only() {
    let options = ReceiveOptions::new().peek_only();

    assert!(options.peek_only);
}

#[test]
fn test_receive_options_with_sequence_number() {
    let options = ReceiveOptions::new().from_sequence_number(12345);

    assert_eq!(options.from_sequence_number, Some(12345));
}

#[test]
fn test_receive_options_batch_configuration() {
    let options = ReceiveOptions::new()
        .with_max_messages(50)
        .with_timeout(Duration::seconds(120));

    assert_eq!(options.max_messages, 50);
    assert_eq!(options.timeout, Duration::seconds(120));
}
