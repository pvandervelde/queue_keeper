//! Webhook ingestion handler.
//!
//! Exposes [`handle_provider_webhook`] which is registered at
//! `POST /webhook/{provider}`.

use crate::{
    queue_delivery::spawn_queue_delivery, responses::store_wrapped_event_to_blob, AppState,
    WebhookHandlerError, WebhookResponse,
};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Json,
};
use bytes::Bytes;
use queue_keeper_core::{
    monitoring::MetricsCollector,
    webhook::{ProcessingOutput, WebhookHeaders, WebhookRequest},
};
use std::collections::HashMap;
use tracing::{error, info, instrument, warn};

/// Handle a webhook for a specific provider.
///
/// Routes `POST /webhook/{provider}` to the processor registered under that
/// provider name. Returns `404 Not Found` when the provider is unknown.
///
/// # Request Flow
///
/// 1. Extract provider name from the URL path.
/// 2. Look it up in the [`ProviderRegistry`]; return 404 if absent.
/// 3. Parse provider-agnostic webhook headers.
/// 4. Delegate to the provider's [`WebhookProcessor::process_webhook`].
/// 5. Return `200 OK` with [`WebhookResponse`] on success.
///
/// # Errors
///
/// - [`WebhookHandlerError::ProviderNotFound`] when the provider is not registered.
/// - [`WebhookHandlerError::InvalidHeaders`] when required headers are missing or malformed.
/// - [`WebhookHandlerError::ProcessingFailed`] when the processor pipeline fails.
#[instrument(skip(state, headers, body), fields(provider = %provider))]
pub async fn handle_provider_webhook(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>, WebhookHandlerError> {
    info!(provider = %provider, "Received webhook request");

    // Resolve provider – return 404 for unknown providers before any further work
    let processor = state.provider_registry.get(&provider).ok_or_else(|| {
        WebhookHandlerError::ProviderNotFound {
            provider: provider.clone(),
        }
    })?;

    // Start timing for metrics
    let start = std::time::Instant::now();

    // Convert headers to HashMap (lowercase keys for consistent lookup)
    let header_map: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    // Determine whether this is a generic (non-GitHub) provider.
    // Generic providers do not send GitHub-specific headers, so we use a
    // relaxed parser that falls back to safe defaults instead of failing.
    //
    // The set is pre-built at startup (O(1) lookup here vs. O(n) scan).
    //
    // Note: generic providers do not support `allowed_event_types` filtering
    // — all event types are accepted regardless of configuration. If you need
    // per-event filtering for a generic provider, implement it downstream.
    let is_generic_provider = state.generic_provider_ids.contains(&provider);

    let webhook_headers = if is_generic_provider {
        // For generic providers, never fail on missing GitHub headers — the
        // provider's own process_webhook will re-extract values from raw headers.
        WebhookHeaders::from_http_headers_relaxed(&header_map)
    } else {
        // For GitHub and other strict providers, require all GitHub-specific headers.
        match WebhookHeaders::from_http_headers(&header_map) {
            Ok(h) => h,
            Err(e) => {
                let duration = start.elapsed();
                state.metrics.record_webhook_request(duration, false);
                state.metrics.record_webhook_validation_failure();
                return Err(WebhookHandlerError::InvalidHeaders(e));
            }
        }
    };

    // Enforce per-provider allowed_event_types if configured.
    // An empty list means all event types are accepted.
    //
    // Note: require_signature enforcement is delegated to the processor's
    // SignatureValidator. When a SignatureValidator is wired into the
    // DefaultWebhookProcessor it will reject requests with an invalid or
    // missing signature regardless of the ProviderConfig setting.
    let provider_config = state.config.providers.iter().find(|p| p.id == provider);
    if let Some(pc) = provider_config {
        if !pc.allowed_event_types.is_empty()
            && !pc.allowed_event_types.contains(&webhook_headers.event_type)
        {
            let duration = start.elapsed();
            state.metrics.record_webhook_request(duration, false);
            state.metrics.record_webhook_validation_failure();
            return Err(WebhookHandlerError::InvalidHeaders(
                queue_keeper_core::ValidationError::InvalidFormat {
                    // Use a provider-neutral field name so non-GitHub providers
                    // receive a sensible error rather than a GitHub header name.
                    field: "event-type".to_string(),
                    message: format!(
                        "event type '{}' is not in the allowed list for provider '{}'",
                        webhook_headers.event_type, provider
                    ),
                },
            ));
        }
    }

    // Create webhook request, carrying the full header map so generic providers
    // can resolve FieldSource::Header values from any header name.
    let webhook_request = WebhookRequest::with_raw_headers(webhook_headers, header_map, body);

    // Delegate to the provider-specific processor
    let processing_output = match processor.process_webhook(webhook_request).await {
        Ok(output) => output,
        Err(e) => {
            let duration = start.elapsed();
            state.metrics.record_webhook_request(duration, false);
            return Err(WebhookHandlerError::ProcessingFailed(e));
        }
    };

    info!(
        event_id = %processing_output.event_id(),
        event_type = processing_output.event_type().unwrap_or("unknown"),
        session_id = ?processing_output.session_id(),
        provider = %provider,
        "Successfully processed webhook - returning immediate response"
    );

    let duration = start.elapsed();
    state.metrics.record_webhook_request(duration, true);

    let event_id = processing_output.event_id();
    let session_id = processing_output.session_id().cloned();

    // Spawn async queue delivery for wrapped events (fire-and-forget).
    // Direct-mode events are forwarded as raw payloads and do not go through
    // the event router.
    if let ProcessingOutput::Wrapped(wrapped_event) = processing_output {
        // Persist the wrapped event to blob storage so that /api/events queries
        // return real data. This is fire-and-forget: a storage failure does not
        // fail the webhook response — the event has already been enqueued for
        // delivery. Storage errors are logged for investigation.
        if let Some(ref blob_storage) = state.event_blob_storage {
            let event_to_persist = wrapped_event.clone();
            let storage = blob_storage.clone();
            let persist_event_id = event_id;
            tokio::spawn(async move {
                if let Err(e) =
                    store_wrapped_event_to_blob(storage.as_ref(), &event_to_persist).await
                {
                    warn!(
                        event_id = %persist_event_id,
                        error = %e,
                        "Failed to persist WrappedEvent to blob; \
                         event was delivered but will not appear in /api/events"
                    );
                }
            });
        }

        if let Some(queue_client) = &state.queue_client {
            let handle = spawn_queue_delivery(
                wrapped_event,
                state.event_router.clone(),
                state.bot_config.clone(),
                queue_client.clone(),
                state.delivery_config.clone(),
            );
            // Detach the task but monitor for panics: if the delivery task
            // panics the JoinHandle will hold the panic payload until dropped.
            // Spawning a watcher task ensures the panic is surfaced in logs
            // rather than silently discarded, and allows tracing the event_id.
            let logged_event_id = event_id;
            tokio::spawn(async move {
                if let Err(join_err) = handle.await {
                    if join_err.is_panic() {
                        error!(
                            event_id = %logged_event_id,
                            "Queue delivery task panicked — event may not have been delivered"
                        );
                    }
                }
            });
        }
    }

    Ok(Json(WebhookResponse {
        event_id,
        session_id,
        status: "processed".to_string(),
        message: "Webhook processed successfully".to_string(),
    }))
}
