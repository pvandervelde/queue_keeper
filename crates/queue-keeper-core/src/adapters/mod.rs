//! # Storage Adapters
//!
//! Infrastructure implementations of blob storage interface.

pub mod filesystem_storage;

pub use filesystem_storage::FilesystemBlobStorage;
