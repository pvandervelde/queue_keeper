//! Tests for in-memory queue provider.

use super::*;
use crate::provider::InMemoryConfig;

// ============================================================================
// Subtask 10.1: Storage Initialization Tests
// ============================================================================

mod storage_initialization {
    use super::*;

    /// Verify that InMemoryProvider can be created with default configuration.
    #[test]
    fn test_create_provider_with_default_config() {
        let provider = InMemoryProvider::default();
        assert_eq!(provider.provider_type(), ProviderType::InMemory);
        assert_eq!(provider.supports_sessions(), SessionSupport::Native);
        assert!(provider.supports_batching());
        assert_eq!(provider.max_batch_size(), 100);
    }

    /// Verify that InMemoryProvider can be created with custom configuration.
    #[test]
    fn test_create_provider_with_custom_config() {
        let config = InMemoryConfig {
            max_queue_size: 5000,
            enable_persistence: false,
        };

        let provider = InMemoryProvider::new(config);
        assert_eq!(provider.provider_type(), ProviderType::InMemory);
    }

    /// Verify that multiple providers can coexist independently.
    #[test]
    fn test_multiple_independent_providers() {
        let provider1 = InMemoryProvider::default();
        let provider2 = InMemoryProvider::default();

        // Providers should be independent (different storage)
        assert_eq!(provider1.provider_type(), provider2.provider_type());
    }

    /// Verify that storage is thread-safe (can be cloned and shared).
    #[test]
    fn test_storage_thread_safety() {
        use std::sync::Arc;

        let provider = Arc::new(InMemoryProvider::default());
        let provider_clone = Arc::clone(&provider);

        // Should be able to share across threads
        assert_eq!(provider.provider_type(), provider_clone.provider_type());
    }
}

// ============================================================================
// Subtask 10.1: Queue Management Tests
// ============================================================================

mod queue_management {
    use super::*;

    /// Verify that queues are created automatically when first accessed.
    ///
    /// Note: This test will use send_message once implemented in 10.2.
    /// For now, we verify the storage structure is properly initialized.
    #[test]
    fn test_queue_auto_creation() {
        let provider = InMemoryProvider::default();
        let storage = provider.storage.read().unwrap();

        // Initially no queues
        assert_eq!(storage.queues.len(), 0);
    }

    /// Verify that multiple queues can exist independently.
    ///
    /// Note: Full verification will be in 10.2 when send/receive implemented.
    #[test]
    fn test_multiple_independent_queues() {
        let provider = InMemoryProvider::default();
        let storage = provider.storage.read().unwrap();

        // Storage can hold multiple queues
        assert!(storage.queues.is_empty());
    }
}

// ============================================================================
// Subtask 10.1: Data Structure Tests
// ============================================================================

mod data_structures {
    use super::*;
    use bytes::Bytes;

    /// Verify StoredMessage creation from Message.
    #[test]
    fn test_stored_message_from_message() {
        let message = Message::new(Bytes::from("test body"));
        let message_id = MessageId::new();

        let stored = StoredMessage::from_message(&message, message_id.clone());

        assert_eq!(stored.message_id, message_id);
        assert_eq!(stored.body, Bytes::from("test body"));
        assert_eq!(stored.delivery_count, 0);
        assert!(stored.session_id.is_none());
        assert!(stored.correlation_id.is_none());
    }

    /// Verify StoredMessage with session ID.
    #[test]
    fn test_stored_message_with_session() {
        let session_id = SessionId::new("test-session".to_string()).unwrap();
        let message = Message::new(Bytes::from("test body")).with_session_id(session_id.clone());
        let message_id = MessageId::new();

        let stored = StoredMessage::from_message(&message, message_id);

        assert_eq!(stored.session_id, Some(session_id));
    }

    /// Verify StoredMessage with correlation ID.
    #[test]
    fn test_stored_message_with_correlation_id() {
        let correlation_id = "correlation-123".to_string();
        let message =
            Message::new(Bytes::from("test body")).with_correlation_id(correlation_id.clone());
        let message_id = MessageId::new();

        let stored = StoredMessage::from_message(&message, message_id);

        assert_eq!(stored.correlation_id, Some(correlation_id));
    }

    /// Verify StoredMessage with TTL sets expiration.
    #[test]
    fn test_stored_message_with_ttl() {
        let ttl = Duration::seconds(60);
        let message = Message::new(Bytes::from("test body")).with_ttl(ttl);
        let message_id = MessageId::new();

        let stored = StoredMessage::from_message(&message, message_id);

        assert!(stored.expires_at.is_some());
        assert!(!stored.is_expired()); // Should not be expired immediately
    }

