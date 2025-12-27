//! # Queue-Keeper Service
//!
//! Binary entry point for the Queue-Keeper HTTP service.
//!
//! This executable:
//! - Loads configuration from environment and files
//! - Initializes observability (logging, metrics, tracing)
//! - Creates webhook processor and dependencies
//! - Starts the HTTP server from queue-keeper-api
//!
//! See specs/interfaces/http-service.md for complete specification.

mod circuit_breaker;

use queue_keeper_api::{
    start_server, DefaultEventStore, DefaultHealthChecker, ServiceConfig, ServiceError,
};
use queue_keeper_core::webhook::DefaultWebhookProcessor;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "queue_keeper_service=info,queue_keeper_api=info,tower_http=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Queue-Keeper Service");

    // Load configuration (TODO: from file/environment)
    let config = ServiceConfig::default();

    // Create service components
    let webhook_processor = Arc::new(DefaultWebhookProcessor::new(None, None, None));
    let health_checker = Arc::new(DefaultHealthChecker);
    let event_store = Arc::new(DefaultEventStore);

    info!(
        host = %config.server.host,
        port = config.server.port,
        "Starting HTTP server"
    );

    // Start the server
    if let Err(e) = start_server(config, webhook_processor, health_checker, event_store).await {
        error!("Failed to start server: {}", e);

        let exit_code = match e {
            ServiceError::BindFailed { .. } => 1,
            ServiceError::ServerFailed { .. } => 2,
            ServiceError::Configuration(_) => 3,
            ServiceError::HealthCheckFailed { .. } => 4,
        };

        std::process::exit(exit_code);
    }

    Ok(())
}
