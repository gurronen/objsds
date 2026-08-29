mod builder;
mod record;

use std::marker::PhantomData;
use std::sync::Arc;

pub use builder::LogBuilder;
use objsds_store::{Location, ObjectStore, Version};
pub use record::{LogId, Record};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::document::{FORMAT_VERSION, validate};
use crate::lifecycle::replace;
use crate::{DocumentError, Error, StoreError};

/// A single-object append-only JSON Log.
pub struct Log<S, V> {
    pub(super) store: Arc<S>,
    pub(super) location: Location,
    pub(super) schema: String,
    pub(super) value: PhantomData<fn() -> V>,
}

impl<S: ObjectStore, V: Serialize + DeserializeOwned> Log<S, V> {
    pub fn append(&self, value: V) -> Result<LogId, Error<S::Error>> {
        let (version, mut document) = self.read()?;
        let id = LogId::now();
        let index = document
            .records
            .binary_search_by_key(&id, |record| record.id)
            .unwrap_or_else(|index| index);
        document.records.insert(index, Record { id, value });
        self.write(&version, &document)?;
        Ok(id)
    }

    pub fn get(&self, id: LogId) -> Result<Option<Record<V>>, Error<S::Error>> {
        let (_, mut document) = self.read()?;
        match document
            .records
            .binary_search_by_key(&id, |record| record.id)
        {
            Ok(index) => Ok(Some(document.records.remove(index))),
            Err(_) => Ok(None),
        }
    }

    pub fn records(&self) -> Result<Vec<Record<V>>, Error<S::Error>> {
        Ok(self.read()?.1.records)
    }

    pub fn records_after(&self, id: LogId) -> Result<Vec<Record<V>>, Error<S::Error>> {
        let (_, document) = self.read()?;
        let index = document.records.partition_point(|record| record.id <= id);
        Ok(document.records.into_iter().skip(index).collect())
    }

    pub(super) fn empty_bytes(&self) -> Result<Vec<u8>, Error<S::Error>> {
        serde_json::to_vec(&LogDocument::<V> {
            format_version: FORMAT_VERSION,
            kind: "log".to_owned(),
            schema: self.schema.clone(),
            records: Vec::new(),
        })
        .map_err(DocumentError::Serialize)
        .map_err(Error::Document)
    }

    pub(super) fn read(&self) -> Result<(Version, LogDocument<V>), Error<S::Error>> {
        let object = self
            .store
            .get(&self.location)
            .map_err(|source| Error::Store(StoreError { source }))?
            .ok_or(Error::NotFound)?;
        let document: LogDocument<V> = serde_json::from_slice(&object.bytes)
            .map_err(DocumentError::Deserialize)
            .map_err(Error::Document)?;
        validate(
            document.format_version,
            &document.kind,
            &document.schema,
            "log",
            &self.schema,
        )?;
        if document
            .records
            .windows(2)
            .any(|records| records[0].id >= records[1].id)
        {
            return Err(Error::Document(DocumentError::Corrupt {
                reason: "log record IDs must be strictly increasing".to_owned(),
            }));
        }
        Ok((object.version, document))
    }

    fn write(
        &self,
        version: &Version,
        document: &LogDocument<V>,
    ) -> Result<Version, Error<S::Error>> {
        let bytes = serde_json::to_vec(document)
            .map_err(DocumentError::Serialize)
            .map_err(Error::Document)?;
        replace(self.store.as_ref(), &self.location, version, &bytes)
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct LogDocument<V> {
    format_version: u32,
    kind: String,
    schema: String,
    records: Vec<Record<V>>,
}