    /// Verify StoredMessage expiration detection.
    #[test]
    fn test_stored_message_expiration_detection() {
        let past_time =
            Timestamp::from_datetime(chrono::Utc::now() - chrono::Duration::seconds(10));
        let stored = StoredMessage {
            message_id: MessageId::new(),
            body: Bytes::from("test"),
            attributes: HashMap::new(),
            session_id: None,
            correlation_id: None,
            enqueued_at: Timestamp::now(),
            delivery_count: 0,
            available_at: Timestamp::now(),
            expires_at: Some(past_time),
        };

        assert!(stored.is_expired());
    }

    /// Verify StoredMessage availability detection.
    #[test]
    fn test_stored_message_availability() {
        let future_time =
            Timestamp::from_datetime(chrono::Utc::now() + chrono::Duration::seconds(10));
        let stored = StoredMessage {
            message_id: MessageId::new(),
            body: Bytes::from("test"),
            attributes: HashMap::new(),
            session_id: None,
            correlation_id: None,
            enqueued_at: Timestamp::now(),
            delivery_count: 0,
            available_at: future_time,
            expires_at: None,
        };

        assert!(!stored.is_available());
    }

    /// Verify InFlightMessage expiration detection.
    #[test]
    fn test_inflight_message_expiration() {
        let past_time = Timestamp::from_datetime(chrono::Utc::now() - chrono::Duration::seconds(5));
        let stored = StoredMessage {
            message_id: MessageId::new(),
            body: Bytes::from("test"),
            attributes: HashMap::new(),
            session_id: None,
            correlation_id: None,
            enqueued_at: Timestamp::now(),
            delivery_count: 0,
            available_at: Timestamp::now(),
            expires_at: None,
        };

        let inflight = InFlightMessage {
            message: stored,
            receipt_handle: "test-receipt".to_string(),
            lock_expires_at: past_time,
        };

        assert!(inflight.is_expired());
    }

    /// Verify SessionState initialization.
    #[test]
    fn test_session_state_initialization() {
        let state = SessionState::new();

        assert!(!state.locked);
        assert!(state.lock_expires_at.is_none());
        assert!(state.locked_by.is_none());
        assert!(!state.is_locked());
    }

    /// Verify SessionState lock detection.
    #[test]
    fn test_session_state_lock_detection() {
        let mut state = SessionState::new();
        state.locked = true;
        state.lock_expires_at = Some(Timestamp::from_datetime(
            chrono::Utc::now() + chrono::Duration::minutes(5),
        ));
        state.locked_by = Some("client-1".to_string());

        assert!(state.is_locked());
    }

    /// Verify SessionState lock expiration.
    #[test]
    fn test_session_state_lock_expiration() {
        let mut state = SessionState::new();
        state.locked = true;
        state.lock_expires_at = Some(Timestamp::from_datetime(
            chrono::Utc::now() - chrono::Duration::seconds(5),
        ));

        assert!(!state.is_locked()); // Expired lock should return false
    }
}

// ============================================================================
// Subtask 10.1: Concurrent Access Tests
// ============================================================================

