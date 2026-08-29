//! Blocking, single-object distributed data structures.

mod client;
mod document;
mod error;
mod lifecycle;
mod log;
mod map;

pub use client::{BuildError, Objsds, ObjsdsBuilder};
pub use error::{Error, Result};
pub use log::{Log, LogBuilder, LogId, Record};
pub use map::{InsertIfAbsent, Map, MapBuilder};
pub use objsds_store::Version;
