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
    start_server, DefaultEventStore, DefaultHealthChecker, ProviderId, ProviderRegistry,
    ServiceConfig, ServiceError,
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

    // Build provider registry from configuration.
    // Each entry in config.providers gets its own webhook processor.
    // TODO: Wire SignatureValidator from provider_config.secret once Key Vault
    //       integration is available (see specs/interfaces/key-vault.md).
    let mut provider_registry = ProviderRegistry::new();
    for provider_config in &config.providers {
        match ProviderId::new(&provider_config.id) {
            Ok(provider_id) => {
                let processor = Arc::new(DefaultWebhookProcessor::new(None, None, None));
                provider_registry.register(provider_id, processor);
                info!(provider = %provider_config.id, "Registered webhook provider from config");
            }
            Err(e) => {
                error!(
                    provider = %provider_config.id,
                    error = %e,
                    "Skipping provider with invalid ID in configuration"
                );
            }
        }
    }

    // Ensure the default GitHub provider is always available for backward
    // compatibility when no explicit provider configuration has been supplied.
    if !provider_registry.contains("github") {
        let github_processor = Arc::new(DefaultWebhookProcessor::new(None, None, None));
        provider_registry.register(
            ProviderId::new("github").expect("'github' is a valid provider ID"),
            github_processor,
        );
        info!("Registered default GitHub webhook provider (no explicit config entry found)");
    }

    let health_checker = Arc::new(DefaultHealthChecker);
    let event_store = Arc::new(DefaultEventStore);

    info!(
        host = %config.server.host,
        port = config.server.port,
        "Starting HTTP server"
    );

    // Start the server
    if let Err(e) = start_server(config, provider_registry, health_checker, event_store).await {
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
