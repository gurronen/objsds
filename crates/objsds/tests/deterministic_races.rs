use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use objsds::{Error, Objsds};
use objsds_store::{CreateError, Location, Object, ObjectStore, ReplaceError, Version};
use objsds_store_memory::MemoryStore;
use serde_json::json;

#[derive(Clone, Debug)]
struct FaultStore {
    inner: MemoryStore,
    fault: Arc<Mutex<Option<Fault>>>,
}

#[derive(Debug)]
enum Fault {
    WinCreate(Vec<u8>),
    WinReplace(Vec<u8>),
}

impl FaultStore {
    fn winning_create(bytes: Vec<u8>) -> Self {
        Self {
            inner: MemoryStore::default(),
            fault: Arc::new(Mutex::new(Some(Fault::WinCreate(bytes)))),
        }
    }

    fn winning_replace(bytes: Vec<u8>) -> Self {
        Self {
            inner: MemoryStore::default(),
            fault: Arc::new(Mutex::new(Some(Fault::WinReplace(bytes)))),
        }
    }

    fn take_fault(&self) -> Option<Fault> {
        self.fault
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }
}

impl ObjectStore for FaultStore {
    type Error = Infallible;

    fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error> {
        self.inner.get(location)
    }

    fn create(
        &self,
        location: &Location,
        bytes: &[u8],
    ) -> Result<Version, CreateError<Self::Error>> {
        match self.take_fault() {
            Some(Fault::WinCreate(winner)) => {
                self.inner
                    .create(location, &winner)
                    .expect("injected concurrent create should win");
                self.inner.create(location, bytes)
            }
            fault => {
                if let Some(fault) = fault {
                    *self.fault.lock().unwrap_or_else(|error| error.into_inner()) = Some(fault);
                }
                self.inner.create(location, bytes)
            }
        }
    }

    fn replace(
        &self,
        location: &Location,
        expected: &Version,
        bytes: &[u8],
    ) -> Result<Version, ReplaceError<Self::Error>> {
        match self.take_fault() {
            Some(Fault::WinReplace(winner)) => {
                self.inner
                    .replace(location, expected, &winner)
                    .expect("injected concurrent replacement should win");
                self.inner.replace(location, expected, bytes)
            }
            fault => {
                if let Some(fault) = fault {
                    *self.fault.lock().unwrap_or_else(|error| error.into_inner()) = Some(fault);
                }
                self.inner.replace(location, expected, bytes)
            }
        }
    }
}

fn map_bytes(schema: &str, entries: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "format_version": 1,
        "kind": "map",
        "schema": schema,
        "entries": entries,
    }))
    .expect("test document should serialize")
}

#[test]
fn open_or_create_reads_the_object_that_won_the_create_race() {
    let store = FaultStore::winning_create(map_bytes("user-v1", json!({ "winner": 7 })));
    let client = Objsds::builder()
        .store(store)
        .namespace("race")
        .build()
        .expect("client configuration should be valid");

    let map = client
        .map::<u64>("users")
        .schema("user-v1")
        .open_or_create()
        .expect("the compatible concurrent winner should be opened");

    assert_eq!(
        map.get("winner").expect("winner should be readable"),
        Some(7)
    );
}

#[test]
fn open_or_create_rejects_an_incompatible_create_race_winner() {
    let store = FaultStore::winning_create(map_bytes("user-v2", json!({})));
    let client = Objsds::builder()
        .store(store)
        .namespace("race")
        .build()
        .expect("client configuration should be valid");

    let result = client
        .map::<u64>("users")
        .schema("user-v1")
        .open_or_create();

    assert!(matches!(result, Err(Error::Incompatible(_))));
}

#[test]
fn stale_concurrent_write_reports_conflict_and_preserves_the_winner() {
    let store = FaultStore::winning_replace(map_bytes("user-v1", json!({ "winner": 7 })));
    let client = Objsds::builder()
        .store(store)
        .namespace("race")
        .build()
        .expect("client configuration should be valid");
    let map = client
        .map::<u64>("users")
        .schema("user-v1")
        .open_or_create()
        .expect("map should be created");

    let result = map.insert("stale", 9);

    assert!(matches!(
        result,
        Err(Error::Conflict(objsds::ConflictError {
            observed: Some(_),
            ..
        }))
    ));
    assert_eq!(
        map.entries().expect("winning state should be readable"),
        [("winner".to_owned(), 7)]
    );
}
