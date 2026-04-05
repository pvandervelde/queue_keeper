//! Tests for response types and health checker implementations.

use super::*;
use crate::provider_registry::{ProviderId, ProviderRegistry};
use async_trait::async_trait;
use queue_keeper_core::{
    webhook::{
        NormalizationError, ProcessingOutput, StorageError, StorageReference, ValidationStatus,
        WebhookError, WebhookRequest, WrappedEvent,
    },
    ValidationError,
};
use std::sync::Arc;

// ============================================================================
// Minimal mock WebhookProcessor for building a populated ProviderRegistry
// ============================================================================

struct NoopWebhookProcessor;

#[async_trait]
impl queue_keeper_core::webhook::WebhookProcessor for NoopWebhookProcessor {
    async fn process_webhook(
        &self,
        _request: WebhookRequest,
    ) -> Result<ProcessingOutput, WebhookError> {
        unimplemented!("not used in health checker tests")
    }

    async fn validate_signature(
        &self,
        _payload: &[u8],
        _signature: &str,
        _event_type: &str,
    ) -> Result<(), ValidationError> {
        unimplemented!("not used in health checker tests")
    }

    async fn store_raw_payload(
        &self,
        _request: &WebhookRequest,
        _validation_status: ValidationStatus,
    ) -> Result<StorageReference, StorageError> {
        unimplemented!("not used in health checker tests")
    }

    async fn normalize_event(
        &self,
        _request: &WebhookRequest,
    ) -> Result<WrappedEvent, NormalizationError> {
        unimplemented!("not used in health checker tests")
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn empty_registry() -> Arc<ProviderRegistry> {
    Arc::new(ProviderRegistry::new())
}

fn populated_registry() -> Arc<ProviderRegistry> {
    let mut registry = ProviderRegistry::new();
    registry.register(
        ProviderId::new("github").unwrap(),
        Arc::new(NoopWebhookProcessor),
    );
    Arc::new(registry)
}

// ============================================================================
// ServiceHealthChecker tests
// ============================================================================

mod service_health_checker_tests {
    use super::*;

    /// Verify that check_readiness() returns false when no providers are registered.
    #[tokio::test]
    async fn test_check_readiness_returns_false_for_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let ready = checker.check_readiness().await;

        assert!(!ready, "empty registry must not report ready");
    }

    /// Verify that check_readiness() returns true when at least one provider is registered.
    #[tokio::test]
    async fn test_check_readiness_returns_true_with_registered_provider() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let ready = checker.check_readiness().await;

        assert!(ready, "registry with 1+ providers must report ready");
    }

    /// Verify that check_basic_health() reports is_healthy = false when no providers
    /// are registered — the "providers" component must be unhealthy and the overall
    /// status must reflect that.
    #[tokio::test]
    async fn test_check_basic_health_is_unhealthy_for_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let status = checker.check_basic_health().await;

