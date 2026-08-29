use objsds::{InsertIfAbsent, Objsds};
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
    assert!(matches!(result, Err(objsds::Error::Incompatible { .. })));
}
