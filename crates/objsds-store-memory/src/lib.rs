//! Deterministic in-memory object store for tests and local use.
//!
//! Operations are blocking and serialized by an in-process mutex. Clones share
//! the same state. Data is neither durable nor shared between processes.

#![deny(missing_docs)]

mod state;
mod store;

pub use store::MemoryStore;