        assert!(
            !status.is_healthy,
            "basic health must be unhealthy when no providers are registered"
        );
        let providers_check = status
            .checks
            .get("providers")
            .expect("'providers' check must be present");
        assert!(
            !providers_check.healthy,
            "'providers' check must report unhealthy for empty registry"
        );
    }

    /// Verify that check_basic_health() reports is_healthy = true when at least one
    /// provider is registered.
    #[tokio::test]
    async fn test_check_basic_health_is_healthy_with_registered_provider() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let status = checker.check_basic_health().await;

        assert!(
            status.is_healthy,
            "basic health must be healthy when 1+ providers are registered"
        );
        let providers_check = status
            .checks
            .get("providers")
            .expect("'providers' check must be present");
        assert!(
            providers_check.healthy,
            "'providers' check must report healthy"
        );
    }

    /// Verify that check_deep_health() reports is_healthy = false when no providers
    /// are registered.
    #[tokio::test]
    async fn test_check_deep_health_is_unhealthy_for_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let status = checker.check_deep_health().await;

        assert!(
            !status.is_healthy,
            "deep health must be unhealthy when no providers are registered"
        );
        let providers_check = status
            .checks
            .get("providers")
            .expect("'providers' check must be present");
        assert!(
            !providers_check.healthy,
            "'providers' check must report unhealthy for empty registry"
        );
    }

    /// Verify that check_deep_health() reports is_healthy = true when at least one
    /// provider is registered.
    #[tokio::test]
    async fn test_check_deep_health_is_healthy_with_registered_provider() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let status = checker.check_deep_health().await;

        assert!(
            status.is_healthy,
            "deep health must be healthy when 1+ providers are registered"
        );
    }

    /// Verify that the "service" component is always reported healthy (process-level
    /// liveness), regardless of provider count.
    #[tokio::test]
    async fn test_check_basic_health_service_component_always_healthy() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let status = checker.check_basic_health().await;

        let service_check = status
            .checks
            .get("service")
            .expect("'service' check must be present");
        assert!(
            service_check.healthy,
            "'service' check must always be healthy while the process is running"
        );
    }

    /// Verify consistent is_healthy semantics between check_basic_health and
    /// check_deep_health: both must agree when providers are absent.
    #[tokio::test]
    async fn test_basic_and_deep_health_agree_on_empty_registry() {
        let checker = ServiceHealthChecker::new(empty_registry());

        let basic = checker.check_basic_health().await;
        let deep = checker.check_deep_health().await;

        assert_eq!(
            basic.is_healthy, deep.is_healthy,
            "check_basic_health and check_deep_health must agree on is_healthy"
        );
    }

    /// Verify consistent is_healthy semantics between check_basic_health and
    /// check_deep_health: both must agree when providers are present.
    #[tokio::test]
    async fn test_basic_and_deep_health_agree_on_populated_registry() {
        let checker = ServiceHealthChecker::new(populated_registry());

        let basic = checker.check_basic_health().await;
        let deep = checker.check_deep_health().await;

        assert_eq!(
            basic.is_healthy, deep.is_healthy,
            "check_basic_health and check_deep_health must agree on is_healthy"
        );
    }
}

// ============================================================================
// BlobBackedEventStore tests
// ============================================================================

mod blob_backed_event_store_tests {
    use crate::responses::{
        store_wrapped_event_to_blob, BlobBackedEventStore, EventListParams, EventStore,
        SessionListParams,
    };
    use queue_keeper_core::adapters::filesystem_storage::FilesystemBlobStorage;
    use queue_keeper_core::blob_storage::BlobStorage;
    use queue_keeper_core::webhook::WrappedEvent;
    use queue_keeper_core::SessionId;
    use std::sync::Arc;

