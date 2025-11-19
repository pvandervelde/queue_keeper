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
