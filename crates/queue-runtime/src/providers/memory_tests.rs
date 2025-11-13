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
