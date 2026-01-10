//! Webhook receiver for HTTP intake and async processing coordination.
//!
//! This module provides the core webhook receiving functionality that applications
//! use to accept GitHub webhooks, validate them, and dispatch to handlers. The
//! receiver implements the fire-and-forget pattern to ensure fast HTTP responses.
//!
//! # Fire-and-Forget Pattern
//!
//! The receiver follows this pattern:
//! 1. Extract headers and payload from HTTP request (fast)
//! 2. Validate signature (fast, ~10ms)
//! 3. Process/normalize event (fast, ~5ms)
//! 4. Return HTTP response immediately (target <100ms total)
//! 5. Spawn async tasks for handlers (non-blocking)
//!
//! This ensures GitHub receives a response within the 10-second timeout while
//! allowing handlers to perform longer operations.
//!
//! # Examples
//!
//! ```rust,no_run
//! use github_bot_sdk::webhook::{WebhookReceiver, WebhookHandler, WebhookRequest};
//! use github_bot_sdk::auth::SecretProvider;
//! use github_bot_sdk::events::{EventProcessor, ProcessorConfig};
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! # async fn example(secret_provider: Arc<dyn SecretProvider>) {
//! // Create receiver with dependencies
//! let processor = EventProcessor::new(ProcessorConfig::default());
//! let receiver = WebhookReceiver::new(secret_provider, processor);
//!
//! // Register handlers
//! // receiver.add_handler(my_handler);
//!
//! // Process incoming webhook
//! let headers = HashMap::from([
//!     ("x-github-event".to_string(), "pull_request".to_string()),
//!     ("x-github-delivery".to_string(), "12345".to_string()),
//!     ("x-hub-signature-256".to_string(), "sha256=abc...".to_string()),
//! ]);
//! let body = bytes::Bytes::from_static(b"{\"action\":\"opened\"}");
//! let request = WebhookRequest::new(headers, body);
//!
//! let response = receiver.receive_webhook(request).await;
//! println!("Status: {}", response.status_code());
//! # }
//! ```

use crate::auth::SecretProvider;
use crate::events::EventProcessor;
use crate::webhook::handler::WebhookHandler;
use crate::webhook::validation::SignatureValidator;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

// ============================================================================
// Webhook Request/Response Types
// ============================================================================

/// Raw HTTP webhook request data.
///
/// Contains the headers and body from an incoming GitHub webhook HTTP request.
/// Headers should include `X-GitHub-Event`, `X-GitHub-Delivery`, and
/// `X-Hub-Signature-256`.
///
/// # Examples
///
/// ```rust
/// use github_bot_sdk::webhook::WebhookRequest;
/// use std::collections::HashMap;
///
/// let headers = HashMap::from([
///     ("x-github-event".to_string(), "pull_request".to_string()),
///     ("x-github-delivery".to_string(), "12345".to_string()),
/// ]);
/// let body = b"{\"action\":\"opened\"}".to_vec();
///
/// let request = WebhookRequest::new(headers, body.into());
/// assert_eq!(request.event_type(), Some("pull_request"));
/// ```
#[derive(Debug, Clone)]
pub struct WebhookRequest {
    headers: HashMap<String, String>,
    body: Bytes,
}

impl WebhookRequest {
    /// Create a new webhook request.
    ///
    /// # Arguments
    ///
    /// * `headers` - HTTP headers (case-insensitive keys recommended)
    /// * `body` - Raw webhook payload bytes
    pub fn new(headers: HashMap<String, String>, body: Bytes) -> Self {
        Self { headers, body }
    }

    /// Get the event type from X-GitHub-Event header.
    pub fn event_type(&self) -> Option<&str> {
        self.headers
            .get("x-github-event")
            .or_else(|| self.headers.get("X-GitHub-Event"))
            .map(|s| s.as_str())
    }

    /// Get the delivery ID from X-GitHub-Delivery header.
    pub fn delivery_id(&self) -> Option<&str> {
        self.headers
            .get("x-github-delivery")
            .or_else(|| self.headers.get("X-GitHub-Delivery"))
            .map(|s| s.as_str())
    }

    /// Get the signature from X-Hub-Signature-256 header.
    pub fn signature(&self) -> Option<&str> {
        self.headers
            .get("x-hub-signature-256")
            .or_else(|| self.headers.get("X-Hub-Signature-256"))
            .map(|s| s.as_str())
    }