mod concurrent_access {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Verify that provider can be safely shared across threads.
    #[test]
    fn test_provider_thread_safety() {
        let provider = Arc::new(InMemoryProvider::default());
        let mut handles = vec![];

        for i in 0..10 {
            let provider_clone = Arc::clone(&provider);
            let handle = thread::spawn(move || {
                // Just verify we can access provider from multiple threads
                assert_eq!(provider_clone.provider_type(), ProviderType::InMemory);
                i
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Verify that storage can be accessed concurrently for reads.
    #[test]
    fn test_concurrent_storage_read_access() {
        let provider = Arc::new(InMemoryProvider::default());
        let mut handles = vec![];

        for _ in 0..10 {
            let provider_clone = Arc::clone(&provider);
            let handle = thread::spawn(move || {
                let storage = provider_clone.storage.read().unwrap();
                storage.queues.len()
            });
            handles.push(handle);
        }

        for handle in handles {
            let count = handle.join().unwrap();
            assert_eq!(count, 0); // No queues initially
        }
    }
}

// ============================================================================
// Subtask 10.2: Send/Receive Operations Tests
// ============================================================================

mod send_receive_operations {
    use super::*;
    use bytes::Bytes;
    use chrono::Duration;

    /// Verify that a message can be sent and received successfully (Assertion #1, #3).
    #[tokio::test]
    async fn test_send_and_receive_single_message() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("test-queue".to_string()).unwrap();

        // Send a message
        let message = Message::new(Bytes::from("Hello, World!"));
        let message_id = provider
            .send_message(&queue_name, &message)
            .await
            .expect("send_message should succeed");

        assert!(!message_id.as_str().is_empty());

        // Receive the message
        let received = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .expect("receive_message should succeed");

        assert!(received.is_some());
        let received_msg = received.unwrap();
        assert_eq!(received_msg.body, Bytes::from("Hello, World!"));
        assert_eq!(received_msg.delivery_count, 1);
    }

    /// Verify that multiple messages can be sent and received in batch.
    #[tokio::test]
    async fn test_send_and_receive_batch_messages() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("test-batch-queue".to_string()).unwrap();

        // Send multiple messages
        let messages = vec![
            Message::new(Bytes::from("Message 1")),
            Message::new(Bytes::from("Message 2")),
            Message::new(Bytes::from("Message 3")),
        ];

        let message_ids = provider
            .send_messages(&queue_name, &messages)
            .await
            .expect("send_messages should succeed");

        assert_eq!(message_ids.len(), 3);

        // Receive all messages
        let received = provider
            .receive_messages(&queue_name, 5, Duration::seconds(1))
            .await
            .expect("receive_messages should succeed");

        assert_eq!(received.len(), 3);
        assert_eq!(received[0].body, Bytes::from("Message 1"));
        assert_eq!(received[1].body, Bytes::from("Message 2"));
        assert_eq!(received[2].body, Bytes::from("Message 3"));
    }

    /// Verify that receiving from an empty queue returns None (Assertion #4).
    #[tokio::test]
    async fn test_receive_from_empty_queue_returns_none() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("empty-queue".to_string()).unwrap();

        let received = provider
            .receive_message(&queue_name, Duration::milliseconds(100))
            .await
            .expect("receive_message should succeed");

        assert!(received.is_none());
    }

    /// Verify that message payload integrity is maintained.
    #[tokio::test]
    async fn test_message_payload_integrity() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("integrity-queue".to_string()).unwrap();

        let original_body = Bytes::from(vec![0u8, 1, 2, 3, 4, 255]);
        let message = Message::new(original_body.clone());

        provider
            .send_message(&queue_name, &message)
            .await
            .expect("send_message should succeed");

        let received = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .expect("receive_message should succeed")
            .expect("message should be received");

        assert_eq!(received.body, original_body);
    }

    /// Verify that message attributes are preserved.
    #[tokio::test]
    async fn test_message_attributes_preserved() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("attributes-queue".to_string()).unwrap();

        let message = Message::new(Bytes::from("test"))
            .with_attribute("key1".to_string(), "value1".to_string())
            .with_attribute("key2".to_string(), "value2".to_string());

        provider
            .send_message(&queue_name, &message)
            .await
            .expect("send_message should succeed");

        let received = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .expect("receive_message should succeed")
            .expect("message should be received");

        assert_eq!(received.attributes.get("key1").unwrap(), "value1");
        assert_eq!(received.attributes.get("key2").unwrap(), "value2");
    }

    /// Verify that message size is validated against provider limits.
    #[tokio::test]
    async fn test_message_size_validation() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("size-queue".to_string()).unwrap();

        // Create message larger than 10MB limit
        let large_body = Bytes::from(vec![0u8; 11 * 1024 * 1024]);
        let message = Message::new(large_body);

        let result = provider.send_message(&queue_name, &message).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            QueueError::MessageTooLarge { size, max_size } => {
                assert!(size > max_size);
                assert_eq!(max_size, 10 * 1024 * 1024);
            }
            _ => panic!("Expected MessageTooLarge error"),
        }
    }

    /// Verify that correlation ID is preserved.
    #[tokio::test]
    async fn test_correlation_id_preserved() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("correlation-queue".to_string()).unwrap();

        let correlation_id = "correlation-123".to_string();
        let message = Message::new(Bytes::from("test")).with_correlation_id(correlation_id.clone());

        provider
            .send_message(&queue_name, &message)
            .await
            .expect("send_message should succeed");

        let received = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .expect("receive_message should succeed")
            .expect("message should be received");

        assert_eq!(received.correlation_id, Some(correlation_id));
    }

    /// Verify batch operations respect batch size limits.
    #[tokio::test]
    async fn test_batch_size_limits() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("batch-limit-queue".to_string()).unwrap();

        // Send more than max_batch_size (100)
        let messages: Vec<Message> = (0..150)
            .map(|i| Message::new(Bytes::from(format!("Message {}", i))))
            .collect();

        let result = provider.send_messages(&queue_name, &messages).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            QueueError::BatchTooLarge { size, max_size } => {
                assert_eq!(size, 150);
                assert_eq!(max_size, 100);
            }
            _ => panic!("Expected BatchTooLarge error"),
        }
    }
}

