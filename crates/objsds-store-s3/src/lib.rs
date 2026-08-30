//! Blocking S3-compatible storage adapter for `objsds`.
//!
//! Every object-store operation performs synchronous network I/O and may block
//! the calling thread. Reads and writes transfer complete objects. Conditional
//! failures may trigger an additional `GET` to report the observed version.

#![deny(missing_docs)]

mod builder;
mod config;
mod credentials;
mod error;
mod store;

pub use builder::S3StoreBuilder;
pub use config::S3Config;
pub use credentials::Credentials;
pub use error::{BuildError, StoreError};
pub use store::S3Store;
