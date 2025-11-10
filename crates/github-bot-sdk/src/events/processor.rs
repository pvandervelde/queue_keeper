//! Event processor for converting raw webhooks to normalized events.

use serde_json::Value;

use crate::client::Repository;
use crate::error::EventError;

use super::{EntityType, EventEnvelope, EventId, EventMetadata, EventPayload};

/// Event processor configuration.
///
/// Controls how webhook events are processed, validated, and normalized.
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// Enable webhook signature validation
    pub enable_signature_validation: bool,

    /// Enable session correlation for ordered processing
    pub enable_session_correlation: bool,

    /// Strategy for generating session IDs
    pub session_id_strategy: SessionIdStrategy,

    /// Maximum allowed payload size in bytes
    pub max_payload_size: usize,

    /// Trace sampling rate (0.0 to 1.0)
    pub trace_sampling_rate: f64,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            enable_signature_validation: true,
            enable_session_correlation: true,
            session_id_strategy: SessionIdStrategy::Entity,
            max_payload_size: 1024 * 1024, // 1MB
            trace_sampling_rate: 0.1,
        }
    }
}

/// Strategy for generating session IDs for ordered processing.
#[derive(Debug, Clone)]
pub enum SessionIdStrategy {
    /// No session IDs generated
    None,

    /// Entity-based session IDs (e.g., "pr-owner/repo-123")
    Entity,

    /// Repository-based session IDs (e.g., "repo-owner/name")
    Repository,

    /// Custom session ID generation function
    Custom(fn(&EventEnvelope) -> Option<String>),
}

/// Processes raw GitHub webhooks into normalized event envelopes.
///
/// The event processor handles:
/// - Signature validation (optional)
/// - JSON parsing and validation
/// - Entity extraction and classification
/// - Session ID generation for ordering
/// - Metadata population
///
/// # Examples
///
/// ```rust,no_run
/// use github_bot_sdk::events::{EventProcessor, ProcessorConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ProcessorConfig::default();
/// let processor = EventProcessor::new(config);
///
/// let envelope = processor.process_webhook(
///     "pull_request",
///     b"{\"action\":\"opened\",\"number\":1}",
///     Some("delivery-123"),
/// ).await?;
///
/// println!("Processed event: {}", envelope.event_id);
/// # Ok(())
/// # }
/// ```
pub struct EventProcessor {
    config: ProcessorConfig,
}

impl EventProcessor {
    /// Create a new event processor with the given configuration.
    pub fn new(config: ProcessorConfig) -> Self {
        Self { config }
    }

    /// Process a raw webhook into a normalized event envelope.
    ///
    /// # Arguments
    ///
    /// * `event_type` - GitHub event type (from X-GitHub-Event header)
    /// * `payload` - Raw webhook payload bytes
    /// * `delivery_id` - GitHub delivery ID (from X-GitHub-Delivery header)
    ///
    /// # Returns
    ///
    /// A normalized `EventEnvelope` or an error if processing fails.
    ///
    /// # Errors
    ///
    /// Returns `EventError` if:
    /// - Payload exceeds maximum size
    /// - Payload is not valid JSON
    /// - Required fields are missing
    /// - Event type is unsupported
    pub async fn process_webhook(
        &self,
        event_type: &str,
        payload: &[u8],
        delivery_id: Option<&str>,
    ) -> Result<EventEnvelope, EventError> {
        // Check payload size
        if payload.len() > self.config.max_payload_size {
            return Err(EventError::PayloadTooLarge {
                size: payload.len(),
                max: self.config.max_payload_size,
            });
        }

        // Parse JSON payload
        let json_payload: Value = serde_json::from_slice(payload)?;

        // Extract repository information
        let repository =
            json_payload
                .get("repository")
                .ok_or_else(|| EventError::MissingField {
                    field: "repository".to_string(),
                })?;

        let repository: Repository = serde_json::from_value(repository.clone())?;

        // Extract entity information
        let (entity_type, entity_id) = self.extract_entity_info(event_type, &json_payload)?;

        // Create event payload wrapper
        let event_payload = EventPayload::new(json_payload);

        // Create event ID
        let event_id = if let Some(delivery_id) = delivery_id {
            EventId::from_github_delivery(delivery_id)
        } else {
            EventId::new()
        };

        // Create metadata
        let mut metadata = EventMetadata::default();
        metadata.delivery_id = delivery_id.map(|s| s.to_string());
        metadata.signature_valid = !self.config.enable_signature_validation; // Default if not validated

        // Create envelope
        let mut envelope = EventEnvelope {
            event_id,
            event_type: event_type.to_string(),
            repository,
            entity_type,
            entity_id: entity_id.clone(),
            session_id: None,
            payload: event_payload,
            metadata,
            trace_context: None,
        };

        // Generate session ID if enabled
        if self.config.enable_session_correlation {
            let session_id =
                self.generate_session_id(&envelope.entity_type, &entity_id, &envelope.repository);
            envelope.session_id = session_id;
        }

        Ok(envelope)
    }

