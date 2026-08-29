use std::marker::PhantomData;
use std::sync::Arc;

use objsds_store::{Location, ObjectStore};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::Map;
use crate::client::{BuildError, location};
use crate::lifecycle::create;
use crate::{Error, Result};

/// Builder for one UTF-8/JSON Map.
pub struct MapBuilder<S, V> {
    store: Arc<S>,
    location: std::result::Result<Location, BuildError>,
    schema: Option<String>,
    value: PhantomData<fn() -> V>,
}

impl<S, V> MapBuilder<S, V> {
    pub(crate) fn new(store: Arc<S>, namespace: &str, name: String) -> Self {
        Self {
            store,
            location: location(namespace, "maps", &name),
            schema: None,
            value: PhantomData,
        }
    }

    #[must_use]
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    fn finish(self) -> std::result::Result<Map<S, V>, BuildError> {
        let schema = self.schema.ok_or(BuildError::MissingSchema)?;
        if schema.is_empty() {
            return Err(BuildError::MissingSchema);
        }
        Ok(Map {
            store: self.store,
            location: self.location?,
            schema,
            value: PhantomData,
        })
    }
}

impl<S: ObjectStore, V: Serialize + DeserializeOwned> MapBuilder<S, V> {
    pub fn create(self) -> Result<Map<S, V>, S::Error> {
        let map = self.finish().map_err(config_error)?;
        let bytes = map.empty_bytes()?;
        create(map.store.as_ref(), &map.location, &bytes)?;
        Ok(map)
    }

    pub fn open(self) -> Result<Map<S, V>, S::Error> {
        let map = self.finish().map_err(config_error)?;
        map.read()?;
        Ok(map)
    }

    pub fn open_or_create(self) -> Result<Map<S, V>, S::Error> {
        let map = self.finish().map_err(config_error)?;
        if map
            .store
            .get(&map.location)
            .map_err(Error::Store)?
            .is_some()
        {
            map.read()?;
            return Ok(map);
        }

        let bytes = map.empty_bytes()?;
        match create(map.store.as_ref(), &map.location, &bytes) {
            Ok(_) => Ok(map),
            Err(Error::AlreadyExists { .. }) => {
                map.read()?;
                Ok(map)
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
