use objsds::Objsds;
use objsds_store_memory::MemoryStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct User {
    name: String,
}

#[test]
fn complete_memory_experience() {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace("memory-e2e")
        .build()
        .expect("client configuration should be valid");

    let users = client
        .map::<User>("users")
        .schema("user-v1")
        .open_or_create()
        .expect("users map should open");
    users
        .insert(
            "alice",
            User {
                name: "Alice".to_owned(),
            },
        )
        .expect("user insertion should succeed");
    assert_eq!(
        users
            .get("alice")
            .expect("user read should succeed")
            .expect("inserted user should exist")
            .name,
        "Alice"
    );

    let events = client
        .log::<String>("events")
        .schema("event-v1")
        .open_or_create()
        .expect("events log should open");
    let id = events
        .append("user-created".to_owned())
        .expect("event append should succeed");
    assert_eq!(
        events
            .get(id)
            .expect("event read should succeed")
            .expect("appended event should exist")
            .value,
        "user-created"
    );
}