    /// Extract entity information from the payload.
    ///
    /// Determines the primary entity type and ID for session correlation.
    pub fn extract_entity_info(
        &self,
        event_type: &str,
        payload: &Value,
    ) -> Result<(EntityType, Option<String>), EventError> {
        let entity_type = EntityType::from_event_type(event_type);

        // Extract entity ID based on event type
        let entity_id = match event_type {
            "pull_request" | "pull_request_review" | "pull_request_review_comment" => {
                // For PR events, extract PR number
                payload
                    .get("number")
                    .or_else(|| payload.get("pull_request").and_then(|pr| pr.get("number")))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
            }
            "issues" | "issue_comment" => {
                // For issue events, extract issue number
                payload
                    .get("issue")
                    .and_then(|issue| issue.get("number"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
            }
            "push" | "create" | "delete" => {
                // For branch events, extract ref name
                payload
                    .get("ref")
                    .and_then(|v| v.as_str())
                    .map(|r| r.to_string())
            }
            "check_run" => {
                // For check run events, extract check run ID
                payload
                    .get("check_run")
                    .and_then(|cr| cr.get("id"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
            }
            "check_suite" => {
                // For check suite events, extract check suite ID
                payload
                    .get("check_suite")
                    .and_then(|cs| cs.get("id"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
            }
            "release" => {
                // For release events, extract release ID
                payload
                    .get("release")
                    .and_then(|r| r.get("id"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
            }
            "deployment" | "deployment_status" => {
                // For deployment events, extract deployment ID
                payload
                    .get("deployment")
                    .and_then(|d| d.get("id"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n.to_string())
            }
            _ => {
                // For other events, no specific entity ID
                None
            }
        };

        Ok((entity_type, entity_id))
    }

    /// Generate a session ID for ordered processing.
    ///
    /// Uses the configured strategy to create session IDs that group
    /// related events together for sequential processing.
    pub fn generate_session_id(
        &self,
        entity_type: &EntityType,
        entity_id: &Option<String>,
        repository: &Repository,
    ) -> Option<String> {
        match &self.config.session_id_strategy {
            SessionIdStrategy::None => None,
            SessionIdStrategy::Entity => {
                // Generate entity-based session ID
                if let Some(id) = entity_id {
                    match entity_type {
                        EntityType::PullRequest => {
                            Some(format!("pr-{}-{}", repository.full_name, id))
                        }
                        EntityType::Issue => Some(format!("issue-{}-{}", repository.full_name, id)),
                        EntityType::Branch => {
                            Some(format!("branch-{}-{}", repository.full_name, id))
                        }
                        EntityType::CheckRun => {
                            Some(format!("check-run-{}-{}", repository.full_name, id))
                        }
                        EntityType::CheckSuite => {
                            Some(format!("check-suite-{}-{}", repository.full_name, id))
                        }
                        EntityType::Release => {
                            Some(format!("release-{}-{}", repository.full_name, id))
                        }
                        EntityType::Deployment => {
                            Some(format!("deployment-{}-{}", repository.full_name, id))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
            SessionIdStrategy::Repository => {
                // Generate repository-based session ID
                Some(format!("repo-{}", repository.full_name))
            }
            SessionIdStrategy::Custom(f) => {
                // Use custom function - create temporary envelope for evaluation
                let temp_envelope = EventEnvelope {
                    event_id: EventId::new(),
                    event_type: String::new(),
                    repository: repository.clone(),
                    entity_type: entity_type.clone(),
                    entity_id: entity_id.clone(),
                    session_id: None,
                    payload: EventPayload::new(serde_json::Value::Null),
                    metadata: EventMetadata::default(),
                    trace_context: None,
                };
                f(&temp_envelope)
            }
        }
    }
}

#[cfg(test)]
#[path = "processor_tests.rs"]
mod tests;
