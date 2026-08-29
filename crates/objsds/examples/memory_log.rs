use objsds::Objsds;
use objsds_store_memory::MemoryStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Objsds::builder()
        .store(MemoryStore::default())
        .namespace("example")
        .build()?;
    let events = client
        .log::<String>("events")
        .schema("event-v1")
        .open_or_create()?;

    let id = events.append("user-created".to_owned())?;
    let record = events
        .get(id)?
        .ok_or_else(|| std::io::Error::other("appended record was not found"))?;
    println!("{}: {}", id, record.value);
    Ok(())
}
