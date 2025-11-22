//! Tests for session management and key generation strategies.

use super::*;
use std::collections::HashMap;

// ============================================================================
// Test Message Implementation
// ============================================================================

/// Generic test message with arbitrary metadata fields
struct TestMessage {
    metadata: HashMap<String, String>,
}

impl TestMessage {
    fn new() -> Self {
        Self {
            metadata: HashMap::new(),
        }
    }

    fn with_field(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

impl SessionKeyExtractor for TestMessage {
    fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.get(key).cloned()
    }

    fn list_metadata_keys(&self) -> Vec<String> {
        self.metadata.keys().cloned().collect()
    }
}

// ============================================================================
// SessionKeyExtractor Tests
// ============================================================================

mod session_key_extractor_tests {
    use super::*;

    #[test]
    fn test_get_metadata_returns_value_when_present() {
        let message = TestMessage::new()
            .with_field("user_id", "12345")
            .with_field("resource_id", "abc-def");

        assert_eq!(message.get_metadata("user_id"), Some("12345".to_string()));
        assert_eq!(
            message.get_metadata("resource_id"),
            Some("abc-def".to_string())
        );
    }

    #[test]
    fn test_get_metadata_returns_none_when_absent() {
        let message = TestMessage::new().with_field("user_id", "12345");

        assert_eq!(message.get_metadata("missing_field"), None);
    }

    #[test]
    fn test_list_metadata_keys() {
        let message = TestMessage::new()
            .with_field("field1", "value1")
            .with_field("field2", "value2")
            .with_field("field3", "value3");

        let keys = message.list_metadata_keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"field1".to_string()));
        assert!(keys.contains(&"field2".to_string()));
        assert!(keys.contains(&"field3".to_string()));
    }

    #[test]
    fn test_get_all_metadata() {
        let message = TestMessage::new()
            .with_field("key1", "value1")
            .with_field("key2", "value2");

        let all_metadata = message.get_all_metadata();
        assert_eq!(all_metadata.len(), 2);
        assert_eq!(all_metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(all_metadata.get("key2"), Some(&"value2".to_string()));
    }
}

// ============================================================================
// CompositeKeyStrategy Tests
// ============================================================================

mod composite_key_strategy_tests {
    use super::*;

    #[test]
    fn test_generate_key_combines_multiple_fields() {
        let strategy = CompositeKeyStrategy::new(
            vec!["tenant_id".to_string(), "resource_id".to_string()],
            "-",
        );

        let message = TestMessage::new()
            .with_field("tenant_id", "tenant123")
            .with_field("resource_id", "res456");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "tenant123-res456");
    }

    #[test]
    fn test_generate_key_returns_none_when_any_field_missing() {
        let strategy =
            CompositeKeyStrategy::new(vec!["field1".to_string(), "field2".to_string()], "-");

        let message = TestMessage::new().with_field("field1", "value1");
        // field2 is missing

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_none());
    }

    #[test]
    fn test_generate_key_with_custom_separator() {
        let strategy =
            CompositeKeyStrategy::new(vec!["region".to_string(), "customer".to_string()], "::");

        let message = TestMessage::new()
            .with_field("region", "us-west")
            .with_field("customer", "acme");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "us-west::acme");
    }

    #[test]
    fn test_generate_key_with_single_field() {
        let strategy = CompositeKeyStrategy::new(vec!["order_id".to_string()], "-");

        let message = TestMessage::new().with_field("order_id", "12345");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "12345");
    }

    #[test]
    fn test_generate_key_with_empty_fields_returns_none() {
        let strategy = CompositeKeyStrategy::new(vec![], "-");

        let message = TestMessage::new().with_field("any_field", "value");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_none());
    }

    #[test]
    fn test_different_field_values_produce_different_keys() {
        let strategy = CompositeKeyStrategy::new(vec!["user_id".to_string()], "-");

        let message1 = TestMessage::new().with_field("user_id", "user1");
        let message2 = TestMessage::new().with_field("user_id", "user2");

        let key1 = strategy.generate_key(&message1).unwrap();
        let key2 = strategy.generate_key(&message2).unwrap();

        assert_ne!(key1.as_str(), key2.as_str());
    }
}

