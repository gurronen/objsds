//! Minimal blocking capabilities required by single-object data structures.
//!
//! Every operation in this crate is synchronous and may block the calling
//! thread. Network-backed implementations should be called from an appropriate
//! blocking thread when used by an asynchronous application.
//!
//! An [`ObjectStore`] exposes only whole-object reads, conditional creates, and
//! compare-and-swap replacements. Versions are opaque equality tokens: callers
//! must not interpret or order them.

#![deny(missing_docs)]

mod error;
mod location;
mod object;
mod store;
mod version;

pub use error::{CreateError, ReplaceError};
pub use location::{InvalidLocation, Location, is_path_segment};
pub use object::Object;
pub use store::ObjectStore;
pub use version::Version;