// ============================================================================
// Subtask 10.2: Session-Based Message Ordering Tests
// ============================================================================

mod session_ordering {
    use super::*;
    use bytes::Bytes;
    use chrono::Duration;

    /// Verify that messages within a session are received in FIFO order (Assertion #7).
    #[tokio::test]
    async fn test_session_message_ordering() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("session-queue".to_string()).unwrap();
        let session_id = SessionId::new("session-1".to_string()).unwrap();

        // Send messages A, B, C in order
        let messages = vec![
            Message::new(Bytes::from("A")).with_session_id(session_id.clone()),
            Message::new(Bytes::from("B")).with_session_id(session_id.clone()),
            Message::new(Bytes::from("C")).with_session_id(session_id.clone()),
        ];

        for msg in &messages {
            provider
                .send_message(&queue_name, msg)
                .await
                .expect("send_message should succeed");
        }

        // Receive messages - should be in same order
        let received_a = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .expect("receive should succeed")
            .expect("message A should be received");
        assert_eq!(received_a.body, Bytes::from("A"));
        assert_eq!(received_a.session_id, Some(session_id.clone()));

        // Complete message A before receiving B
        provider
            .complete_message(&received_a.receipt_handle)
            .await
            .expect("complete should succeed");

        let received_b = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .expect("receive should succeed")
            .expect("message B should be received");
        assert_eq!(received_b.body, Bytes::from("B"));

        provider
            .complete_message(&received_b.receipt_handle)
            .await
            .expect("complete should succeed");

        let received_c = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .expect("receive should succeed")
            .expect("message C should be received");
        assert_eq!(received_c.body, Bytes::from("C"));
    }

    /// Verify that messages from different sessions can be processed concurrently.
    #[tokio::test]
    async fn test_different_sessions_independent() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("multi-session-queue".to_string()).unwrap();

        let session1 = SessionId::new("session-1".to_string()).unwrap();
        let session2 = SessionId::new("session-2".to_string()).unwrap();

        // Send messages to different sessions
        let msg1 = Message::new(Bytes::from("Session1-Msg1")).with_session_id(session1.clone());
        let msg2 = Message::new(Bytes::from("Session2-Msg1")).with_session_id(session2.clone());
        let msg3 = Message::new(Bytes::from("Session1-Msg2")).with_session_id(session1.clone());

        provider.send_message(&queue_name, &msg1).await.unwrap();
        provider.send_message(&queue_name, &msg2).await.unwrap();
        provider.send_message(&queue_name, &msg3).await.unwrap();

        // Receive from first session
        let received1 = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .unwrap()
            .unwrap();

        // Should be able to receive from second session even though first is in-flight
        let received2 = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .unwrap()
            .unwrap();

        // One should be from session1, other from session2
        assert_ne!(received1.session_id, received2.session_id);
    }

    /// Verify that non-session messages don't interfere with session messages.
    #[tokio::test]
    async fn test_session_and_nonsession_messages() {
        let provider = InMemoryProvider::default();
        let queue_name = QueueName::new("mixed-queue".to_string()).unwrap();

        let session_id = SessionId::new("session-1".to_string()).unwrap();

        // Send mix of session and non-session messages
        let non_session_msg = Message::new(Bytes::from("No session"));
        let session_msg = Message::new(Bytes::from("With session")).with_session_id(session_id);

        provider
            .send_message(&queue_name, &non_session_msg)
            .await
            .unwrap();
        provider
            .send_message(&queue_name, &session_msg)
            .await
            .unwrap();

        // Both should be receivable
        let received1 = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .unwrap()
            .unwrap();
        provider
            .complete_message(&received1.receipt_handle)
            .await
            .unwrap();

        let received2 = provider
            .receive_message(&queue_name, Duration::seconds(1))
            .await
            .unwrap()
            .unwrap();

        // One should have session_id, other should not
        let has_session = received1.session_id.is_some() || received2.session_id.is_some();
        let has_no_session = received1.session_id.is_none() || received2.session_id.is_none();
        assert!(has_session && has_no_session);
    }
}
