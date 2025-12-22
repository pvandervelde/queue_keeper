//! # Infrastructure Adapters
//!
//! Infrastructure implementations of blob storage and key vault interfaces.

pub mod circuit_breaker_key_vault;
pub mod filesystem_storage;
pub mod memory_key_vault;

#[cfg(feature = "azure")]
pub mod azure_key_vault;

pub use circuit_breaker_key_vault::CircuitBreakerKeyVaultProvider;
pub use filesystem_storage::FilesystemBlobStorage;
pub use memory_key_vault::{InMemoryKeyVaultProvider, InMemorySecretCache};

#[cfg(feature = "azure")]
pub use azure_key_vault::AzureKeyVaultProvider;
