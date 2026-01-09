//! Tests for WebhookHandler trait.

use super::*;
use crate::client::{Repository, RepositoryOwner, OwnerType};
use crate::events::{EventId, EventMetadata, EventPayload, EventSource, EntityType};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;

// ============================================================================
// Mock Handler for Testing
// ============================================================================

#[derive(Clone)]
struct MockHandler {
    calls: Arc<Mutex<Vec<EventId>>>,
    should_fail: bool,
}

impl MockHandler {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        }
    }

    fn new_failing() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: true,
        }
    }

    async fn call_count(&self) -> usize {
        self.calls.lock().await.len()
    }

    async fn was_called_with(&self, event_id: &EventId) -> bool {
        self.calls.lock().await.contains(event_id)
    }
}

#[async_trait]
impl WebhookHandler for MockHandler {
    async fn handle_event(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.calls.lock().await.push(envelope.event_id.clone());

        if self.should_fail {
            Err("Handler intentionally failed".into())
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_envelope(event_type: &str) -> EventEnvelope {
    let repository = Repository {
        id: 12345,
        name: "test-repo".to_string(),
        full_name: "owner/test-repo".to_string(),
        owner: RepositoryOwner {
            login: "owner".to_string(),
            id: 1,
            avatar_url: "https://github.com/avatars/u/1".to_string(),
            owner_type: OwnerType::Organization,
        },
        private: false,
        description: Some("Test repository".to_string()),
        default_branch: "main".to_string(),
        html_url: "https://github.com/owner/test-repo".to_string(),
        clone_url: "https://github.com/owner/test-repo.git".to_string(),
        ssh_url: "git@github.com:owner/test-repo.git".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    EventEnvelope {
        event_id: EventId::new(),
        event_type: event_type.to_string(),
        repository,
        entity_type: EntityType::from_event_type(event_type),
        entity_id: Some("123".to_string()),
        session_id: Some("session-123".to_string()),
        payload: EventPayload::new(serde_json::json!({
            "action": "opened",
            "number": 123
        })),
        metadata: EventMetadata {
            received_at: Utc::now(),
            processed_at: None,
            source: EventSource::GitHub,
            delivery_id: Some("delivery-123".to_string()),
            signature_valid: true,
            retry_count: 0,
            routing_rules: vec![],
        },
        trace_context: None,
    }
}

// ============================================================================
// Test: Successful Handler Execution
// ============================================================================

/// Verify that handler receives event envelope correctly.
///
/// Tests that WebhookHandler trait implementation can successfully process
/// an event envelope and complete without errors.
#[tokio::test]
async fn test_handler_receives_event_envelope() {
    // Arrange
    let handler = MockHandler::new();
    let envelope = create_test_envelope("pull_request");
    let event_id = envelope.event_id.clone();

    // Act
    let result = handler.handle_event(&envelope).await;

    // Assert
    assert!(result.is_ok(), "Handler should succeed");
    assert_eq!(handler.call_count().await, 1, "Handler should be called once");
    assert!(
        handler.was_called_with(&event_id).await,
        "Handler should be called with correct event ID"
    );
}

/// Verify that handler can access event metadata.
///
/// Tests that all fields of the EventEnvelope are accessible to the handler
/// implementation for processing decisions.
#[tokio::test]
async fn test_handler_can_access_event_metadata() {
    // Arrange
    struct MetadataCheckingHandler {
        verified: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl WebhookHandler for MetadataCheckingHandler {
        async fn handle_event(
            &self,
            envelope: &EventEnvelope,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            // Verify we can access all envelope fields
            let _event_id = &envelope.event_id;
            let _event_type = &envelope.event_type;
            let _repository = &envelope.repository;
            let _entity_type = &envelope.entity_type;
            let _entity_id = &envelope.entity_id;
            let _session_id = &envelope.session_id;
            let _payload = &envelope.payload;
            let _metadata = &envelope.metadata;

            *self.verified.lock().await = true;
            Ok(())
        }
    }

    let verified = Arc::new(Mutex::new(false));
    let handler = MetadataCheckingHandler {
        verified: verified.clone(),
    };
    let envelope = create_test_envelope("issues");

    // Act
    let result = handler.handle_event(&envelope).await;

    // Assert
    assert!(result.is_ok(), "Handler should succeed");
    assert!(*verified.lock().await, "Handler should verify metadata access");
}

// ============================================================================
// Test: Handler Error Handling
// ============================================================================

/// Verify that handler errors are properly propagated.
///
/// Tests that when a handler returns an error, the error is properly
/// returned to the caller for logging/handling.
#[tokio::test]
async fn test_handler_error_is_propagated() {
    // Arrange
    let handler = MockHandler::new_failing();
    let envelope = create_test_envelope("pull_request");

    // Act
    let result = handler.handle_event(&envelope).await;

    // Assert
    assert!(result.is_err(), "Handler should return error");
    assert_eq!(
        handler.call_count().await,
        1,
        "Handler should be called even if it fails"
    );
}

// ============================================================================
// Test: Multiple Event Types
// ============================================================================

/// Verify that handler can process different event types.
///
/// Tests that the same handler can be called multiple times with
/// different event types.
#[tokio::test]
async fn test_handler_processes_multiple_event_types() {
    // Arrange
    let handler = MockHandler::new();

    // Act & Assert
    for event_type in &["pull_request", "issues", "push", "release"] {
        let envelope = create_test_envelope(event_type);
        let result = handler.handle_event(&envelope).await;
        assert!(result.is_ok(), "Handler should succeed for {}", event_type);
    }

    assert_eq!(
        handler.call_count().await,
        4,
        "Handler should be called for each event type"
    );
}

// ============================================================================
// Test: Async Execution
// ============================================================================

/// Verify that handler supports async operations.
///
/// Tests that handlers can perform async operations like delays
/// without blocking other operations.
#[tokio::test]
async fn test_handler_supports_async_operations() {
    // Arrange
    struct AsyncHandler;

    #[async_trait]
    impl WebhookHandler for AsyncHandler {
        async fn handle_event(
            &self,
            _envelope: &EventEnvelope,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            // Simulate async work
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            Ok(())
        }
    }

    let handler = AsyncHandler;
    let envelope = create_test_envelope("pull_request");

    // Act
    let start = tokio::time::Instant::now();
    let result = handler.handle_event(&envelope).await;
    let duration = start.elapsed();

    // Assert
    assert!(result.is_ok(), "Handler should succeed");
    assert!(
        duration >= tokio::time::Duration::from_millis(10),
        "Handler should perform async work"
    );
}

// ============================================================================
// Test: Concurrent Handler Execution
// ============================================================================

/// Verify that handler can be executed concurrently.
///
/// Tests that the same handler instance can process multiple events
/// concurrently without data races or panics.
#[tokio::test]
async fn test_handler_supports_concurrent_execution() {
    // Arrange
    let handler = Arc::new(MockHandler::new());
    let mut tasks = vec![];

    // Act - Spawn multiple concurrent handler executions
    for i in 0..10 {
        let handler_clone = handler.clone();
        let envelope = create_test_envelope("pull_request");
        
        tasks.push(tokio::spawn(async move {
            handler_clone.handle_event(&envelope).await
        }));
    }

    // Wait for all tasks to complete
    let mut results = vec![];
    for task in tasks {
        results.push(task.await);
    }

    // Assert
    for (i, result) in results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "Task {} should complete without panic",
            i
        );
        assert!(
            result.as_ref().unwrap().is_ok(),
            "Handler {} should succeed",
            i
        );
    }

    assert_eq!(
        handler.call_count().await,
        10,
        "Handler should be called 10 times concurrently"
    );
}