    /// Helper: construct an in-temp-dir `Arc<dyn BlobStorage>` for a named test.
    async fn make_storage(test_name: &str) -> (Arc<dyn BlobStorage>, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("qk-event-store-test-{}", test_name));
        let _ = std::fs::remove_dir_all(&path);
        let storage: Arc<dyn BlobStorage> = Arc::new(
            FilesystemBlobStorage::new(path.clone())
                .await
                .expect("FilesystemBlobStorage::new must succeed"),
        );
        (storage, path)
    }

    /// Store one event and retrieve it back.
    ///
    /// Verifies that `store_wrapped_event_to_blob` + `BlobBackedEventStore::get_event`
    /// round-trips correctly, including event_id, event_type, and provider fields.
    #[tokio::test]
    async fn test_get_event_round_trips() {
        let (storage, dir) = make_storage("get-event").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        let event = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        let event_id = event.event_id;

        store_wrapped_event_to_blob(storage.as_ref(), &event)
            .await
            .expect("store must succeed");

        let retrieved = store
            .get_event(&event_id)
            .await
            .expect("get_event must succeed");

        assert_eq!(retrieved.event_id, event_id);
        assert_eq!(retrieved.event_type, "push");
        assert_eq!(retrieved.provider, "github");

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `get_event` returns `QueueKeeperError::NotFound` for unknown event IDs.
    #[tokio::test]
    async fn test_get_event_not_found_returns_error() {
        let (storage, dir) = make_storage("get-event-not-found").await;
        let store = BlobBackedEventStore::new(storage);

        let nonexistent = queue_keeper_core::EventId::new();
        let result = store.get_event(&nonexistent).await;

        assert!(result.is_err(), "must return error for unknown event");
        assert!(
            matches!(
                result.unwrap_err(),
                queue_keeper_core::QueueKeeperError::NotFound { .. }
            ),
            "error must be NotFound"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `list_events` returns all stored events with correct pagination.
    #[tokio::test]
    async fn test_list_events_returns_stored_events() {
        let (storage, dir) = make_storage("list-events").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        // Store 3 push events
        for _ in 0..3 {
            let e = WrappedEvent::new(
                "github".to_string(),
                "push".to_string(),
                None,
                None,
                serde_json::json!({}),
            );
            store_wrapped_event_to_blob(storage.as_ref(), &e)
                .await
                .expect("store must succeed");
        }

        let params = EventListParams {
            page: None,
            per_page: None,
            event_type: None,
            repository: None,
            session_id: None,
            since: None,
        };
        let response = store
            .list_events(params)
            .await
            .expect("list_events must succeed");

        assert_eq!(response.total, 3, "must return 3 stored events");
        assert_eq!(response.events.len(), 3);
        assert!(response.events.iter().all(|e| e.event_type == "push"));

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `list_events` with `event_type` filter returns only matching events.
    #[tokio::test]
    async fn test_list_events_filters_by_event_type() {
        let (storage, dir) = make_storage("list-events-filter").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        // Store 2 push + 1 pull_request
        for event_type in &["push", "push", "pull_request"] {
            let e = WrappedEvent::new(
                "github".to_string(),
                event_type.to_string(),
                None,
                None,
                serde_json::json!({}),
            );
            store_wrapped_event_to_blob(storage.as_ref(), &e)
                .await
                .unwrap();
        }

        let params = EventListParams {
            event_type: Some("pull_request".to_string()),
            page: None,
            per_page: None,
            repository: None,
            session_id: None,
            since: None,
        };
        let response = store.list_events(params).await.unwrap();

        assert_eq!(
            response.total, 1,
            "only pull_request events should be returned"
        );
        assert_eq!(response.events[0].event_type, "pull_request");

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `list_events` pagination: page 2 with per_page=1 returns the second event.
    #[tokio::test]
    async fn test_list_events_pagination() {
        let (storage, dir) = make_storage("list-events-page").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        for _ in 0..3 {
            let e = WrappedEvent::new(
                "github".to_string(),
                "push".to_string(),
                None,
                None,
                serde_json::json!({}),
            );
            store_wrapped_event_to_blob(storage.as_ref(), &e)
                .await
                .unwrap();
        }

        let params = EventListParams {
            page: Some(1),
            per_page: Some(2),
            event_type: None,
            repository: None,
            session_id: None,
            since: None,
        };
        let response = store.list_events(params).await.unwrap();

        assert_eq!(response.total, 3, "total must include all events");
        assert_eq!(
            response.events.len(),
            2,
            "per_page=2 → only 2 items returned"
        );
        assert_eq!(response.page, 1);
        assert_eq!(response.per_page, 2);

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `list_sessions` groups events by session_id.
    #[tokio::test]
    async fn test_list_sessions_groups_by_session_id() {
        let (storage, dir) = make_storage("list-sessions").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        let session_a = SessionId::from_parts("owner", "repo", "pull_request", "1");
        let session_b = SessionId::from_parts("owner", "repo", "pull_request", "2");

        // 2 events on session A, 1 event on session B
        for session in [&session_a, &session_a, &session_b] {
            let e = WrappedEvent::new(
                "github".to_string(),
                "pull_request".to_string(),
                Some("opened".to_string()),
                Some(session.clone()),
                serde_json::json!({}),
            );
            store_wrapped_event_to_blob(storage.as_ref(), &e)
                .await
                .unwrap();
        }

        let params = SessionListParams {
            repository: None,
            entity_type: None,
            status: None,
            limit: None,
        };
        let response = store.list_sessions(params).await.unwrap();

        assert_eq!(response.total, 2, "must return 2 distinct sessions");

        let session_a_summary = response
            .sessions
            .iter()
            .find(|s| s.session_id == session_a)
            .expect("session A must be present");
        assert_eq!(session_a_summary.event_count, 2, "session A has 2 events");
        assert_eq!(
            response
                .sessions
                .iter()
                .find(|s| s.session_id == session_b)
                .unwrap()
                .event_count,
            1
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `get_session` returns all events for a known session.
    #[tokio::test]
    async fn test_get_session_returns_session_details() {
        let (storage, dir) = make_storage("get-session").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        let session = SessionId::from_parts("owner", "myrepo", "pull_request", "42");

        for action in &["opened", "synchronize"] {
            let e = WrappedEvent::new(
                "github".to_string(),
                "pull_request".to_string(),
                Some(action.to_string()),
                Some(session.clone()),
                serde_json::json!({}),
            );
            store_wrapped_event_to_blob(storage.as_ref(), &e)
                .await
                .unwrap();
        }

        let details = store
            .get_session(&session)
            .await
            .expect("get_session must succeed");

        assert_eq!(details.session_id, session);
        assert_eq!(details.event_count, 2);
        assert_eq!(details.entity_type, "pull_request");
        assert_eq!(details.entity_id, "42");
        assert_eq!(details.events.len(), 2);

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `get_session` returns `NotFound` for an unknown session.
    #[tokio::test]
    async fn test_get_session_not_found() {
        let (storage, dir) = make_storage("get-session-missing").await;
        let store = BlobBackedEventStore::new(storage);

        let missing_session = SessionId::from_parts("x", "y", "issues", "999");
        let result = store.get_session(&missing_session).await;

        assert!(result.is_err());
        assert!(
            matches!(
                result.unwrap_err(),
                queue_keeper_core::QueueKeeperError::NotFound { .. }
            ),
            "must be NotFound for unknown session"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `get_statistics` reflects stored event count.
    #[tokio::test]
    async fn test_get_statistics_counts_stored_events() {
        let (storage, dir) = make_storage("get-statistics").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        // Initially zero events
        let stats_before = store.get_statistics().await.unwrap();
        assert_eq!(stats_before.total_events, 0);

        // Store 5 events
        for _ in 0..5 {
            let e = WrappedEvent::new(
                "github".to_string(),
                "push".to_string(),
                None,
                None,
                serde_json::json!({}),
            );
            store_wrapped_event_to_blob(storage.as_ref(), &e)
                .await
                .unwrap();
        }

        let stats_after = store.get_statistics().await.unwrap();
        assert_eq!(stats_after.total_events, 5);
        assert!(
            stats_after.uptime_seconds < 10,
            "uptime should be very small in tests"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `store_wrapped_event_to_blob` on a tampered event causes `get_event` to fail
    /// with `QueueKeeperError::Internal` (wrapping the `ChecksumMismatch`).
    #[tokio::test]
    async fn test_get_event_detects_tampered_blob() {
        use std::io::Write;

        let (storage, dir) = make_storage("get-event-tamper").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        let event = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        let event_id = event.event_id;

        store_wrapped_event_to_blob(storage.as_ref(), &event)
            .await
            .unwrap();

        // Locate and tamper the blob file on disk
        let blob_path = event_id.to_blob_path(); // "webhook-payloads/year=.../..."
        let file_path = dir.join(&blob_path);
        assert!(file_path.exists(), "blob file must exist");

        let raw = std::fs::read_to_string(&file_path).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        value["payload"]["body"] = serde_json::json!([99u8, 99u8, 99u8]); // corrupt body
        let tampered = serde_json::to_string_pretty(&value).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&file_path)
            .unwrap();
        f.write_all(tampered.as_bytes()).unwrap();
        drop(f);

        let result = store.get_event(&event_id).await;

        assert!(result.is_err(), "tampered blob must cause an error");
        // The error should be Internal (wrapping ChecksumMismatch) per map_storage_error
        assert!(
            matches!(
                result.unwrap_err(),
                queue_keeper_core::QueueKeeperError::Internal { .. }
            ),
            "must be Internal error for checksum mismatch"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `list_events` with a `since` filter returns only events received after the cutoff.
    #[tokio::test]
    async fn test_list_events_filters_by_since() {
        use queue_keeper_core::Timestamp;
        use std::time::Duration;

        let (storage, dir) = make_storage("list-events-since").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        // Store 2 events, then record a timestamp, then store 1 more event
        for _ in 0..2 {
            let e = WrappedEvent::new(
                "github".to_string(),
                "push".to_string(),
                None,
                None,
                serde_json::json!({}),
            );
            store_wrapped_event_to_blob(storage.as_ref(), &e)
                .await
                .unwrap();
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
        let cutoff = Timestamp::now();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let e = WrappedEvent::new(
            "github".to_string(),
            "push".to_string(),
            None,
            None,
            serde_json::json!({}),
        );
        store_wrapped_event_to_blob(storage.as_ref(), &e)
            .await
            .unwrap();

        let params = EventListParams {
            page: None,
            per_page: None,
            event_type: None,
            repository: None,
            session_id: None,
            since: Some(cutoff.to_rfc3339()),
        };
        let response = store.list_events(params).await.unwrap();

        assert_eq!(
            response.total, 1,
            "only the event after the cutoff should be returned"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    /// `list_sessions` with a limit must return the most-recently-active sessions,
    /// not an arbitrary subset limited before sorting.
    #[tokio::test]
    async fn test_list_sessions_limit_returns_most_recent() {
        use std::time::Duration;

        let (storage, dir) = make_storage("list-sessions-limit").await;
        let store = BlobBackedEventStore::new(Arc::clone(&storage));

        // Store session A first (oldest), then B (newest)
        let session_a = SessionId::from_parts("owner", "repo", "issues", "1");
        let session_b = SessionId::from_parts("owner", "repo", "issues", "2");

        let e_a = WrappedEvent::new(
            "github".to_string(),
            "issues".to_string(),
            None,
            Some(session_a.clone()),
            serde_json::json!({}),
        );
        store_wrapped_event_to_blob(storage.as_ref(), &e_a)
            .await
            .unwrap();

        // Ensure session B has a clearly later timestamp
        tokio::time::sleep(Duration::from_millis(50)).await;

        let e_b = WrappedEvent::new(
            "github".to_string(),
            "issues".to_string(),
            None,
            Some(session_b.clone()),
            serde_json::json!({}),
        );
        store_wrapped_event_to_blob(storage.as_ref(), &e_b)
            .await
            .unwrap();

        // Request only 1 session — must be the most-recently-active one (session B)
        let params = SessionListParams {
            repository: None,
            entity_type: None,
            status: None,
            limit: Some(1),
        };
        let response = store.list_sessions(params).await.unwrap();

        assert_eq!(
            response.sessions.len(),
            1,
            "limit=1 must return exactly 1 session"
        );
        assert_eq!(
            response.sessions[0].session_id, session_b,
            "the most recently active session must be returned when limit=1"
        );
        assert_eq!(
            response.total, 2,
            "total includes all sessions before the limit"
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}
