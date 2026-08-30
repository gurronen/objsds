//! Blocking, single-object distributed data structures.
//!
//! Each [`Map`] or [`Log`] is stored as one JSON object. Every public operation
//! is synchronous and may block the calling thread on object-store I/O. Reads
//! download and decode the complete structure. Mutations additionally encode
//! and conditionally replace the complete structure, so their CPU, memory, and
//! transfer costs are **O(n)** in the total structure size, not merely the item
//! being accessed. Applications must bound structure growth accordingly.
//!
//! A read operation observes one coherent object version (a snapshot), but a
//! sequence of calls is not a transaction. Mutations use compare-and-swap. A
//! concurrent writer can therefore produce [`Error::Conflict`]; this crate does
//! not retry automatically because retry policy, backoff, and idempotency belong
//! to the caller.
//!
//! # Bounded conflict retries
//!
//! A retry must re-run the complete operation so it reads a fresh snapshot.
//! Keep attempts bounded and inspect the final conflict:
//!
//! ```
//! use objsds::{Error, Map, Version};
//! use objsds_store::ObjectStore;
//! use serde::{Serialize, de::DeserializeOwned};
//!
//! fn insert_with_limit<S, V>(
//!     map: &Map<S, V>,
//!     key: &str,
//!     value: V,
//!     max_attempts: usize,
//! ) -> Result<Version, Error<S::Error>>
//! where
//!     S: ObjectStore,
//!     V: Clone + Serialize + DeserializeOwned,
//! {
//!     assert!(max_attempts > 0);
//!     let mut last_conflict = None;
//!     for _ in 0..max_attempts {
//!         match map.insert(key, value.clone()) {
//!             Ok(version) => return Ok(version),
//!             Err(Error::Conflict(conflict)) => last_conflict = Some(conflict),
//!             Err(error) => return Err(error),
//!         }
//!         // Apply bounded backoff or jitter here if appropriate.
//!     }
//!     Err(Error::Conflict(
//!         last_conflict.expect("at least one attempt was made"),
//!     ))
//! }
//! # let _ = insert_with_limit::<objsds_store_memory::MemoryStore, String>;
//! ```
//!
//! `observed: Some(version)` means another object version was present when the
//! failed condition was inspected; `None` means the object was absent then.
//! Either observation can already be stale, so use it for diagnostics rather
//! than as a new expected version.

#![deny(missing_docs)]

mod client;
mod document;
mod error;
mod lifecycle;
mod log;
mod map;

pub use client::{BuildError, Objsds, ObjsdsBuilder};
pub use error::{
    AlreadyExistsError, CompatibilityError, ConflictError, DocumentError, Error, StoreError,
};
pub use log::{Log, LogBuilder, LogId, Record};
pub use map::{InsertIfAbsent, Map, MapBuilder};
pub use objsds_store::Version;