// ============================================================================
// SingleFieldStrategy Tests
// ============================================================================

mod single_field_strategy_tests {
    use super::*;

    #[test]
    fn test_generate_key_from_field_with_prefix() {
        let strategy = SingleFieldStrategy::new("user_id", Some("user"));

        let message = TestMessage::new().with_field("user_id", "12345");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "user-12345");
    }

    #[test]
    fn test_generate_key_from_field_without_prefix() {
        let strategy = SingleFieldStrategy::new("order_id", None);

        let message = TestMessage::new().with_field("order_id", "ORD-999");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "ORD-999");
    }

    #[test]
    fn test_generate_key_returns_none_when_field_missing() {
        let strategy = SingleFieldStrategy::new("missing_field", Some("prefix"));

        let message = TestMessage::new().with_field("other_field", "value");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_none());
    }

    #[test]
    fn test_different_field_values_produce_different_keys() {
        let strategy = SingleFieldStrategy::new("resource_id", Some("res"));

        let message1 = TestMessage::new().with_field("resource_id", "abc");
        let message2 = TestMessage::new().with_field("resource_id", "xyz");

        let key1 = strategy.generate_key(&message1).unwrap();
        let key2 = strategy.generate_key(&message2).unwrap();

        assert_ne!(key1.as_str(), key2.as_str());
    }

    #[test]
    fn test_same_field_value_produces_same_key() {
        let strategy = SingleFieldStrategy::new("tenant_id", Some("tenant"));

        let message1 = TestMessage::new().with_field("tenant_id", "abc123");
        let message2 = TestMessage::new().with_field("tenant_id", "abc123");

        let key1 = strategy.generate_key(&message1).unwrap();
        let key2 = strategy.generate_key(&message2).unwrap();

        assert_eq!(key1.as_str(), key2.as_str());
    }
}

// ============================================================================
// NoOrderingStrategy Tests
// ============================================================================

mod no_ordering_strategy_tests {
    use super::*;

    #[test]
    fn test_always_returns_none() {
        let strategy = NoOrderingStrategy;

        let message1 = TestMessage::new()
            .with_field("user_id", "123")
            .with_field("resource_id", "456");
        let message2 = TestMessage::new().with_field("order_id", "789");
        let message3 = TestMessage::new();

        assert!(strategy.generate_key(&message1).is_none());
        assert!(strategy.generate_key(&message2).is_none());
        assert!(strategy.generate_key(&message3).is_none());
    }
}

// ============================================================================
// FallbackStrategy Tests
// ============================================================================

mod fallback_strategy_tests {
    use super::*;

