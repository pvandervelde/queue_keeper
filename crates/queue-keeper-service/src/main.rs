use queue_keeper_core::webhook::DefaultWebhookProcessor;
use queue_keeper_service::{
    start_server, DefaultEventStore, DefaultHealthChecker, ServiceConfig, ServiceError,
};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "queue_keeper_service=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Queue-Keeper Service");

    // Load configuration (TODO: from file/environment)
    let config = ServiceConfig::default();

    // Create service components
    let webhook_processor = Arc::new(DefaultWebhookProcessor);
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
