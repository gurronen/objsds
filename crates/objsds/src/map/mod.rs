mod builder;

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;

pub use builder::MapBuilder;
use objsds_store::{Location, ObjectStore, Version};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::document::{FORMAT_VERSION, validate};
use crate::lifecycle::replace;
use crate::{Error, Result};

/// A single-object UTF-8/JSON Map.
pub struct Map<S, V> {
    pub(super) store: Arc<S>,
    pub(super) location: Location,
    pub(super) schema: String,
    pub(super) value: PhantomData<fn() -> V>,
}

impl<S: ObjectStore, V: Serialize + DeserializeOwned> Map<S, V> {
    pub fn get(&self, key: &str) -> Result<Option<V>, S::Error> {
        Ok(self.read()?.1.entries.remove(key))
    }

    pub fn entries(&self) -> Result<Vec<(String, V)>, S::Error> {
        Ok(self.read()?.1.entries.into_iter().collect())
    }

    pub fn insert(&self, key: impl Into<String>, value: V) -> Result<Version, S::Error> {
        let (version, mut document) = self.read()?;
        document.entries.insert(key.into(), value);
        self.write(&version, &document)
    }

    pub fn insert_if_absent(
        &self,
        key: impl Into<String>,
        value: V,
    ) -> Result<InsertIfAbsent<V>, S::Error> {
        let key = key.into();
        let (version, mut document) = self.read()?;
        if let Some(value) = document.entries.remove(&key) {
            return Ok(InsertIfAbsent::Occupied(value));
        }
        document.entries.insert(key, value);
        self.write(&version, &document)
            .map(InsertIfAbsent::Inserted)
    }

    pub fn remove(&self, key: &str) -> Result<Option<V>, S::Error> {
        let (version, mut document) = self.read()?;
        let removed = document.entries.remove(key);
        if removed.is_some() {
            self.write(&version, &document)?;
        }
        Ok(removed)
    }

    pub(super) fn empty_bytes(&self) -> Result<Vec<u8>, S::Error> {
        serde_json::to_vec(&MapDocument::<V> {
            format_version: FORMAT_VERSION,
            kind: "map",
            schema: &self.schema,
            entries: BTreeMap::new(),
        })
        .map_err(Error::Json)
    }

    pub(super) fn read(&self) -> Result<(Version, OwnedMapDocument<V>), S::Error> {
        let object = self
            .store
            .get(&self.location)
            .map_err(Error::Store)?
            .ok_or(Error::NotFound)?;
        validate(&object.bytes, "map", &self.schema)?;
        let document = serde_json::from_slice(&object.bytes)?;
        Ok((object.version, document))
    }

    fn write(
        &self,
        version: &Version,
        document: &OwnedMapDocument<V>,
    ) -> Result<Version, S::Error> {
        let bytes = serde_json::to_vec(document)?;
        replace(self.store.as_ref(), &self.location, version, &bytes)
    }
}

/// Result of [`Map::insert_if_absent`].
#[derive(Debug, Eq, PartialEq)]
pub enum InsertIfAbsent<V> {
    Inserted(Version),
    Occupied(V),
}

#[derive(Serialize)]
struct MapDocument<'a, V> {
    format_version: u32,
    kind: &'static str,
    schema: &'a str,
    entries: BTreeMap<String, V>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct OwnedMapDocument<V> {
    format_version: u32,
    kind: String,
    schema: String,
    entries: BTreeMap<String, V>,
}
