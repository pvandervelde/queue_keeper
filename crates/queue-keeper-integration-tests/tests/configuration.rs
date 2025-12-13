//! Integration tests for configuration validation and defaults

mod common;

use queue_keeper_api::{ServerConfig, ServiceConfig, WebhookConfig};

/// Verify that ServiceConfig has proper defaults
#[test]
fn test_service_config_defaults() {
    let config = ServiceConfig::default();

    assert_eq!(config.server.port, 8080);
    assert_eq!(config.webhooks.endpoint_path, "/webhook");
    assert!(config.webhooks.require_signature);
}

/// Verify that ServerConfig defaults are production-ready
#[test]
fn test_server_config_defaults() {
    let config = ServerConfig::default();

    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 8080);
    assert_eq!(config.timeout_seconds, 30);
    assert_eq!(config.shutdown_timeout_seconds, 30);
    assert!(config.max_body_size > 0);
    assert!(config.enable_cors);
    assert!(config.enable_compression);
}

/// Verify that shutdown timeout can be customized
#[test]
fn test_custom_shutdown_timeout() {
    let config = ServerConfig {
        shutdown_timeout_seconds: 60,
        ..Default::default()
    };

    assert_eq!(config.shutdown_timeout_seconds, 60);
}

/// Verify that webhook endpoint path can be customized
#[test]
fn test_custom_webhook_endpoint() {
    let config = WebhookConfig {
        endpoint_path: "/hooks/github".to_string(),
        ..Default::default()
    };

    assert_eq!(config.endpoint_path, "/hooks/github");
}

/// Verify that signature validation can be disabled (for testing)
#[test]
fn test_signature_validation_can_be_disabled() {
    let config = WebhookConfig {
        require_signature: false,
        ..Default::default()
    };

    assert!(!config.require_signature);
}

/// Verify that server config includes timeout settings
#[test]
fn test_server_config_includes_timeouts() {
    let config = ServerConfig::default();

    assert!(config.timeout_seconds > 0);
    assert!(config.shutdown_timeout_seconds > 0);
}

/// Verify that max body size is reasonable
#[test]
fn test_max_body_size_is_reasonable() {
    let config = ServerConfig::default();

    // Should be at least 1MB for webhook payloads
    assert!(config.max_body_size >= 1024 * 1024);

    // Should not be too large (prevent memory exhaustion)
    assert!(config.max_body_size <= 100 * 1024 * 1024);
}
