//! Queue provider implementations.
//!
//! This module contains concrete implementations of the `QueueProvider` and
//! `SessionProvider` traits for different queue backends.

pub mod memory;

pub use memory::{InMemoryProvider, InMemorySessionProvider};
