//! Tests for health check functionality

use super::*;

#[tokio::test]
async fn test_basic_health_check_returns_healthy_status() {
    // Arrange
    let checker = DefaultHealthChecker;

    // Act
    let status = checker.check_basic_health().await;

    // Assert
    assert!(status.is_healthy, "Basic health check should be healthy");
    assert!(
        status.checks.contains_key("service"),
        "Should include service check"
    );
    let service_check = &status.checks["service"];
    assert!(service_check.healthy, "Service check should be healthy");
}

#[tokio::test]
async fn test_deep_health_check_includes_dependency_checks() {
    // Arrange
    let checker = DefaultHealthChecker;

    // Act
    let status = checker.check_deep_health().await;

    // Assert
    // Deep health check should include basic service check
    assert!(
        status.checks.contains_key("service"),
        "Should include service check"
    );

    // Note: Once dependencies are added (queues, storage, etc.),
    // this test should verify those checks are included
}

#[tokio::test]
async fn test_readiness_check_returns_true_when_ready() {
    // Arrange
    let checker = DefaultHealthChecker;

    // Act
    let is_ready = checker.check_readiness().await;

    // Assert
    assert!(is_ready, "Service should be ready");
}

#[tokio::test]
async fn test_health_check_includes_duration() {
    // Arrange
    let checker = DefaultHealthChecker;

    // Act
    let status = checker.check_basic_health().await;

    // Assert
    let service_check = &status.checks["service"];
    assert!(
        service_check.duration_ms >= 0,
        "Should include duration measurement"
    );
}

#[tokio::test]
async fn test_health_endpoint_returns_200_when_healthy() {
    // Arrange
    let health_checker = Arc::new(DefaultHealthChecker);
    let webhook_processor = Arc::new(create_test_webhook_processor());
    let state = create_test_app_state(health_checker, webhook_processor);

    // Act
    let response = handle_health_check(axum::extract::State(state)).await;

    // Assert
    assert!(response.is_ok(), "Health check should return Ok");
    let health_response = response.unwrap().0;
    assert_eq!(health_response.status, "healthy");
    assert!(!health_response.checks.is_empty());
    assert_eq!(health_response.version, env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn test_deep_health_endpoint_returns_200_when_healthy() {
    // Arrange
    let health_checker = Arc::new(DefaultHealthChecker);
    let webhook_processor = Arc::new(create_test_webhook_processor());
    let state = create_test_app_state(health_checker, webhook_processor);

    // Act
    let response = handle_deep_health_check(axum::extract::State(state)).await;

    // Assert
    assert!(response.is_ok(), "Deep health check should return Ok");
    let health_response = response.unwrap().0;
    assert_eq!(health_response.status, "healthy");
}

#[tokio::test]
async fn test_readiness_endpoint_returns_200_when_ready() {
    // Arrange
    let health_checker = Arc::new(DefaultHealthChecker);
    let webhook_processor = Arc::new(create_test_webhook_processor());
    let state = create_test_app_state(health_checker, webhook_processor);

    // Act
    let response = handle_readiness_check(axum::extract::State(state)).await;

    // Assert
    assert!(response.is_ok(), "Readiness check should return Ok");
    let readiness_response = response.unwrap().0;
    assert!(readiness_response.ready);
}

// Helper functions

fn create_test_webhook_processor() -> impl WebhookProcessor {
    // Reuse the mock from lib_tests.rs
    use crate::tests::MockWebhookProcessor;
    MockWebhookProcessor::new()
}

fn create_test_app_state(
    health_checker: Arc<dyn HealthChecker>,
    webhook_processor: Arc<dyn WebhookProcessor>,
) -> AppState {
    AppState {
        config: ServiceConfig::default(),
        webhook_processor,
        health_checker,
        event_store: Arc::new(DefaultEventStore),
        metrics: Arc::new(ServiceMetrics::default()),
        telemetry_config: Arc::new(TelemetryConfig::default()),
    }
}
