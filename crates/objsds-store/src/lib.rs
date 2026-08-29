//! Minimal blocking capabilities required by single-object data structures.

mod error;
mod location;
mod object;
mod store;
mod version;

pub use error::{CreateError, ReplaceError};
pub use location::{InvalidLocation, Location};
pub use object::Object;
pub use store::ObjectStore;
pub use version::Version;
