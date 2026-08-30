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
use crate::{DocumentError, Error, StoreError};

/// A single-object UTF-8/JSON map.
///
/// Each call reads one complete coherent snapshot. Calls do not share a
/// snapshot and are not transactions. All methods are blocking and O(n) in the
/// total encoded map size; mutations conditionally rewrite the entire object.
pub struct Map<S, V> {
    pub(super) store: Arc<S>,
    pub(super) location: Location,
    pub(super) schema: String,
    pub(super) value: PhantomData<fn() -> V>,
}

impl<S: ObjectStore, V: Serialize + DeserializeOwned> Map<S, V> {
    /// Returns the value for `key` from one coherent map snapshot.
    ///
    /// Performs one complete object read and O(n) decoding, despite selecting
    /// only one key.
    pub fn get(&self, key: &str) -> Result<Option<V>, Error<S::Error>> {
        Ok(self.read()?.1.entries.remove(key))
    }

    /// Returns all entries from one coherent snapshot in key order.
    ///
    /// Performs one complete object read and O(n) decoding and allocation.
    pub fn entries(&self) -> Result<Vec<(String, V)>, Error<S::Error>> {
        Ok(self.read()?.1.entries.into_iter().collect())
    }

    /// Inserts or replaces an entry and returns the new object version.
    ///
    /// Performs one complete read followed by one conditional complete-object
    /// replacement. Encoding, memory, and transfer costs are O(n). A concurrent
    /// mutation returns [`Error::Conflict`] and is not retried.
    pub fn insert(&self, key: impl Into<String>, value: V) -> Result<Version, Error<S::Error>> {
        let (version, mut document) = self.read()?;
        document.entries.insert(key.into(), value);
        self.write(&version, &document)
    }

    /// Inserts only when `key` is absent from the snapshot read by this call.
    ///
    /// An absent key causes one read and one conditional complete-object write;
    /// a present key causes only the read. The operation is O(n). A concurrent
    /// mutation can return [`Error::Conflict`] rather than [`InsertIfAbsent`].
    pub fn insert_if_absent(
        &self,
        key: impl Into<String>,
        value: V,
    ) -> Result<InsertIfAbsent<V>, Error<S::Error>> {
        let key = key.into();
        let (version, mut document) = self.read()?;
        if let Some(value) = document.entries.remove(&key) {
            return Ok(InsertIfAbsent::Occupied(value));
        }
        document.entries.insert(key, value);
        self.write(&version, &document)
            .map(InsertIfAbsent::Inserted)
    }

    /// Removes and returns a value when present in this call's snapshot.
    ///
    /// Always performs one complete read. A present key also causes one
    /// conditional complete-object replacement. The operation is O(n); a
    /// concurrent mutation returns [`Error::Conflict`] without retrying.
    pub fn remove(&self, key: &str) -> Result<Option<V>, Error<S::Error>> {
        let (version, mut document) = self.read()?;
        let removed = document.entries.remove(key);
        if removed.is_some() {
            self.write(&version, &document)?;
        }
        Ok(removed)
    }

    pub(super) fn empty_bytes(&self) -> Result<Vec<u8>, Error<S::Error>> {
        serde_json::to_vec(&MapDocument::<V> {
            format_version: FORMAT_VERSION,
            kind: "map",
            schema: &self.schema,
            entries: BTreeMap::new(),
        })
        .map_err(DocumentError::Serialize)
        .map_err(Error::Document)
    }

    pub(super) fn read(&self) -> Result<(Version, OwnedMapDocument<V>), Error<S::Error>> {
        let object = self
            .store
            .get(&self.location)
            .map_err(|source| Error::Store(StoreError { source }))?
            .ok_or(Error::NotFound)?;
        let document: OwnedMapDocument<V> = serde_json::from_slice(&object.bytes)
            .map_err(DocumentError::Deserialize)
            .map_err(Error::Document)?;
        validate(
            document.format_version,
            &document.kind,
            &document.schema,
            "map",
            &self.schema,
        )?;
        Ok((object.version, document))
    }

    fn write(
        &self,
        version: &Version,
        document: &OwnedMapDocument<V>,
    ) -> Result<Version, Error<S::Error>> {
        let bytes = serde_json::to_vec(document)
            .map_err(DocumentError::Serialize)
            .map_err(Error::Document)?;
        replace(self.store.as_ref(), &self.location, version, &bytes)
    }
}

/// Result of [`Map::insert_if_absent`].
#[derive(Debug, Eq, PartialEq)]
pub enum InsertIfAbsent<V> {
    /// The value was inserted; contains the resulting object version.
    Inserted(Version),
    /// The snapshot already contained the key; contains its stored value.
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
