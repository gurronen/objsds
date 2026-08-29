use objsds::Objsds;
use objsds_store_memory::MemoryStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct User {
    name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace("example")
        .build()?;
    let users = client
        .map::<User>("users")
        .schema("user-v1")
        .open_or_create()?;

    users.insert(
        "alice",
        User {
            name: "Alice".to_owned(),
        },
    )?;
    println!("{:?}", users.get("alice")?);
    Ok(())
}