    #[test]
    fn test_uses_primary_when_available() {
        let primary = SingleFieldStrategy::new("entity_id", Some("entity"));
        let fallback = SingleFieldStrategy::new("tenant_id", Some("tenant"));

        let strategy = FallbackStrategy::new(vec![Box::new(primary), Box::new(fallback)]);

        let message = TestMessage::new()
            .with_field("entity_id", "ent123")
            .with_field("tenant_id", "ten456");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "entity-ent123");
    }

    #[test]
    fn test_falls_back_when_primary_unavailable() {
        let primary = SingleFieldStrategy::new("entity_id", Some("entity"));
        let fallback = SingleFieldStrategy::new("tenant_id", Some("tenant"));

        let strategy = FallbackStrategy::new(vec![Box::new(primary), Box::new(fallback)]);

        let message = TestMessage::new().with_field("tenant_id", "ten456");
        // entity_id is missing

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "tenant-ten456");
    }

    #[test]
    fn test_returns_none_when_all_strategies_fail() {
        let primary = SingleFieldStrategy::new("field1", None);
        let fallback = SingleFieldStrategy::new("field2", None);

        let strategy = FallbackStrategy::new(vec![Box::new(primary), Box::new(fallback)]);

        let message = TestMessage::new().with_field("field3", "value");
        // Neither field1 nor field2 present

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_none());
    }

    #[test]
    fn test_multiple_fallback_levels() {
        let level1 = SingleFieldStrategy::new("specific_id", Some("specific"));
        let level2 = SingleFieldStrategy::new("group_id", Some("group"));
        let level3 = SingleFieldStrategy::new("global_id", Some("global"));

        let strategy =
            FallbackStrategy::new(vec![Box::new(level1), Box::new(level2), Box::new(level3)]);

        // Test skipping to level 2
        let message = TestMessage::new()
            .with_field("group_id", "grp789")
            .with_field("global_id", "glb999");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_some());
        assert_eq!(session_id.unwrap().as_str(), "group-grp789");
    }

    #[test]
    fn test_with_no_ordering_as_ultimate_fallback() {
        let primary = SingleFieldStrategy::new("entity_id", Some("entity"));
        let ultimate = NoOrderingStrategy;

        let strategy = FallbackStrategy::new(vec![Box::new(primary), Box::new(ultimate)]);

        let message = TestMessage::new().with_field("other_field", "value");

        let session_id = strategy.generate_key(&message);
        assert!(session_id.is_none()); // NoOrderingStrategy returns None
    }
}

// ============================================================================
// SessionLock Tests
// ============================================================================

mod session_lock_tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    /// Verify that a newly created lock is not expired.
    #[tokio::test]
    async fn test_new_lock_is_not_expired() {
        let session_id = SessionId::new("test-session".to_string()).unwrap();
        let lock = SessionLock::new(
            session_id,
            "consumer-1".to_string(),
            Duration::from_secs(30),
        );

        assert!(!lock.is_expired());
        assert_eq!(lock.owner(), "consumer-1");
    }

    /// Verify that lock tracks session ID and owner correctly.
    #[tokio::test]
    async fn test_lock_tracks_session_and_owner() {
        let session_id = SessionId::new("order-123".to_string()).unwrap();
        let lock = SessionLock::new(
            session_id.clone(),
            "worker-5".to_string(),
            Duration::from_secs(60),
        );

        assert_eq!(lock.session_id(), &session_id);
        assert_eq!(lock.owner(), "worker-5");
        assert_eq!(lock.lock_duration(), Duration::from_secs(60));
    }

    /// Verify that lock expires after the specified duration.
    #[tokio::test]
    async fn test_lock_expires_after_duration() {
        let session_id = SessionId::new("short-lived".to_string()).unwrap();
        let lock = SessionLock::new(
            session_id,
            "consumer-1".to_string(),
            Duration::from_millis(50),
        );

        assert!(!lock.is_expired());

        sleep(Duration::from_millis(60)).await;

        assert!(lock.is_expired());
    }

    /// Verify that time_remaining returns correct values.
    #[tokio::test]
    async fn test_time_remaining_calculation() {
        let session_id = SessionId::new("timed-session".to_string()).unwrap();
        let lock = SessionLock::new(
            session_id,
            "consumer-1".to_string(),
            Duration::from_millis(100),
        );

        let remaining = lock.time_remaining();
        assert!(remaining <= Duration::from_millis(100));
        assert!(remaining > Duration::from_millis(50)); // Should still have time

        sleep(Duration::from_millis(110)).await;

        assert_eq!(lock.time_remaining(), Duration::ZERO);
    }

    /// Verify that lock can be renewed to extend expiration.
    #[tokio::test]
    async fn test_lock_renewal() {
        let session_id = SessionId::new("renewable".to_string()).unwrap();
        let original_lock = SessionLock::new(
            session_id.clone(),
            "consumer-1".to_string(),
            Duration::from_millis(50),
        );

        sleep(Duration::from_millis(30)).await;

        let renewed_lock = original_lock.renew(Duration::from_millis(100));

        assert_eq!(renewed_lock.session_id(), &session_id);
        assert_eq!(renewed_lock.owner(), "consumer-1");
        assert!(!renewed_lock.is_expired());

        // Original lock should still expire on schedule
        sleep(Duration::from_millis(30)).await;
        assert!(original_lock.is_expired());

        // Renewed lock should still be valid
        assert!(!renewed_lock.is_expired());
    }
}

