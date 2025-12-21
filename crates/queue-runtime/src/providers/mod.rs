//! Queue provider implementations.
//!
//! This module contains concrete implementations of the `QueueProvider` and
//! `SessionProvider` traits for different queue backends.

pub mod azure;
pub mod memory;

pub use azure::{AzureAuthMethod, AzureServiceBusProvider, AzureSessionProvider};
pub use memory::{InMemoryProvider, InMemorySessionProvider};
