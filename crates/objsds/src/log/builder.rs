use std::marker::PhantomData;
use std::sync::Arc;

use objsds_store::{Location, ObjectStore};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::Log;
use crate::client::{BuildError, location};
use crate::lifecycle::create;
use crate::{Error, Result};

/// Builder for one append-only JSON Log.
pub struct LogBuilder<S, V> {
    store: Arc<S>,
    location: std::result::Result<Location, BuildError>,
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

    #[must_use]
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    fn finish(self) -> std::result::Result<Log<S, V>, BuildError> {
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
    pub fn create(self) -> Result<Log<S, V>, S::Error> {
        let log = self.finish().map_err(config_error)?;
        let bytes = log.empty_bytes()?;
        create(log.store.as_ref(), &log.location, &bytes)?;
        Ok(log)
    }

    pub fn open(self) -> Result<Log<S, V>, S::Error> {
        let log = self.finish().map_err(config_error)?;
        log.read()?;
        Ok(log)
    }

    pub fn open_or_create(self) -> Result<Log<S, V>, S::Error> {
        let log = self.finish().map_err(config_error)?;
        if log
            .store
            .get(&log.location)
            .map_err(Error::Store)?
            .is_some()
        {
            log.read()?;
            return Ok(log);
        }

        let bytes = log.empty_bytes()?;
        match create(log.store.as_ref(), &log.location, &bytes) {
            Ok(_) => Ok(log),
            Err(Error::AlreadyExists { .. }) => {
                log.read()?;
                Ok(log)
            }
            Err(error) => Err(error),
        }
    }
}

fn config_error<E>(error: BuildError) -> Error<E> {
    Error::Incompatible {
        expected: "valid structure configuration".to_owned(),
        observed: error.to_string(),
    }
}
