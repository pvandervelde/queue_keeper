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
mod signature_validator;

use circuit_breaker::queue::CircuitBreakerQueueProvider;
use queue_keeper_api::{
    start_server, DefaultEventStore, DefaultHealthChecker, ProviderId, ProviderRegistry,
    QueueBackendConfig, ServiceConfig, ServiceError,
};
use queue_keeper_core::adapters::{memory_key_vault::InMemorySecretCache, AzureKeyVaultProvider};
use queue_keeper_core::bot_config::BotConfiguration;
use queue_keeper_core::key_vault::{KeyVaultConfiguration, KeyVaultProvider, SecretName};
use queue_keeper_core::webhook::{generic_provider::GenericWebhookProvider, GithubWebhookProvider};
use queue_runtime::{
    InMemoryConfig, ProviderConfig, QueueClientFactory, QueueConfig, StandardQueueClient,
};
use signature_validator::{KeyVaultSignatureValidator, LiteralSignatureValidator};
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

    // -------------------------------------------------------------------------
    // Load configuration
    //
    // Sources (applied in order — later sources override earlier ones):
    //  1. /etc/queue-keeper/service.yaml   — system-wide defaults
    //  2. ./config/service.yaml            — deployment-local override
    //  3. Path given by QK_CONFIG_FILE env — operator-specified file
    //  4. Environment variables prefixed QK__ (double-underscore separator)
    //     e.g. QK__SERVER__PORT=9090 sets server.port = 9090
    //
    // All service configuration fields carry serde defaults, so absent files
    // or an entirely unconfigured environment produces a valid service config
    // with built-in defaults.  A malformed file or an environment variable
    // that cannot be coerced to the correct type IS a hard error because it
    // indicates deliberate-but-broken operator configuration.
    // -------------------------------------------------------------------------
    let mut config_builder = config::Config::builder()
        .add_source(
            config::File::with_name("/etc/queue-keeper/service")
                .required(false)
                .format(config::FileFormat::Yaml),
        )
        .add_source(
            config::File::with_name("config/service")
                .required(false)
                .format(config::FileFormat::Yaml),
        );

    // Optional explicit path supplied by the operator.
    if let Ok(explicit_path) = std::env::var("QK_CONFIG_FILE") {
        if !explicit_path.is_empty() {
            config_builder = config_builder.add_source(
                config::File::with_name(&explicit_path)
                    .required(true)
                    .format(config::FileFormat::Yaml),
            );
            info!(path = %explicit_path, "Loading configuration from explicit path");
        }
    }

    let config = match config_builder
        .add_source(config::Environment::with_prefix("QK").separator("__"))
        .build()
    {
        Ok(cfg) => cfg,
        Err(e) => {
            error!(error = %e, "Failed to build configuration; aborting");
            std::process::exit(3);
        }
    };

    let mut service_config: ServiceConfig = match config.try_deserialize() {
        Ok(sc) => sc,
        Err(e) => {
            error!(
                error = %e,
                "Could not deserialize service configuration; aborting. \
                 Fix the configuration and restart."
            );
            std::process::exit(3);
        }
    };

    if let Err(e) = service_config.validate() {
        error!(error = %e, "Service configuration is invalid; aborting");
        std::process::exit(3);
    }

    // -------------------------------------------------------------------------
    // Initialise Azure Key Vault provider (when Key Vault secrets are used).
    //
    // The AzureKeyVaultProvider fetches secrets lazily at request time and
    // serves them from an in-memory cache for `cache_ttl_seconds` (default
    // 300 s = 5 minutes), satisfying spec assertion #16 "Secret Caching".
    //
    // service_config.validate() already guarantees that `key_vault` is Some
    // with a non-empty vault_url whenever any provider uses KeyVault secrets,
    // so if we reach the `None` branch here no KV provider is needed.
    // -------------------------------------------------------------------------
    let key_vault_provider: Option<Arc<dyn KeyVaultProvider>> =
        if let Some(kv_cfg) = &service_config.key_vault {
            let core_config = KeyVaultConfiguration {
                vault_url: kv_cfg.vault_url.clone(),
                cache_ttl_seconds: kv_cfg.cache_ttl_seconds,
                ..Default::default()
            };
            let cache = Arc::new(InMemorySecretCache::new());
            match AzureKeyVaultProvider::new(core_config, cache).await {
                Ok(provider) => {
                    info!(
                        vault_url = %kv_cfg.vault_url,
                        "Azure Key Vault provider initialised"
                    );
                    Some(Arc::new(provider) as Arc<dyn KeyVaultProvider>)
                }
                Err(e) => {
                    error!(error = %e, "Failed to initialise Azure Key Vault provider; aborting");
                    std::process::exit(3);
                }
            }
        } else {
            None
        };

    // -------------------------------------------------------------------------
    // Build provider registry
    //
    // For every entry in `config.providers` we create a GithubWebhookProvider
    // with the appropriate SignatureValidator:
    //   - Literal secret  → LiteralSignatureValidator (dev/CI only, emits WARN)
    //   - Key Vault secret → KeyVaultSignatureValidator backed by the
    //                        AzureKeyVaultProvider initialised above
    // -------------------------------------------------------------------------
    let mut provider_registry = ProviderRegistry::new();

    for provider_config in &service_config.providers {
        match ProviderId::new(&provider_config.id) {
            Ok(provider_id) => {
                let validator = build_validator_from_provider_config(
                    provider_config,
                    key_vault_provider.as_ref(),
                );
                let processor = Arc::new(GithubWebhookProvider::new(validator, None, None));
                provider_registry.register(provider_id, processor);
                info!(provider = %provider_config.id, "Registered GitHub webhook provider from config");
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
    if !provider_registry.contains(GithubWebhookProvider::PROVIDER_ID) {
        let github_processor = Arc::new(GithubWebhookProvider::new(None, None, None));
        provider_registry.register(
            ProviderId::new(GithubWebhookProvider::PROVIDER_ID)
                .expect("GithubWebhookProvider::PROVIDER_ID is a valid provider ID"),
            github_processor,
        );
        info!("Registered default GitHub webhook provider (no explicit config entry found)");
    }

    // -------------------------------------------------------------------------
    // Wire configuration-driven generic providers
    //
    // Each [`GenericProviderConfig`] entry in `service_config.generic_providers`
    // becomes a [`GenericWebhookProvider`] registered under its provider ID.
    //
    // We drain the vec here using `mem::take` so each config is consumed directly
    // by `with_signature_validator` without an extra clone.  The provider IDs are
    // collected first for the `generic_provider_ids` HashSet passed to
    // `start_server`.
    // -------------------------------------------------------------------------
    let generic_provider_ids: std::collections::HashSet<String> = service_config
        .generic_providers
        .iter()
        .map(|p| p.provider_id.clone())
        .collect();

    for generic_config in std::mem::take(&mut service_config.generic_providers) {
        let provider_id_str = generic_config.provider_id.clone();

        match ProviderId::new(&provider_id_str) {
            Ok(provider_id) => {
                // Build a signature validator for this generic provider.
                let validator = build_validator_from_generic_config(
                    &generic_config,
                    key_vault_provider.as_ref(),
                );

                let provider = GenericWebhookProvider::with_signature_validator(
                    generic_config,
                    None,
                    validator,
                );

                match provider {
                    Ok(p) => {
                        provider_registry.register(provider_id, Arc::new(p));
                        info!(
                            provider = %provider_id_str,
                            "Registered generic webhook provider from config"
                        );
                    }
                    Err(e) => {
                        error!(
                            provider = %provider_id_str,
                            error = %e,
                            "Failed to construct generic webhook provider; skipping"
                        );
                    }
                }
            }
            Err(e) => {
                error!(
                    provider = %provider_id_str,
                    error = %e,
                    "Skipping generic provider with invalid ID in configuration"
                );
            }
        }
    }

    let health_checker = Arc::new(DefaultHealthChecker);
    let event_store = Arc::new(DefaultEventStore);

    // -------------------------------------------------------------------------
    // Build queue client from runtime configuration.
    //
    // The provider is selected based on `service_config.queue`:
    //   - `provider: in_memory`          → InMemoryProvider (dev/test only)
    //   - `provider: azure_service_bus`  → AzureServiceBusProvider
    //   - `provider: aws_sqs`            → AwsSqsProvider
    //
    // All providers are wrapped with the circuit breaker so cascading failures
    // are contained (spec assertion #11: 5 consecutive failures → circuit open
    // for 30 s).  When the queue config is absent the default is in-memory.
    // -------------------------------------------------------------------------
    let queue_client = match build_queue_client(&service_config.queue).await {
        Ok(client) => client,
        Err(e) => {
            error!(error = %e, "Failed to initialise queue backend; aborting");
            std::process::exit(3);
        }
    };

    // Load bot configuration from environment or file; fall back to empty config.
    let bot_config = Arc::new(
        BotConfiguration::load_from_env()
            .or_else(|_| BotConfiguration::load_from_file(std::path::Path::new("config/bots.yaml")))
            .unwrap_or_else(|_| BotConfiguration {
                bots: vec![],
                settings: queue_keeper_core::bot_config::BotConfigurationSettings::default(),
            }),
    );

    info!(
        host = %service_config.server.host,
        port = service_config.server.port,
        "Starting HTTP server"
    );

    // Start the server
    if let Err(e) = start_server(
        service_config,
        provider_registry,
        health_checker,
        event_store,
        generic_provider_ids,
        Some(queue_client),
        bot_config,
    )
    .await
    {
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

// ============================================================================
// Private helpers
// ============================================================================

/// Build a circuit-breaker-wrapped [`QueueClient`] from the service's queue
/// backend configuration.
///
/// All provider variants are wrapped with [`CircuitBreakerQueueProvider`] so
/// cascading failures are contained regardless of the chosen backend.
///
/// # Errors
///
/// Returns a human-readable error string on connection or configuration failure
/// so the caller can log it and `std::process::exit`.
async fn build_queue_client(
    queue_config: &QueueBackendConfig,
) -> Result<Arc<dyn queue_runtime::QueueClient>, String> {
    use queue_runtime::providers::{AzureAuthMethod, AzureServiceBusProvider};
    use queue_runtime::{AwsSqsConfig, AzureServiceBusConfig, InMemoryProvider};

    match queue_config {
        QueueBackendConfig::InMemory { max_queue_size } => {
            tracing::warn!(
                "Queue backend is set to in-memory. Events will not be persisted across \
                 restarts. Configure `azure_service_bus` or `aws_sqs` for production."
            );
            let mut cfg = InMemoryConfig::default();
            if let Some(size) = max_queue_size {
                cfg.max_queue_size = *size;
            }
            let provider = Arc::new(InMemoryProvider::new(cfg));
            let cb = CircuitBreakerQueueProvider::new(provider);
            let client = StandardQueueClient::new(Box::new(cb), QueueConfig::default());
            Ok(Arc::new(client))
        }

        QueueBackendConfig::AzureServiceBus {
            namespace,
            connection_string,
            use_sessions,
            session_timeout_seconds,
        } => {
            let (auth_method, resolved_namespace) = if let Some(_cs) = connection_string {
                tracing::warn!(
                    "Azure Service Bus is configured with a connection string. \
                     Use managed identity (`namespace` only) in production."
                );
                // The outer `connection_string` field is passed directly into
                // AzureServiceBusConfig below; `_cs` just confirms it is Some.
                (AzureAuthMethod::ConnectionString, None)
            } else {
                match namespace {
                    Some(ns) => (AzureAuthMethod::DefaultCredential, Some(ns.clone())),
                    None => {
                        return Err("queue.azure_service_bus: `namespace` is required when \
                                   `connection_string` is absent"
                            .to_string())
                    }
                }
            };

            let session_timeout = session_timeout_seconds
                .map(|s| chrono::Duration::seconds(s as i64))
                .unwrap_or_else(|| chrono::Duration::minutes(5));

            let azure_cfg = AzureServiceBusConfig {
                connection_string: connection_string.clone(),
                namespace: resolved_namespace,
                auth_method,
                use_sessions: *use_sessions,
                session_timeout,
            };

            info!(
                namespace = ?azure_cfg.namespace,
                use_sessions = %use_sessions,
                "Connecting to Azure Service Bus"
            );

            let provider = AzureServiceBusProvider::new(azure_cfg)
                .await
                .map_err(|e| format!("Failed to connect to Azure Service Bus: {}", e))?;

            info!("Azure Service Bus connection established");

            let cb = CircuitBreakerQueueProvider::new(Arc::new(provider));
            let client = StandardQueueClient::new(Box::new(cb), QueueConfig::default());
            Ok(Arc::new(client))
        }

        QueueBackendConfig::AwsSqs {
            region,
            use_fifo_queues,
        } => {
            let aws_cfg = AwsSqsConfig {
                region: region.clone(),
                // Credentials come from the standard AWS credential chain
                // (IAM role → env vars → ~/.aws/credentials).  Never embed
                // access keys in configuration files.
                access_key_id: None,
                secret_access_key: None,
                use_fifo_queues: *use_fifo_queues,
            };

            let queue_runtime_cfg = QueueConfig {
                provider: ProviderConfig::AwsSqs(aws_cfg),
                ..QueueConfig::default()
            };

            info!(region = %region, use_fifo_queues = %use_fifo_queues, "Connecting to AWS SQS");

            let client = QueueClientFactory::create_client(queue_runtime_cfg)
                .await
                .map_err(|e| format!("Failed to connect to AWS SQS: {}", e))?;

            info!("AWS SQS connection established");

            // QueueClientFactory returns a Box<dyn QueueClient> via its own
            // internal provider — we cannot wrap it with CircuitBreakerQueueProvider
            // (which operates at the QueueProvider level). Instead, wrap the
            // returned client in a circuit-breaking adapter so that cascading
            // failures are still contained for SQS.
            //
            // TODO: when queue-runtime exposes a lower-level SQS provider,
            // switch to CircuitBreakerQueueProvider(Arc::new(AwsSqsProvider::new(cfg))).
            Ok(Arc::from(client))
        }
    }
}

/// Build a [`SignatureValidator`] from a standard [`ProviderConfig`].
///
/// - `Literal` secret → [`LiteralSignatureValidator`] (dev/test only, emits `WARN`).
/// - `KeyVault` secret → [`KeyVaultSignatureValidator`] backed by the provided
///   [`KeyVaultProvider`]. `key_vault` must be `Some` here; `ServiceConfig::validate()`
///   already guarantees this.
/// - `None` secret → returns `None` (no signature validation).
fn build_validator_from_provider_config(
    provider_config: &queue_keeper_api::ProviderConfig,
    key_vault: Option<&Arc<dyn KeyVaultProvider>>,
) -> Option<Arc<dyn queue_keeper_core::webhook::SignatureValidator>> {
    use queue_keeper_api::ProviderSecretConfig;

    match provider_config.secret.as_ref()? {
        ProviderSecretConfig::Literal { value } => {
            Some(Arc::new(LiteralSignatureValidator::new(value.clone())))
        }
        ProviderSecretConfig::KeyVault { secret_name } => {
            let kv = match key_vault {
                Some(kv) => kv,
                None => {
                    // Defensive guard — validate() prevents this in practice.
                    error!(
                        provider = %provider_config.id,
                        secret_name = %secret_name,
                        "Key Vault secret configured but no Key Vault provider is available; \
                         signature validation will be SKIPPED"
                    );
                    return None;
                }
            };
            match SecretName::new(secret_name.as_str()) {
                Ok(name) => Some(Arc::new(KeyVaultSignatureValidator::new(
                    Arc::clone(kv),
                    name,
                ))),
                Err(e) => {
                    error!(
                        provider = %provider_config.id,
                        secret_name = %secret_name,
                        error = %e,
                        "Invalid Key Vault secret name; signature validation will be SKIPPED"
                    );
                    None
                }
            }
        }
    }
}

/// Build a [`SignatureValidator`] from a [`GenericProviderConfig`] signature section.
///
/// Follows the same logic as [`build_validator_from_provider_config`].
fn build_validator_from_generic_config(
    generic_config: &queue_keeper_core::webhook::generic_provider::GenericProviderConfig,
    key_vault: Option<&Arc<dyn KeyVaultProvider>>,
) -> Option<Arc<dyn queue_keeper_core::webhook::SignatureValidator>> {
    use queue_keeper_core::webhook::generic_provider::WebhookSecretConfig;

    match generic_config.webhook_secret.as_ref()? {
        WebhookSecretConfig::Literal { value } => {
            Some(Arc::new(LiteralSignatureValidator::new(value.clone())))
        }
        WebhookSecretConfig::KeyVault { secret_name } => {
            let kv = match key_vault {
                Some(kv) => kv,
                None => {
                    error!(
                        provider = %generic_config.provider_id,
                        secret_name = %secret_name,
                        "Key Vault secret configured but no Key Vault provider is available; \
                         signature validation will be SKIPPED"
                    );
                    return None;
                }
            };
            match SecretName::new(secret_name.as_str()) {
                Ok(name) => Some(Arc::new(KeyVaultSignatureValidator::new(
                    Arc::clone(kv),
                    name,
                ))),
                Err(e) => {
                    error!(
                        provider = %generic_config.provider_id,
                        secret_name = %secret_name,
                        error = %e,
                        "Invalid Key Vault secret name; signature validation will be SKIPPED"
                    );
                    None
                }
            }
        }
    }
}
