//! # Queue Integration Layer
//!
//! Integrates queue-runtime QueueClient with webhook processing for event publishing.
//!
//! This module provides the EventRouter trait and implementation for routing normalized
//! events to configured bot queues based on bot subscriptions.
//!
//! See specs/interfaces/queue-client.md for queue operations specification.
//! See specs/interfaces/bot-configuration.md for routing configuration.

use crate::{
    bot_config::{BotConfiguration, BotSubscription},
    webhook::EventEnvelope,
    BotName, EventId,
};
use async_trait::async_trait;

// Re-export queue-runtime types for convenience
pub use queue_runtime::{Message, MessageId, QueueClient, QueueError, QueueName, SessionId};

// ============================================================================
// Core Types
// ============================================================================

/// Result of event routing operation
///
/// Contains information about successful and failed deliveries to bot queues.
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    /// Event that was routed
    pub event_id: EventId,

    /// Successful deliveries
    pub successful: Vec<SuccessfulDelivery>,

    /// Failed deliveries
    pub failed: Vec<FailedDelivery>,
}

impl DeliveryResult {
    /// Create new delivery result
    pub fn new(event_id: EventId) -> Self {
        Self {
            event_id,
            successful: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Check if all deliveries were successful
    pub fn is_complete_success(&self) -> bool {
        self.failed.is_empty()
    }

    /// Check if any deliveries succeeded
    pub fn has_any_success(&self) -> bool {
        !self.successful.is_empty()
    }

    /// Check if all deliveries failed
    pub fn is_complete_failure(&self) -> bool {
        self.successful.is_empty() && !self.failed.is_empty()
    }

    /// Check if this was a no-op (no target queues)
    pub fn is_no_op(&self) -> bool {
        self.successful.is_empty() && self.failed.is_empty()
    }
}

/// Successful delivery to a bot queue
#[derive(Debug, Clone)]
pub struct SuccessfulDelivery {
    pub bot_name: BotName,
    pub queue_name: crate::QueueName,
    pub message_id: MessageId,
}

/// Failed delivery to a bot queue
#[derive(Debug, Clone)]
pub struct FailedDelivery {
    pub bot_name: BotName,
    pub queue_name: crate::QueueName,
    pub error: String,
    pub is_transient: bool,
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during queue delivery operations
#[derive(Debug, thiserror::Error)]
pub enum QueueDeliveryError {
    #[error("Failed to deliver to all target queues: {successful} succeeded, {failed} failed")]
    PartialDelivery { successful: usize, failed: usize },

    #[error("Failed to deliver to any target queue: {failures:?}")]
    CompleteFailure { failures: Vec<FailedDelivery> },

    #[error("Queue client error: {0}")]
    QueueClientError(#[from] QueueError),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

impl QueueDeliveryError {
    /// Check if error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        match self {
            Self::PartialDelivery { .. } => true, // Retry partial deliveries
            Self::CompleteFailure { failures } => {
                // Only transient if all failures are transient
                failures.iter().all(|f| f.is_transient)
            }
            Self::QueueClientError(queue_error) => queue_error.is_transient(),
            Self::SerializationError(_) => false,
            Self::ConfigurationError(_) => false,
        }
    }

    /// Get retry classification
    pub fn should_retry(&self) -> bool {
        self.is_transient()
    }
}

// ============================================================================
// Event Router Trait
// ============================================================================

/// Interface for routing events to bot queues
///
/// Implementations determine target queues based on bot configuration and
/// handle message delivery through the queue client.
#[async_trait]
pub trait EventRouter: Send + Sync {
    /// Route event to configured bot queues
    ///
    /// # Arguments
    ///
    /// * `event` - Normalized event envelope to route
    /// * `config` - Bot configuration defining routing rules
    /// * `queue_client` - Queue client for message delivery
    ///
    /// # Returns
    ///
    /// `Ok(DeliveryResult)` - Details of successful and failed deliveries
    /// `Err(QueueDeliveryError)` - Critical routing failure
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - All delivery attempts fail
    /// - Serialization fails
    /// - Configuration is invalid
    async fn route_event(
        &self,
        event: &EventEnvelope,
        config: &BotConfiguration,
        queue_client: &dyn QueueClient,
    ) -> Result<DeliveryResult, QueueDeliveryError>;
}

// ============================================================================
// Default Implementation
// ============================================================================

/// Default event router implementation
///
/// Routes events to all matching bot subscriptions, handling both ordered
/// and unordered delivery modes.
pub struct DefaultEventRouter;

impl DefaultEventRouter {
    /// Create new default event router
    pub fn new() -> Self {
        Self
    }

    /// Create queue message from event envelope
    ///
    /// Serializes event to JSON and creates Message with appropriate metadata.
    fn create_queue_message(
        &self,
        event: &EventEnvelope,
        bot: &BotSubscription,
    ) -> Result<Message, QueueDeliveryError> {
        // Serialize event envelope to JSON
        let body = serde_json::to_vec(event)
            .map_err(|e| QueueDeliveryError::SerializationError(e.to_string()))?;

        // Create message with metadata
        let mut message = Message::new(body.into());

        // Add session ID for ordered processing
        if bot.ordered {
            // Convert core SessionId to queue-runtime SessionId
            let session_id =
                SessionId::new(event.session_id.as_str().to_string()).map_err(|e| {
                    QueueDeliveryError::SerializationError(format!("Invalid session ID: {}", e))
                })?;
            message = message.with_session_id(session_id);
        }

        // Add correlation ID for tracing
        message = message.with_correlation_id(event.correlation_id.to_string());

        // Add bot name as attribute
        message = message.with_attribute("bot_name".to_string(), bot.name.as_str().to_string());

        // Add event type as attribute
        message = message.with_attribute("event_type".to_string(), event.event_type.clone());

        Ok(message)
    }

    /// Handle delivery failures and determine error response
    fn handle_delivery_failures(
        &self,
        result: &DeliveryResult,
    ) -> Result<DeliveryResult, QueueDeliveryError> {
        if result.is_complete_success() || result.is_no_op() {
            Ok(result.clone())
        } else if result.is_complete_failure() {
            Err(QueueDeliveryError::CompleteFailure {
                failures: result.failed.clone(),
            })
        } else {
            // Partial delivery - some succeeded, some failed
            Err(QueueDeliveryError::PartialDelivery {
                successful: result.successful.len(),
                failed: result.failed.len(),
            })
        }
    }
}

impl Default for DefaultEventRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventRouter for DefaultEventRouter {
    async fn route_event(
        &self,
        event: &EventEnvelope,
        config: &BotConfiguration,
        queue_client: &dyn QueueClient,
    ) -> Result<DeliveryResult, QueueDeliveryError> {
        let mut result = DeliveryResult::new(event.event_id.clone());

        // Get target bots from configuration
        let target_bots = config.get_target_bots(event);

        // If no bots match, return successful no-op
        if target_bots.is_empty() {
            return Ok(result);
        }

        // Attempt delivery to each target bot queue
        for bot in target_bots {
            // Convert core QueueName to queue-runtime QueueName
            let queue_name = match QueueName::new(bot.queue.as_str().to_string()) {
                Ok(qn) => qn,
                Err(e) => {
                    result.failed.push(FailedDelivery {
                        bot_name: bot.name.clone(),
                        queue_name: bot.queue.clone(),
                        error: format!("Invalid queue name: {}", e),
                        is_transient: false,
                    });
                    continue;
                }
            };

            // Create message for this bot
            let message = match self.create_queue_message(event, bot) {
                Ok(msg) => msg,
                Err(e) => {
                    // Serialization failure - permanent error
                    result.failed.push(FailedDelivery {
                        bot_name: bot.name.clone(),
                        queue_name: bot.queue.clone(),
                        error: e.to_string(),
                        is_transient: false,
                    });
                    continue;
                }
            };

            // Send message to queue
            match queue_client.send_message(&queue_name, message).await {
                Ok(message_id) => {
                    result.successful.push(SuccessfulDelivery {
                        bot_name: bot.name.clone(),
                        queue_name: bot.queue.clone(),
                        message_id,
                    });
                }
                Err(queue_error) => {
                    result.failed.push(FailedDelivery {
                        bot_name: bot.name.clone(),
                        queue_name: bot.queue.clone(),
                        error: queue_error.to_string(),
                        is_transient: queue_error.is_transient(),
                    });
                }
            }
        }

        // Check results and return appropriate response
        self.handle_delivery_failures(&result)
    }
}

#[cfg(test)]
#[path = "queue_integration_tests.rs"]
mod tests;
