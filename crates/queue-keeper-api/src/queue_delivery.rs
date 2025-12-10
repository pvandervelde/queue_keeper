//! # Queue Delivery Module
//!
//! Implements async queue delivery with retry loop for distributing normalized
//! events to bot queues after immediate webhook response.
//!
//! This module separates the fast path (immediate HTTP response) from the slow
//! path (queue delivery with retries), ensuring GitHub receives a response
//! within the 10-second timeout.
//!
//! See specs/interfaces/queue-client.md for queue operations specification.
//! See specs/constraints.md for retry and performance requirements.

use crate::dlq_storage::{DlqReason, DlqStorageService, FailedEventRecord, FailedQueueInfo};
use crate::retry::{RetryPolicy, RetryState};
use queue_keeper_core::{
    bot_config::BotConfiguration,
    queue_integration::{DeliveryResult, EventRouter, FailedDelivery, SuccessfulDelivery},
    webhook::EventEnvelope,
    EventId, Timestamp,
};
use queue_runtime::QueueClient;
use std::sync::Arc;
use tracing::{error, info, warn};

// ============================================================================
// Queue Delivery Configuration
// ============================================================================

/// Configuration for queue delivery retry behavior
///
/// Encapsulates retry policy and DLQ settings for queue delivery operations.
#[derive(Debug, Clone)]
pub struct QueueDeliveryConfig {
    /// Retry policy for transient failures
    pub retry_policy: RetryPolicy,

    /// Enable DLQ persistence for permanent failures
    pub enable_dlq: bool,

    /// Optional DLQ storage service for persisting failed events
    /// When None and enable_dlq is true, failures will be logged but not persisted
    pub dlq_service: Option<Arc<DlqStorageService>>,
}

impl Default for QueueDeliveryConfig {
    fn default() -> Self {
        Self {
            retry_policy: RetryPolicy::default(),
            enable_dlq: true,
            dlq_service: None,
        }
    }
}

impl QueueDeliveryConfig {
    /// Create a new configuration with a DLQ service
    pub fn with_dlq_service(mut self, dlq_service: Arc<DlqStorageService>) -> Self {
        self.dlq_service = Some(dlq_service);
        self
    }
}

// ============================================================================
// Queue Delivery Result Types
// ============================================================================

/// Outcome of the async queue delivery process
///
/// Represents the final state after all retries have been exhausted.
#[derive(Debug, Clone)]
pub enum QueueDeliveryOutcome {
    /// All target queues received the event successfully
    AllQueuesSucceeded {
        event_id: EventId,
        successful_count: usize,
    },

    /// Some or all queues failed after exhausting retries
    SomeQueuesFailed {
        event_id: EventId,
        successful_count: usize,
        failed_count: usize,
        /// Indicates if failed events were persisted to DLQ
        persisted_to_dlq: bool,
    },

    /// No target queues matched the event (no-op)
    NoTargetQueues { event_id: EventId },

    /// Complete failure - no queues received the event
    CompleteFailure {
        event_id: EventId,
        error: String,
        /// Indicates if the event was persisted to DLQ
        persisted_to_dlq: bool,
    },
}

impl QueueDeliveryOutcome {
    /// Check if delivery was completely successful
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            QueueDeliveryOutcome::AllQueuesSucceeded { .. }
                | QueueDeliveryOutcome::NoTargetQueues { .. }
        )
    }

    /// Check if any failures occurred
    pub fn has_failures(&self) -> bool {
        matches!(
            self,
            QueueDeliveryOutcome::SomeQueuesFailed { .. }
                | QueueDeliveryOutcome::CompleteFailure { .. }
        )
    }
}

// ============================================================================
// Async Queue Delivery
// ============================================================================

