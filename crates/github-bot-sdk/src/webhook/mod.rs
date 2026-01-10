//! GitHub webhook processing and validation.
//!
//! This module provides webhook signature validation and event processing
//! for GitHub App webhooks.
//!
//! # Security
//!
//! Webhook signature validation uses HMAC-SHA256 with constant-time comparison
//! to prevent timing attacks. All validation operations complete in under 100ms.
//!
//! # Examples
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
