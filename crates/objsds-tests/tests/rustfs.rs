use objsds::{Error, Objsds};
use objsds_store::{CreateError, Location, ObjectStore, ReplaceError};
use objsds_tests::{ensure_rustfs_bucket, rustfs_enabled, rustfs_store, unique_namespace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct User {
    name: String,
}

#[test]
fn full_rustfs_experience() -> Result<(), Box<dyn std::error::Error>> {
    if !rustfs_enabled() {
        eprintln!("skipped: run `mise run test:e2e` to start RustFS through Pitchfork");
        return Ok(());
    }

    ensure_rustfs_bucket()?;
    let store = rustfs_store()?;
    let namespace = unique_namespace("rustfs-e2e");
    let client = Objsds::builder()
        .store(store.clone())
        .namespace(&namespace)
        .build()?;

    let users = client.map::<User>("users").schema("user-v1").create()?;
    users.insert(
        "alice",
        User {
            name: "Alice".to_owned(),
        },
    )?;
    assert_eq!(
        users
            .get("alice")?
            .expect("inserted user should be present")
            .name,
        "Alice"
    );
    assert!(matches!(
        client.map::<User>("users").schema("user-v2").open(),
        Err(Error::Incompatible { .. })
    ));

    let events = client
        .log::<String>("events")
        .schema("event-v1")
        .open_or_create()?;
    let first = events.append("created".to_owned())?;
    let second = events.append("updated".to_owned())?;
    assert!(first < second);
    assert_eq!(events.records_after(first)?[0].value, "updated");

    let location = Location::new(format!("{namespace}/contract/cas.json"))?;
    let initial = store
        .create(&location, b"one")
        .expect("contract object creation should succeed");
    assert!(matches!(
        store.create(&location, b"duplicate"),
        Err(CreateError::AlreadyExists { .. })
    ));
    let current = store
        .replace(&location, &initial, b"two")
        .expect("current contract replacement should succeed");
    assert!(matches!(
        store.replace(&location, &initial, b"stale"),
        Err(ReplaceError::Conflict {
            observed: Some(observed)
        }) if observed == current
    ));

    Ok(())
}