// ============================================================================
// SessionLockManager Tests
// ============================================================================

mod session_lock_manager_tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    /// Verify that a session can be locked successfully.
    #[tokio::test]
    async fn test_acquire_lock_succeeds() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("session-1".to_string()).unwrap();

        let result = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await;

        assert!(result.is_ok());
        let lock = result.unwrap();
        assert_eq!(lock.session_id(), &session_id);
        assert_eq!(lock.owner(), "consumer-1");
    }

    /// Verify that locking same session twice by different consumers fails.
    #[tokio::test]
    async fn test_acquire_locked_session_fails() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("contested-session".to_string()).unwrap();

        // First consumer acquires lock
        let _lock1 = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        // Second consumer tries to acquire same session
        let result = manager
            .try_acquire_lock(session_id.clone(), "consumer-2".to_string())
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QueueError::SessionLocked { .. }
        ));
    }

    /// Verify that same consumer can acquire lock again (idempotent).
    #[tokio::test]
    async fn test_same_consumer_can_reacquire_lock() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("idempotent-session".to_string()).unwrap();

        let _lock1 = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        // Same consumer tries again - should succeed
        let result = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await;

        assert!(result.is_ok());
    }

    /// Verify that expired locks can be acquired by new consumers.
    #[tokio::test]
    async fn test_expired_lock_can_be_reacquired() {
        let manager = SessionLockManager::new(Duration::from_millis(50));
        let session_id = SessionId::new("expiring-session".to_string()).unwrap();

        // First consumer acquires lock
        let _lock1 = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        // Wait for lock to expire
        sleep(Duration::from_millis(60)).await;

        // Second consumer can now acquire the session
        let result = manager
            .try_acquire_lock(session_id.clone(), "consumer-2".to_string())
            .await;

        assert!(result.is_ok());
        let lock = result.unwrap();
        assert_eq!(lock.owner(), "consumer-2");
    }

    /// Verify that locks can be renewed successfully.
    #[tokio::test]
    async fn test_renew_lock_succeeds() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("renewable-session".to_string()).unwrap();

        let _lock = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        let result = manager
            .renew_lock(&session_id, "consumer-1", Some(Duration::from_secs(60)))
            .await;

        assert!(result.is_ok());
        let renewed = result.unwrap();
        assert_eq!(renewed.lock_duration(), Duration::from_secs(60));
    }

    /// Verify that renewing lock for wrong owner fails.
    #[tokio::test]
    async fn test_renew_lock_wrong_owner_fails() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("owned-session".to_string()).unwrap();

        let _lock = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        let result = manager.renew_lock(&session_id, "consumer-2", None).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QueueError::SessionLocked { .. }
        ));
    }

    /// Verify that renewing non-existent lock fails.
    #[tokio::test]
    async fn test_renew_nonexistent_lock_fails() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("nonexistent".to_string()).unwrap();

        let result = manager.renew_lock(&session_id, "consumer-1", None).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QueueError::SessionNotFound { .. }
        ));
    }

    /// Verify that locks can be released successfully.
    #[tokio::test]
    async fn test_release_lock_succeeds() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("releasable-session".to_string()).unwrap();

        let _lock = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        let result = manager.release_lock(&session_id, "consumer-1").await;
        assert!(result.is_ok());

        // Session should no longer be locked
        assert!(!manager.is_locked(&session_id).await);
    }

    /// Verify that releasing lock for wrong owner fails.
    #[tokio::test]
    async fn test_release_lock_wrong_owner_fails() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("protected-session".to_string()).unwrap();

        let _lock = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        let result = manager.release_lock(&session_id, "consumer-2").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QueueError::SessionLocked { .. }
        ));
    }

    /// Verify that is_locked returns correct status.
    #[tokio::test]
    async fn test_is_locked_status() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("status-check".to_string()).unwrap();

        assert!(!manager.is_locked(&session_id).await);

        let _lock = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        assert!(manager.is_locked(&session_id).await);

        manager
            .release_lock(&session_id, "consumer-1")
            .await
            .unwrap();

        assert!(!manager.is_locked(&session_id).await);
    }

    /// Verify that get_lock returns lock information.
    #[tokio::test]
    async fn test_get_lock_returns_info() {
        let manager = SessionLockManager::new(Duration::from_secs(30));
        let session_id = SessionId::new("info-session".to_string()).unwrap();

        assert!(manager.get_lock(&session_id).await.is_none());

        let _original = manager
            .try_acquire_lock(session_id.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        let lock_info = manager.get_lock(&session_id).await;
        assert!(lock_info.is_some());
        assert_eq!(lock_info.unwrap().owner(), "consumer-1");
    }

    /// Verify that expired locks are cleaned up.
    #[tokio::test]
    async fn test_cleanup_expired_locks() {
        let manager = SessionLockManager::new(Duration::from_millis(50));

        // Create several locks
        let session1 = SessionId::new("session-1".to_string()).unwrap();
        let session2 = SessionId::new("session-2".to_string()).unwrap();

        manager
            .try_acquire_lock(session1.clone(), "consumer-1".to_string())
            .await
            .unwrap();
        manager
            .try_acquire_lock(session2.clone(), "consumer-2".to_string())
            .await
            .unwrap();

        assert_eq!(manager.lock_count().await, 2);

        // Wait for locks to expire
        sleep(Duration::from_millis(60)).await;

        let cleaned = manager.cleanup_expired_locks().await;
        assert_eq!(cleaned, 2);
        assert_eq!(manager.lock_count().await, 0);
    }

    /// Verify lock count tracking.
    #[tokio::test]
    async fn test_lock_count_tracking() {
        let manager = SessionLockManager::new(Duration::from_secs(30));

        assert_eq!(manager.lock_count().await, 0);
        assert_eq!(manager.active_lock_count().await, 0);

        let session1 = SessionId::new("session-1".to_string()).unwrap();
        let session2 = SessionId::new("session-2".to_string()).unwrap();

        manager
            .try_acquire_lock(session1.clone(), "consumer-1".to_string())
            .await
            .unwrap();
        manager
            .try_acquire_lock(session2.clone(), "consumer-2".to_string())
            .await
            .unwrap();

        assert_eq!(manager.lock_count().await, 2);
        assert_eq!(manager.active_lock_count().await, 2);

        manager.release_lock(&session1, "consumer-1").await.unwrap();

        assert_eq!(manager.lock_count().await, 1);
        assert_eq!(manager.active_lock_count().await, 1);
    }

    /// Verify that active lock count excludes expired locks.
    #[tokio::test]
    async fn test_active_lock_count_excludes_expired() {
        let manager = SessionLockManager::new(Duration::from_millis(50));

        let session1 = SessionId::new("active".to_string()).unwrap();
        let session2 = SessionId::new("expired".to_string()).unwrap();

        manager
            .try_acquire_lock(session1.clone(), "consumer-1".to_string())
            .await
            .unwrap();

        sleep(Duration::from_millis(30)).await;

        manager
            .try_acquire_lock(session2.clone(), "consumer-2".to_string())
            .await
            .unwrap();

        assert_eq!(manager.lock_count().await, 2);

        // Wait for first lock to expire
        sleep(Duration::from_millis(30)).await;

        assert_eq!(manager.lock_count().await, 2); // Still stored
        assert_eq!(manager.active_lock_count().await, 1); // Only one active
    }
}
