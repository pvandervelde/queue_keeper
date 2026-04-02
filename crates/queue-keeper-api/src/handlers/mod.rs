//! HTTP handler modules for queue-keeper-api.
//!
//! Handlers are split by functional area:
//! - [`health`] — liveness, readiness, and health-check endpoints
//! - [`webhook`] — provider webhook ingestion endpoint

pub mod health;
pub mod webhook;