    /// Get the raw payload bytes.
    pub fn payload(&self) -> &[u8] {
        &self.body
    }

    /// Get all headers.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

/// HTTP response for webhook requests.
///
/// Represents the immediate HTTP response sent to GitHub after webhook
/// validation and processing (but before handler execution).
#[derive(Debug, Clone)]
pub enum WebhookResponse {
    /// 200 OK - Webhook accepted and queued for processing
    Ok { message: String, event_id: String },

    /// 401 Unauthorized - Invalid or missing signature
    Unauthorized { message: String },

    /// 400 Bad Request - Malformed request (missing headers, invalid JSON)
    BadRequest { message: String },

    /// 500 Internal Server Error - Processing failed
    InternalError { message: String },
}

impl WebhookResponse {
    /// Get the HTTP status code for this response.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Ok { .. } => 200,
            Self::Unauthorized { .. } => 401,
            Self::BadRequest { .. } => 400,
            Self::InternalError { .. } => 500,
        }
    }

    /// Get the response message.
    pub fn message(&self) -> &str {
        match self {
            Self::Ok { message, .. } => message,
            Self::Unauthorized { message } => message,
            Self::BadRequest { message } => message,
            Self::InternalError { message } => message,
        }
    }

    /// Check if response indicates success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

// ============================================================================
// Webhook Receiver
// ============================================================================

/// Webhook receiver for processing incoming GitHub webhooks.
///
/// The receiver coordinates validation, event processing, and handler
/// execution using a fire-and-forget pattern to ensure fast HTTP responses.
///
/// # Architecture
///
/// - Validates signatures using SignatureValidator
/// - Processes events using EventProcessor
/// - Dispatches to registered WebhookHandlers asynchronously
/// - Returns HTTP responses within 100ms (target)
///
/// # Examples
///
/// ```rust,no_run
/// use github_bot_sdk::webhook::WebhookReceiver;
/// use github_bot_sdk::auth::SecretProvider;
/// use github_bot_sdk::events::{EventProcessor, ProcessorConfig};
/// use std::sync::Arc;
///
/// # async fn example(secret_provider: Arc<dyn SecretProvider>) -> Result<(), Box<dyn std::error::Error>> {
/// let processor = EventProcessor::new(ProcessorConfig::default());
/// let receiver = WebhookReceiver::new(secret_provider, processor);
/// # Ok(())
/// # }
/// ```
pub struct WebhookReceiver {
    validator: SignatureValidator,
    processor: EventProcessor,
    handlers: Arc<RwLock<Vec<Arc<dyn WebhookHandler>>>>,
}

impl WebhookReceiver {
    /// Create a new webhook receiver.
    ///
    /// # Arguments
    ///
    /// * `secrets` - Provider for retrieving webhook secrets
    /// * `processor` - Event processor for normalizing webhooks
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use github_bot_sdk::webhook::WebhookReceiver;
    /// # use github_bot_sdk::auth::SecretProvider;
    /// # use github_bot_sdk::events::{EventProcessor, ProcessorConfig};
    /// # use std::sync::Arc;
    /// # async fn example(secret_provider: Arc<dyn SecretProvider>) -> Result<(), Box<dyn std::error::Error>> {
    /// let processor = EventProcessor::new(ProcessorConfig::default());
    /// let receiver = WebhookReceiver::new(secret_provider, processor);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(secrets: Arc<dyn SecretProvider>, processor: EventProcessor) -> Self {
        let validator = SignatureValidator::new(secrets);

