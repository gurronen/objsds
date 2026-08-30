use objsds::Objsds;
use objsds_store_filesystem::FilesystemStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = FilesystemStore::builder().root("./objsds-data").build()?;
    let client = Objsds::builder()
        .store(store)
        .namespace("example")
        .build()?;
    let events = client
        .log::<String>("events")
        .schema("event-json-v1")
        .open_or_create()?;
    events.append("filesystem adapter ready".to_owned())?;
    println!("{} records", events.records()?.len());
    Ok(())
}
