//! Configuration types for the HTTP service

use serde::{Deserialize, Serialize};

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceConfig {
    /// HTTP server settings
    pub server: ServerConfig,

    /// Webhook processing settings
    pub webhooks: WebhookConfig,

    /// Security settings
    pub security: SecurityConfig,

    /// Logging configuration
    pub logging: LoggingConfig,
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Graceful shutdown timeout in seconds
    pub shutdown_timeout_seconds: u64,

    /// Maximum request size in bytes
    pub max_body_size: usize,

    /// Enable CORS
    pub enable_cors: bool,

    /// Enable compression
    pub enable_compression: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            timeout_seconds: 30,
            shutdown_timeout_seconds: 30,
            max_body_size: 10 * 1024 * 1024, // 10MB
            enable_cors: true,
            enable_compression: true,
        }
    }
}

/// Webhook processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook endpoint path
    pub endpoint_path: String,

    /// Require signature validation
    pub require_signature: bool,

    /// Enable payload storage for audit
    pub store_payloads: bool,

    /// Supported event types (empty = all)
    pub allowed_event_types: Vec<String>,

    /// Maximum events per repository per minute
    pub rate_limit_per_repo: Option<u32>,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            endpoint_path: "/webhook".to_string(),
            require_signature: true,
            store_payloads: true,
            allowed_event_types: vec![], // All events allowed by default
            rate_limit_per_repo: Some(100), // 100 events per minute per repo
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable request rate limiting
    pub enable_rate_limiting: bool,

    /// Global rate limit (requests per minute)
    pub global_rate_limit: u32,

    /// Enable IP-based rate limiting
    pub enable_ip_rate_limiting: bool,

    /// IP rate limit (requests per minute per IP)
    pub ip_rate_limit: u32,

    /// Enable request logging
    pub log_requests: bool,

    /// Log request bodies (security risk)
    pub log_request_bodies: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_rate_limiting: true,
            global_rate_limit: 1000,
            enable_ip_rate_limiting: true,
            ip_rate_limit: 100,
            log_requests: true,
            log_request_bodies: false,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Logging level
    pub level: String,

    /// Enable JSON structured logging
    pub json_format: bool,

    /// Log file path (optional)
    pub file_path: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            file_path: None,
        }
    }
}