        Self {
            validator,
            processor,
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a webhook handler.
    ///
    /// Handlers are invoked asynchronously after the HTTP response is sent.
    /// Multiple handlers can be registered and will execute concurrently.
    ///
    /// # Arguments
    ///
    /// * `handler` - The handler implementation to register
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use github_bot_sdk::webhook::{WebhookReceiver, WebhookHandler};
    /// # use github_bot_sdk::auth::SecretProvider;
    /// # use github_bot_sdk::events::{EventProcessor, ProcessorConfig, EventEnvelope};
    /// # use std::sync::Arc;
    /// # use async_trait::async_trait;
    /// # struct MyHandler;
    /// # #[async_trait]
    /// # impl WebhookHandler for MyHandler {
    /// #     async fn handle_event(&self, envelope: &EventEnvelope) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    /// #         Ok(())
    /// #     }
    /// # }
    /// # async fn example(secret_provider: Arc<dyn SecretProvider>) -> Result<(), Box<dyn std::error::Error>> {
    /// let processor = EventProcessor::new(ProcessorConfig::default());
    /// let mut receiver = WebhookReceiver::new(secret_provider, processor);
    ///
    /// receiver.add_handler(Arc::new(MyHandler)).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_handler(&mut self, handler: Arc<dyn WebhookHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }

    /// Process an incoming webhook request.
    ///
    /// This is the main entry point for webhook processing. It performs
    /// validation, event processing, and returns an immediate HTTP response.
    /// Handler execution happens asynchronously after the response is returned.
    ///
    /// # Processing Steps
    ///
    /// 1. Extract headers (event type, delivery ID, signature)
    /// 2. Validate signature using webhook secret
    /// 3. Process event (parse and normalize)
    /// 4. Return HTTP response immediately
    /// 5. Spawn async task for handlers (fire-and-forget)
    ///
    /// # Arguments
    ///
    /// * `request` - The incoming webhook request
    ///
    /// # Returns
    ///
    /// HTTP response to send to GitHub
    ///
    /// # Errors
    ///
    /// Returns error responses for:
    /// - Missing required headers (BadRequest)
    /// - Invalid signature (Unauthorized)
    /// - Malformed payload (BadRequest)
    /// - Processing failures (InternalError)
    pub async fn receive_webhook(&self, request: WebhookRequest) -> WebhookResponse {
        // Extract required headers
        let event_type = match request.event_type() {
            Some(et) => et,
            None => {
                return WebhookResponse::BadRequest {
                    message: "Missing X-GitHub-Event header".to_string(),
                };
            }
        };

        let signature = match request.signature() {
            Some(sig) => sig,
            None => {
                return WebhookResponse::Unauthorized {
                    message: "Missing X-Hub-Signature-256 header".to_string(),
                };
            }
        };

        let delivery_id = request.delivery_id();

        // Validate signature
        match self.validator.validate(request.payload(), signature).await {
            Ok(true) => {
                info!(
                    event_type = %event_type,
                    delivery_id = ?delivery_id,
                    "Webhook signature validated"
                );
            }
            Ok(false) => {
                warn!(
                    event_type = %event_type,
                    delivery_id = ?delivery_id,
                    "Invalid webhook signature"
                );
                return WebhookResponse::Unauthorized {
                    message: "Invalid signature".to_string(),
                };
            }
            Err(e) => {
                error!(
                    event_type = %event_type,
                    delivery_id = ?delivery_id,
                    error = %e,
                    "Signature validation failed"
                );
                return WebhookResponse::InternalError {
                    message: format!("Validation error: {}", e),
                };
            }
        }

        // Process event (parse and normalize)
        let envelope = match self
            .processor
            .process_webhook(event_type, request.payload(), delivery_id)
            .await
        {
            Ok(env) => env,
            Err(e) => {
                error!(
                    event_type = %event_type,
                    delivery_id = ?delivery_id,
                    error = %e,
                    "Event processing failed"
                );
                return WebhookResponse::BadRequest {
                    message: format!("Invalid webhook payload: {}", e),
                };
            }
        };

        let event_id = envelope.event_id.to_string();

        info!(
            event_id = %envelope.event_id,
            event_type = %envelope.event_type,
            repository = %envelope.repository.full_name,
            "Webhook processed successfully"
        );

        // Spawn async handler tasks (fire-and-forget)
        let handlers = self.handlers.clone();
        tokio::spawn(async move {
            let handlers_guard = handlers.read().await;
            for handler in handlers_guard.iter() {
                let handler_clone = handler.clone();
                let envelope_clone = envelope.clone();

                tokio::spawn(async move {
                    if let Err(e) = handler_clone.handle_event(&envelope_clone).await {
                        error!(
                            event_id = %envelope_clone.event_id,
                            error = %e,
                            "Handler execution failed"
                        );
                    }
                });
            }
        });

        // Return immediate response
        WebhookResponse::Ok {
            message: "Webhook received".to_string(),
            event_id,
        }
    }
}

#[cfg(test)]
#[path = "receiver_tests.rs"]
mod tests;
