use objsds_store::{CreateError, Location, ObjectStore, ReplaceError};
use objsds_store_memory::MemoryStore;

fn assert_contract(store: &impl ObjectStore<Error = std::convert::Infallible>) {
    let location = Location::new("contract/object.json").expect("location should be valid");
    assert!(
        store
            .get(&location)
            .expect("initial read should succeed")
            .is_none()
    );

    let first = store
        .create(&location, b"one")
        .expect("initial create should succeed");
    assert!(matches!(
        store.create(&location, b"duplicate"),
        Err(CreateError::AlreadyExists { .. })
    ));

    let second = store
        .replace(&location, &first, b"two")
        .expect("current replacement should succeed");
    assert!(matches!(
        store.replace(&location, &first, b"stale"),
        Err(ReplaceError::Conflict {
            observed: Some(observed)
        }) if observed == second
    ));
    assert_eq!(
        store
            .get(&location)
            .expect("final read should succeed")
            .expect("object should exist")
            .bytes,
        b"two"
    );
}

#[test]
fn memory_store_satisfies_the_backend_contract() {
    assert_contract(&MemoryStore::default());
}
