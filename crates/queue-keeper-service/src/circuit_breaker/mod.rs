//! Circuit breaker wrappers for external service clients.
//!
//! This module provides circuit breaker protection for github-bot-sdk and
//! queue-runtime clients at the service layer.

pub mod github;
pub mod queue;

pub use github::CircuitBreakerGitHubClient;
pub use queue::CircuitBreakerQueueProvider;
