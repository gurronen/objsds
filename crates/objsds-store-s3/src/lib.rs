//! Blocking S3-compatible storage adapter for `objsds`.
//!
//! Configuration is implemented while transport selection remains pending.

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