/// Deliver event to bot queues with retry logic
///
/// This function implements the async delivery loop that runs in a spawned task
/// after the immediate HTTP response. It handles:
///
/// 1. Initial delivery attempt to all target queues
/// 2. Retry logic with exponential backoff for transient failures
/// 3. Partial failure handling (retry only failed queues)
/// 4. DLQ persistence for permanent failures or exhausted retries
///
/// # Arguments
///
/// * `event` - Normalized event envelope to deliver
/// * `event_router` - Router for determining target queues and delivery
/// * `bot_config` - Bot subscription configuration
/// * `queue_client` - Queue client for message delivery
/// * `delivery_config` - Retry and DLQ configuration
///
/// # Returns
///
/// `QueueDeliveryOutcome` indicating the final delivery state
///
/// # Example
///
/// ```rust,ignore
/// let outcome = deliver_event_to_queues(
///     event,
///     event_router.clone(),
///     bot_config.clone(),
///     queue_client.clone(),
///     QueueDeliveryConfig::default(),
/// ).await;
///
/// match outcome {
///     QueueDeliveryOutcome::AllQueuesSucceeded { .. } => info!("Delivery successful"),
///     QueueDeliveryOutcome::SomeQueuesFailed { persisted_to_dlq, .. } => {
///         if persisted_to_dlq {
///             warn!("Some deliveries failed, events persisted to DLQ");
///         }
///     }
///     _ => {}
/// }
/// ```
pub async fn deliver_event_to_queues(
    event: EventEnvelope,
    event_router: Arc<dyn EventRouter>,
    bot_config: Arc<BotConfiguration>,
    queue_client: Arc<dyn QueueClient>,
    delivery_config: QueueDeliveryConfig,
) -> QueueDeliveryOutcome {
    let event_id = event.event_id;
    let mut retry_state = RetryState::new();
    let first_attempt_at = Timestamp::now();

    loop {
        // Attempt delivery to all target queues
        match event_router
            .route_event(&event, &bot_config, queue_client.as_ref())
            .await
        {
            Ok(result) if result.is_no_op() => {
                // No target queues matched (must check before is_complete_success
                // because is_complete_success is also true when no targets)
                info!(
                    event_id = %event_id,
                    "No target queues matched for event"
                );

                return QueueDeliveryOutcome::NoTargetQueues { event_id };
            }

            Ok(result) if result.is_complete_success() => {
                // All deliveries succeeded
                info!(
                    event_id = %event_id,
                    successful_count = result.successful.len(),
                    total_attempts = retry_state.total_attempts,
                    "Event delivered to all target queues"
                );

                return QueueDeliveryOutcome::AllQueuesSucceeded {
                    event_id,
                    successful_count: result.successful.len(),
                };
            }

            Ok(result) => {
                // Partial or complete failure - check if we should retry
                let transient_failures: Vec<_> =
                    result.failed.iter().filter(|f| f.is_transient).collect();

                if !transient_failures.is_empty()
                    && retry_state.can_retry(&delivery_config.retry_policy)
                {
                    // Retry transient failures with backoff
                    let delay = retry_state.get_delay(&delivery_config.retry_policy);

                    warn!(
                        event_id = %event_id,
                        transient_failures = transient_failures.len(),
                        permanent_failures = result.failed.len() - transient_failures.len(),
                        attempt = retry_state.total_attempts,
                        delay_ms = delay.as_millis(),
                        "Retrying transient queue delivery failures"
                    );

                    tokio::time::sleep(delay).await;
                    retry_state.next_attempt();
                    continue;
                }

                // Max retries exceeded or all failures are permanent
                return handle_final_delivery_result(
                    &event,
                    result,
                    retry_state.total_attempts,
                    first_attempt_at,
                    &delivery_config,
                )
                .await;
            }

            Err(error) => {
                // Critical routing error
                if error.is_transient() && retry_state.can_retry(&delivery_config.retry_policy) {
                    let delay = retry_state.get_delay(&delivery_config.retry_policy);

                    warn!(
                        event_id = %event_id,
                        error = %error,
                        attempt = retry_state.total_attempts,
                        delay_ms = delay.as_millis(),
                        "Retrying after routing error"
                    );

                    tokio::time::sleep(delay).await;
                    retry_state.next_attempt();
                    continue;
                }

                // Permanent error or max retries exceeded
                error!(
                    event_id = %event_id,
                    error = %error,
                    total_attempts = retry_state.total_attempts,
                    "Queue delivery failed permanently"
                );

                // Persist to DLQ if enabled
                let persisted_to_dlq = persist_routing_error_to_dlq(
                    &event,
                    &error.to_string(),
                    retry_state.total_attempts,
                    first_attempt_at,
                    &delivery_config,
                )
                .await;

                return QueueDeliveryOutcome::CompleteFailure {
                    event_id,
                    error: error.to_string(),
                    persisted_to_dlq,
                };
            }
        }
    }
}

