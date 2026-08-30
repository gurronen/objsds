use objsds::{BuildError, CompatibilityError, DocumentError, Error, InsertIfAbsent, Objsds};
use objsds_store::{Location, ObjectStore};
use objsds_store_memory::MemoryStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct User {
    name: String,
}

#[test]
fn map_lifecycle_and_operations() {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace("test")
        .build()
        .expect("client configuration should be valid");
    let users = client
        .map::<User>("users")
        .schema("user-v1")
        .create()
        .expect("users map creation should succeed");

    users
        .insert(
            "bob",
            User {
                name: "Bob".to_owned(),
            },
        )
        .expect("Bob insertion should succeed");
    users
        .insert(
            "alice",
            User {
                name: "Alice".to_owned(),
            },
        )
        .expect("Alice insertion should succeed");

    assert_eq!(
        users.get("alice").expect("user read should succeed"),
        Some(User {
            name: "Alice".to_owned()
        })
    );
    assert_eq!(
        users
            .entries()
            .expect("entry snapshot should succeed")
            .into_iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>(),
        ["alice", "bob"]
    );
    assert!(matches!(
        users
            .insert_if_absent(
                "alice",
                User {
                    name: "Other".to_owned()
                }
            )
            .expect("conditional insertion should succeed"),
        InsertIfAbsent::Occupied(User { name }) if name == "Alice"
    ));
}

#[test]
fn log_is_sorted_and_append_only() {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace("test")
        .build()
        .expect("client configuration should be valid");
    let log = client
        .log::<String>("events")
        .schema("event-v1")
        .open_or_create()
        .expect("events log should open");

    let first = log
        .append("first".to_owned())
        .expect("first append should succeed");
    let second = log
        .append("second".to_owned())
        .expect("second append should succeed");

    assert!(first < second);
    assert_eq!(
        log.get(first)
            .expect("record read should succeed")
            .expect("first record should exist")
            .value,
        "first"
    );
    assert_eq!(
        log.records_after(first)
            .expect("record snapshot should succeed")
            .into_iter()
            .map(|record| record.value)
            .collect::<Vec<_>>(),
        ["second"]
    );
}

#[test]
fn opening_with_an_incompatible_schema_fails() {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace("test")
        .build()
        .expect("client configuration should be valid");
    client
        .map::<User>("users")
        .schema("user-v1")
        .create()
        .expect("users map creation should succeed");

    let result = client.map::<User>("users").schema("user-v2").open();
    assert!(matches!(
        result,
        Err(Error::Incompatible(CompatibilityError::Schema {
            expected,
            observed,
        })) if expected == "user-v2" && observed == "user-v1"
    ));
}

#[test]
fn map_rejects_a_malformed_typed_document() {
    let store = MemoryStore::default();
    let client = Objsds::builder()
        .store(store.clone())
        .namespace("test")
        .build()
        .expect("client configuration should be valid");
    client
        .map::<User>("users")
        .schema("user-v1")
        .create()
        .expect("users map creation should succeed");
    replace_document(
        &store,
        "test/maps/users.json",
        br#"{"format_version":1,"kind":"map","schema":"user-v1","entries":{"alice":{"unknown":true}}}"#,
    );

    let result = client.map::<User>("users").schema("user-v1").open();
    assert!(matches!(
        result,
        Err(Error::Document(DocumentError::Deserialize(_)))
    ));
}

#[test]
fn log_rejects_unsorted_record_ids() {
    assert_invalid_log_ids(
        "01900000-0000-7000-8000-000000000002",
        "01900000-0000-7000-8000-000000000001",
    );
}

#[test]
fn log_rejects_duplicate_record_ids() {
    assert_invalid_log_ids(
        "01900000-0000-7000-8000-000000000001",
        "01900000-0000-7000-8000-000000000001",
    );
}

fn assert_invalid_log_ids(first: &str, second: &str) {
    let store = MemoryStore::default();
    let client = Objsds::builder()
        .store(store.clone())
        .namespace("test")
        .build()
        .expect("client configuration should be valid");
    client
        .log::<String>("events")
        .schema("event-v1")
        .create()
        .expect("events log creation should succeed");
    let document = format!(
        r#"{{"format_version":1,"kind":"log","schema":"event-v1","records":[{{"id":"{first}","value":"first"}},{{"id":"{second}","value":"second"}}]}}"#
    );
    replace_document(&store, "test/logs/events.json", document.as_bytes());

    let result = client.log::<String>("events").schema("event-v1").open();
    assert!(matches!(
        result,
        Err(Error::Document(DocumentError::Corrupt { reason }))
            if reason == "log record IDs must be strictly increasing"
    ));
}

fn replace_document(store: &MemoryStore, path: &str, bytes: &[u8]) {
    let location = Location::new(path).expect("test location should be valid");
    let object = store
        .get(&location)
        .expect("memory read should succeed")
        .expect("test document should exist");
    store
        .replace(&location, &object.version, bytes)
        .expect("test document replacement should succeed");
}

#[test]
fn structure_configuration_errors_remain_actionable() {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace("test")
        .build()
        .expect("client configuration should be valid");

    assert!(matches!(
        client.map::<User>("users").open(),
        Err(Error::Configuration(BuildError::MissingSchema))
    ));
    assert!(matches!(
        client.log::<User>("bad/name").schema("user-v1").open(),
        Err(Error::Configuration(BuildError::InvalidName))
    ));
}

#[test]
fn malformed_persisted_document_is_not_a_compatibility_error() {
    let store = MemoryStore::default();
    store
        .create(
            &Location::new("test/maps/users.json").expect("location should be valid"),
            b"not JSON",
        )
        .expect("malformed fixture creation should succeed");
    let client = Objsds::builder()
        .store(store)
        .namespace("test")
        .build()
        .expect("client configuration should be valid");

    assert!(matches!(
        client.map::<User>("users").schema("user-v1").open(),
        Err(Error::Document(DocumentError::Deserialize(_)))
    ));
}
