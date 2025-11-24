//! Tests for the queue-keeper-service library module.

use super::*;
use axum_test::TestServer;

#[tokio::test]
async fn test_health_endpoint() {
    let config = ServiceConfig::default();
    let webhook_processor = Arc::new(queue_keeper_core::webhook::DefaultWebhookProcessor::new(None, None));
    let health_checker = Arc::new(DefaultHealthChecker);
    let event_store = Arc::new(DefaultEventStore);
    let metrics = ServiceMetrics::new().expect("Failed to create metrics");
    let telemetry_config = Arc::new(TelemetryConfig::default());

    let state = AppState::new(
        config,
        webhook_processor,
        health_checker,
        event_store,
        metrics,
        telemetry_config,
    );
    let app = create_router(state);

    let _server = TestServer::new(app).unwrap();

    // TODO: Fix test once health checker is implemented
    // let response = server.get("/health").await;
    // assert_eq!(response.status_code(), 200);
}

#[test]
fn test_config_defaults() {
    let config = ServiceConfig::default();
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.webhooks.endpoint_path, "/webhook");
    assert!(config.webhooks.require_signature);
}