/// Handle the final delivery result after retries are exhausted
///
/// Processes remaining failures and optionally persists to DLQ.
async fn handle_final_delivery_result(
    event: &EventEnvelope,
    result: DeliveryResult,
    total_attempts: u32,
    first_attempt_at: Timestamp,
    delivery_config: &QueueDeliveryConfig,
) -> QueueDeliveryOutcome {
    let event_id = event.event_id;
    let successful_count = result.successful.len();
    let failed_count = result.failed.len();

    if failed_count == 0 {
        return QueueDeliveryOutcome::AllQueuesSucceeded {
            event_id,
            successful_count,
        };
    }

    // Log each failure
    for failure in &result.failed {
        error!(
            event_id = %event_id,
            bot_name = %failure.bot_name,
            queue_name = %failure.queue_name,
            error = %failure.error,
            is_transient = failure.is_transient,
            "Queue delivery failed for bot"
        );
    }

    // Persist failed deliveries to DLQ
    let persisted_to_dlq = persist_delivery_failures_to_dlq(
        event,
        &result.successful,
        &result.failed,
        total_attempts,
        first_attempt_at,
        delivery_config,
    )
    .await;

    if successful_count > 0 {
        warn!(
            event_id = %event_id,
            successful_count = successful_count,
            failed_count = failed_count,
            total_attempts = total_attempts,
            persisted_to_dlq = persisted_to_dlq,
            "Partial queue delivery completed with failures"
        );

        QueueDeliveryOutcome::SomeQueuesFailed {
            event_id,
            successful_count,
            failed_count,
            persisted_to_dlq,
        }
    } else {
        error!(
            event_id = %event_id,
            failed_count = failed_count,
            total_attempts = total_attempts,
            persisted_to_dlq = persisted_to_dlq,
            "Complete queue delivery failure"
        );

        QueueDeliveryOutcome::CompleteFailure {
            event_id,
            error: format!("All {} queue deliveries failed", failed_count),
            persisted_to_dlq,
        }
    }
}

/// Persist delivery failures to DLQ storage
///
/// Creates a FailedEventRecord and persists it to blob storage.
async fn persist_delivery_failures_to_dlq(
    event: &EventEnvelope,
    successful: &[SuccessfulDelivery],
    failed: &[FailedDelivery],
    total_attempts: u32,
    first_attempt_at: Timestamp,
    delivery_config: &QueueDeliveryConfig,
) -> bool {
    if !delivery_config.enable_dlq {
        return false;
    }

    let dlq_service = match &delivery_config.dlq_service {
        Some(service) => service,
        None => {
            warn!(
                event_id = %event.event_id,
                failed_count = failed.len(),
                "DLQ service not configured - failed events not persisted"
            );
            return false;
        }
    };

    // Build failed queue info
    let failed_queues: Vec<FailedQueueInfo> = failed
        .iter()
        .map(|f| FailedQueueInfo {
            bot_name: f.bot_name.as_str().to_string(),
            queue_name: f.queue_name.as_str().to_string(),
            error: f.error.clone(),
            was_transient: f.is_transient,
        })
        .collect();

    // Build successful queue names
    let successful_queues: Vec<String> = successful
        .iter()
        .map(|s| format!("{}/{}", s.bot_name.as_str(), s.queue_name.as_str()))
        .collect();

    // Determine DLQ reason
    let reason = if successful.is_empty() {
        DlqReason::AllQueuesFailed {
            queue_count: failed.len(),
        }
    } else {
        DlqReason::RetriesExhausted {
            attempts: total_attempts,
        }
    };

    // Create the failed event record
    let record = FailedEventRecord::new(
        event.clone(),
        reason,
        failed_queues,
        successful_queues,
        total_attempts,
        first_attempt_at,
    );

    // Persist to DLQ
    match dlq_service.persist_failed_event(&record).await {
        Ok(blob_path) => {
            info!(
                event_id = %event.event_id,
                blob_path = %blob_path,
                failed_count = failed.len(),
                "Failed event persisted to DLQ"
            );
            true
        }
        Err(e) => {
            error!(
                event_id = %event.event_id,
                error = %e,
                "Failed to persist event to DLQ - event may be lost"
            );
            false
        }
    }
}

