use std::marker::PhantomData;
use std::sync::Arc;

use objsds_store::{Location, ObjectStore};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::Log;
use crate::Error;
use crate::client::{BuildError, location};
use crate::lifecycle::{create_structure, open_or_create_structure, open_structure};

/// Builder for one append-only JSON Log.
pub struct LogBuilder<S, V> {
    store: Arc<S>,
    location: Result<Location, BuildError>,
    schema: Option<String>,
    value: PhantomData<fn() -> V>,
}

impl<S, V> LogBuilder<S, V> {
    pub(crate) fn new(store: Arc<S>, namespace: &str, name: String) -> Self {
        Self {
            store,
            location: location(namespace, "logs", &name),
            schema: None,
            value: PhantomData,
        }
    }

    /// Sets the stable application-defined schema identifier.
    ///
    /// Opening fails if stored metadata has a different identifier. Use an
    /// explicit versioned value rather than a Rust type name.
    #[must_use]
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    fn finish(self) -> Result<Log<S, V>, BuildError> {
        let schema = self.schema.ok_or(BuildError::MissingSchema)?;
        if schema.is_empty() {
            return Err(BuildError::MissingSchema);
        }
        Ok(Log {
            store: self.store,
            location: self.location?,
            schema,
            value: PhantomData,
        })
    }
}

impl<S: ObjectStore, V: Serialize + DeserializeOwned> LogBuilder<S, V> {
    /// Conditionally creates an empty log.
    ///
    /// Performs one conditional object-store create and returns
    /// [`Error::AlreadyExists`] if the location is occupied.
    pub fn create(self) -> Result<Log<S, V>, Error<S::Error>> {
        let log = self.finish().map_err(config_error)?;
        create_structure(log.store.as_ref(), &log.location, || log.empty_bytes())?;
        Ok(log)
    }

    /// Opens and validates an existing log snapshot.
    ///
    /// Performs one complete object-store read and O(n) JSON decoding.
    pub fn open(self) -> Result<Log<S, V>, Error<S::Error>> {
        let log = self.finish().map_err(config_error)?;
        open_structure::<S, _, _>(|| log.read())?;
        Ok(log)
    }

    /// Opens a valid log or atomically creates it when absent.
    ///
    /// Performs one read when present. When absent it performs a read and a
    /// conditional create; if another creator wins, it performs another read.
    pub fn open_or_create(self) -> Result<Log<S, V>, Error<S::Error>> {
        let log = self.finish().map_err(config_error)?;
        open_or_create_structure(
            log.store.as_ref(),
            &log.location,
            || log.empty_bytes(),
            || log.read(),
        )?;
        Ok(log)
    }
}

fn config_error<E>(error: BuildError) -> Error<E> {
    Error::Configuration(error)
}
