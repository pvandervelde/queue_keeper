//! Webhook handler trait and types for application-provided processing logic.
//!
//! This module defines the interface that applications implement to process
//! GitHub webhook events. Handlers receive normalized EventEnvelope instances
//! and can perform async processing without blocking webhook HTTP responses.
//!
//! # Fire-and-Forget Pattern
//!
//! Handlers execute asynchronously after the HTTP response is sent to GitHub.
//! This ensures GitHub receives a response within the 10-second timeout while
//! allowing handlers to perform longer-running operations.
//!
//! # Examples
//!
//! ```rust,no_run
//! use github_bot_sdk::webhook::WebhookHandler;
//! use github_bot_sdk::events::EventEnvelope;
//! use async_trait::async_trait;
//!
//! struct MyHandler;
//!
//! #[async_trait]
//! impl WebhookHandler for MyHandler {
//!     async fn handle_event(&self, envelope: &EventEnvelope) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!         println!("Processing event: {}", envelope.event_id);
//!         // Perform async processing here
//!         Ok(())
//!     }
//! }
//! ```

use crate::events::EventEnvelope;
use async_trait::async_trait;
use std::error::Error;

/// Application-provided webhook event handler.
///
/// Implementations of this trait define custom processing logic for GitHub
/// webhook events. Handlers are invoked asynchronously after the HTTP response
/// is sent, allowing long-running operations without blocking GitHub's webhook
/// delivery.
///
/// # Error Handling
///
/// Handler errors are logged but do not affect the HTTP response to GitHub.
/// Failed handler executions should implement their own retry/recovery logic
/// if needed.
///
/// # Concurrency
///
/// Multiple handlers can be registered and will execute concurrently for each
/// webhook event. Handlers must be `Send + Sync` to support concurrent execution.
///
/// # Examples
///
/// ```rust,no_run
/// use github_bot_sdk::webhook::WebhookHandler;
/// use github_bot_sdk::events::EventEnvelope;
/// use async_trait::async_trait;
///
/// struct PullRequestHandler;
///
/// #[async_trait]
/// impl WebhookHandler for PullRequestHandler {
///     async fn handle_event(&self, envelope: &EventEnvelope) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///         if envelope.event_type == "pull_request" {
///             println!("Processing PR event for {}", envelope.repository.full_name);
///             // Add PR processing logic
///         }
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait WebhookHandler: Send + Sync {
    /// Handle a webhook event asynchronously.
    ///
    /// This method is called after the HTTP response has been sent to GitHub.
    /// It should process the event and return a result indicating success or failure.
    ///
    /// # Arguments
    ///
    /// * `envelope` - The normalized event envelope containing event data and metadata
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Event processed successfully
    /// * `Err(e)` - Event processing failed with error details
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use github_bot_sdk::webhook::WebhookHandler;
    /// # use github_bot_sdk::events::EventEnvelope;
    /// # use async_trait::async_trait;
    /// # struct MyHandler;
    /// # #[async_trait]
    /// # impl WebhookHandler for MyHandler {
    /// async fn handle_event(&self, envelope: &EventEnvelope) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ///     match envelope.event_type.as_str() {
    ///         "pull_request" => {
    ///             // Handle PR events
    ///             println!("PR event: {}", envelope.event_id);
    ///         }
    ///         "issues" => {
    ///             // Handle issue events
    ///             println!("Issue event: {}", envelope.event_id);
    ///         }
    ///         _ => {
    ///             // Ignore other event types
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// # }
    /// ```
    async fn handle_event(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[cfg(test)]
#[path = "handler_tests.rs"]
mod tests;