/// Persist routing error to DLQ storage
///
/// Creates a FailedEventRecord for routing errors and persists it.
async fn persist_routing_error_to_dlq(
    event: &EventEnvelope,
    error: &str,
    total_attempts: u32,
    first_attempt_at: Timestamp,
    delivery_config: &QueueDeliveryConfig,
) -> bool {
    if !delivery_config.enable_dlq {
        return false;
    }

    let dlq_service = match &delivery_config.dlq_service {
        Some(service) => service,
        None => {
            warn!(
                event_id = %event.event_id,
                error = %error,
                "DLQ service not configured - routing error not persisted"
            );
            return false;
        }
    };

    let reason = DlqReason::RoutingError {
        error: error.to_string(),
    };

    let record = FailedEventRecord::new(
        event.clone(),
        reason,
        vec![], // No specific queue failures for routing errors
        vec![], // No successful deliveries
        total_attempts,
        first_attempt_at,
    );

    match dlq_service.persist_failed_event(&record).await {
        Ok(blob_path) => {
            info!(
                event_id = %event.event_id,
                blob_path = %blob_path,
                "Routing error persisted to DLQ"
            );
            true
        }
        Err(e) => {
            error!(
                event_id = %event.event_id,
                error = %e,
                "Failed to persist routing error to DLQ"
            );
            false
        }
    }
}

/// Spawn an async task to deliver event to queues
///
/// This is the entry point called from `handle_webhook()` after the immediate
/// response is sent. The task runs in the background and handles all retry
/// logic independently.
///
/// # Arguments
///
/// * `event` - Normalized event envelope to deliver
/// * `event_router` - Router for determining target queues and delivery
/// * `bot_config` - Bot subscription configuration
/// * `queue_client` - Queue client for message delivery
/// * `delivery_config` - Retry and DLQ configuration
///
/// # Returns
///
/// `tokio::task::JoinHandle` for the spawned delivery task
///
/// # Example
///
/// ```rust,ignore
/// // In handle_webhook():
/// let handle = spawn_queue_delivery(
///     event_envelope.clone(),
///     state.event_router.clone(),
///     state.bot_config.clone(),
///     state.queue_client.clone(),
///     QueueDeliveryConfig::default(),
/// );
///
/// // Fire-and-forget: we don't await the handle, let it complete in background
/// // The handle can be stored for monitoring/testing if needed
/// ```
pub fn spawn_queue_delivery(
    event: EventEnvelope,
    event_router: Arc<dyn EventRouter>,
    bot_config: Arc<BotConfiguration>,
    queue_client: Arc<dyn QueueClient>,
    delivery_config: QueueDeliveryConfig,
) -> tokio::task::JoinHandle<QueueDeliveryOutcome> {
    let event_id = event.event_id;

    tokio::spawn(async move {
        info!(
            event_id = %event_id,
            "Starting async queue delivery"
        );

        let outcome = deliver_event_to_queues(
            event,
            event_router,
            bot_config,
            queue_client,
            delivery_config,
        )
        .await;

        match &outcome {
            QueueDeliveryOutcome::AllQueuesSucceeded {
                successful_count, ..
            } => {
                info!(
                    event_id = %event_id,
                    successful_count = successful_count,
                    "Async queue delivery completed successfully"
                );
            }
            QueueDeliveryOutcome::NoTargetQueues { .. } => {
                info!(
                    event_id = %event_id,
                    "Async queue delivery completed (no targets)"
                );
            }
            QueueDeliveryOutcome::SomeQueuesFailed {
                successful_count,
                failed_count,
                ..
            } => {
                warn!(
                    event_id = %event_id,
                    successful_count = successful_count,
                    failed_count = failed_count,
                    "Async queue delivery completed with partial failures"
                );
            }
            QueueDeliveryOutcome::CompleteFailure { error, .. } => {
                error!(
                    event_id = %event_id,
                    error = error,
                    "Async queue delivery failed completely"
                );
            }
        }

        outcome
    })
}

#[cfg(test)]
#[path = "queue_delivery_tests.rs"]
mod tests;
