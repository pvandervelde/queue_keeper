//! GitHub webhook processing and validation.
//!
//! This module provides comprehensive webhook handling for GitHub Apps, including
//! signature validation, event processing, and async handler execution using a
//! fire-and-forget pattern to ensure fast HTTP responses.
//!
//! # Core Components
//!
//! - [`WebhookHandler`] - Trait for application-provided event processing logic
//! - [`WebhookReceiver`] - HTTP webhook intake with validation and async dispatch
//! - [`SignatureValidator`] - HMAC-SHA256 signature validation
//! - [`WebhookRequest`]/[`WebhookResponse`] - HTTP request/response types
//!
//! # Fire-and-Forget Pattern
//!
//! The receiver ensures GitHub receives responses within the 10-second timeout:
//!
//! 1. Validate signature (< 10ms)
//! 2. Process/normalize event (< 5ms)
//! 3. Return HTTP response immediately (target < 100ms)
//! 4. Spawn async tasks for handlers (non-blocking)
//!
//! # Security
//!
//! Webhook signature validation uses HMAC-SHA256 with constant-time comparison
//! to prevent timing attacks. All validation operations complete in under 100ms.
//!
//! # Complete Usage Example
//!
//! ## Basic Webhook Handler
//!
//! ```rust,no_run
//! use github_bot_sdk::webhook::{WebhookHandler, WebhookReceiver, WebhookRequest};
//! use github_bot_sdk::auth::SecretProvider;
//! use github_bot_sdk::events::{EventProcessor, ProcessorConfig, EventEnvelope};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//! use std::collections::HashMap;
//!
//! // Define your handler
//! struct MyBotHandler;
//!
//! #[async_trait]
//! impl WebhookHandler for MyBotHandler {
//!     async fn handle_event(
//!         &self,
//!         envelope: &EventEnvelope,
//!     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!         match envelope.event_type.as_str() {
//!             "pull_request" => {
//!                 println!("Processing PR event: {}", envelope.event_id);
//!                 // Add your PR processing logic here
//!             }
//!             "issues" => {
//!                 println!("Processing issue event: {}", envelope.event_id);
//!                 // Add your issue processing logic here
//!             }
//!             _ => {
//!                 println!("Ignoring event type: {}", envelope.event_type);
//!             }
//!         }
//!         Ok(())
//!     }
//! }
//!
//! # async fn example(secret_provider: Arc<dyn SecretProvider>) -> Result<(), Box<dyn std::error::Error>> {
//! // Setup receiver with processor
//! let processor = EventProcessor::new(ProcessorConfig::default());
//! let mut receiver = WebhookReceiver::new(secret_provider, processor);
//!
//! // Register your handler
//! receiver.add_handler(Arc::new(MyBotHandler)).await;
//!
//! // Process incoming webhook (typically called from HTTP server)
//! let headers = HashMap::from([
//!     ("x-github-event".to_string(), "pull_request".to_string()),
//!     ("x-github-delivery".to_string(), "12345-67890".to_string()),
//!     ("x-hub-signature-256".to_string(), "sha256=abc...".to_string()),
//! ]);
//! let body = bytes::Bytes::from_static(b"{\"action\":\"opened\",\"number\":1}");
//! let request = WebhookRequest::new(headers, body);
//!
//! let response = receiver.receive_webhook(request).await;
//! println!("Response status: {}", response.status_code());
//! # Ok(())
//! # }
//! ```
//!
//! ## Multiple Handlers
//!
//! ```rust,no_run
//! use github_bot_sdk::webhook::{WebhookHandler, WebhookReceiver};
//! use github_bot_sdk::events::EventEnvelope;
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! // Define specialized handlers
//! struct PullRequestHandler;
//! struct IssueHandler;
//! struct SecurityHandler;
//!
//! #[async_trait]
//! impl WebhookHandler for PullRequestHandler {
//!     async fn handle_event(&self, envelope: &EventEnvelope) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!         if envelope.event_type == "pull_request" {
//!             println!("PR Handler: Processing {}", envelope.event_id);
//!             // PR-specific logic
//!         }
//!         Ok(())
//!     }
//! }
//!
//! #[async_trait]
//! impl WebhookHandler for IssueHandler {
//!     async fn handle_event(&self, envelope: &EventEnvelope) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!         if envelope.event_type == "issues" {
//!             println!("Issue Handler: Processing {}", envelope.event_id);
//!             // Issue-specific logic
//!         }
//!         Ok(())
//!     }
//! }
//!
//! #[async_trait]
//! impl WebhookHandler for SecurityHandler {
//!     async fn handle_event(&self, envelope: &EventEnvelope) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!         // Security monitoring across all event types
//!         println!("Security: Auditing event {}", envelope.event_id);
//!         Ok(())
//!     }
//! }
//!
//! # async fn example(mut receiver: WebhookReceiver) -> Result<(), Box<dyn std::error::Error>> {
//! // Register multiple handlers - all will be invoked concurrently
//! receiver.add_handler(Arc::new(PullRequestHandler)).await;
//! receiver.add_handler(Arc::new(IssueHandler)).await;
//! receiver.add_handler(Arc::new(SecurityHandler)).await;
//! # Ok(())
//! # }
//! ```
//!
//! ## HTTP Server Integration (Axum Example)
//!
//! ```rust,ignore
//! use github_bot_sdk::webhook::{WebhookReceiver, WebhookRequest, WebhookResponse};
//! use axum::{
//!     extract::State,
//!     http::{HeaderMap, StatusCode},
//!     response::{IntoResponse, Response},
//!     Json, Router,
//!     routing::post,
//! };
//! use bytes::Bytes;
//! use std::sync::Arc;
//!
//! // Application state with receiver
//! #[derive(Clone)]
//! struct AppState {
//!     receiver: Arc<WebhookReceiver>,
//! }
//!
//! // HTTP handler for webhook endpoint
//! async fn handle_webhook(
//!     State(state): State<AppState>,
//!     headers: HeaderMap,
//!     body: Bytes,
//! ) -> Response {
//!     // Convert HTTP headers to HashMap
//!     let header_map: std::collections::HashMap<String, String> = headers
//!         .iter()
//!         .map(|(k, v)| (k.as_str().to_lowercase(), v.to_str().unwrap_or("").to_string()))
//!         .collect();
//!
//!     // Create webhook request
//!     let request = WebhookRequest::new(header_map, body);
//!
//!     // Process webhook
//!     let response = state.receiver.receive_webhook(request).await;
//!
//!     // Convert to HTTP response
//!     let status = StatusCode::from_u16(response.status_code()).unwrap();
//!     let message = response.message().to_string();
//!
//!     (status, Json(serde_json::json!({
//!         "message": message
//!     }))).into_response()
//! }
//!
//! # async fn example(receiver: Arc<WebhookReceiver>) -> Result<(), Box<dyn std::error::Error>> {
//! let state = AppState { receiver };
//!
//! let app = Router::new()
//!     .route("/webhook", post(handle_webhook))
//!     .with_state(state);
//!
//! // Run server
//! // let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
//! // axum::serve(listener, app).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Direct Signature Validation
//!
//! If you need to validate signatures independently:
//!
//! ```rust,no_run
//! use github_bot_sdk::webhook::SignatureValidator;
//! use github_bot_sdk::auth::SecretProvider;
//! use std::sync::Arc;
//!
//! # async fn example(secret_provider: Arc<dyn SecretProvider>) -> Result<(), Box<dyn std::error::Error>> {
//! let validator = SignatureValidator::new(secret_provider);
//!
//! let payload = b"{\"action\":\"opened\",\"number\":1}";
//! let signature = "sha256=5c4a...";  // From X-Hub-Signature-256 header
//!
//! let is_valid = validator.validate(payload, signature).await?;
//! if is_valid {
//!     println!("Webhook signature is valid");
//! } else {
//!     println!("Invalid webhook signature - possible tampering");
//! }
//! # Ok(())
//! # }
//! ```

pub mod handler;
pub mod receiver;
pub mod validation;

// Re-export main types
pub use handler::WebhookHandler;
pub use receiver::{WebhookReceiver, WebhookRequest, WebhookResponse};
pub use validation::SignatureValidator;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
