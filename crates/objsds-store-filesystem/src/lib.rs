//! Persistent blocking object store backed by a local filesystem.
//!
//! The adapter is designed for filesystems that provide reliable advisory
//! locks and atomic rename within one directory. All access to managed objects
//! must go through this adapter to preserve conditional-write semantics.

#![deny(missing_docs)]

mod error;
#[allow(dead_code)]
mod format;
#[allow(dead_code)]
mod lock;
#[allow(dead_code)]
mod path;
mod store;

pub use error::{BuildError, StoreError};
pub use store::{FilesystemStore, FilesystemStoreBuilder};
