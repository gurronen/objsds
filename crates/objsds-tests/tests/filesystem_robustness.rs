use std::sync::{Arc, Barrier};

use objsds_store::{CreateError, Location, ObjectStore};
use objsds_store_filesystem::{FilesystemStore, StoreError};

fn store(root: &tempfile::TempDir) -> FilesystemStore {
    FilesystemStore::builder()
        .root(root.path())
        .build()
        .expect("store should build")
}

#[test]
fn persists_across_store_instances() {
    let root = tempfile::tempdir().expect("root should exist");
    let location = Location::new("maps/users.json").expect("location should be valid");
    let version = store(&root)
        .create(&location, b"users")
        .expect("create should succeed");
    let object = store(&root)
        .get(&location)
        .expect("get should succeed")
        .expect("object should exist");
    assert_eq!(object.bytes, b"users");
    assert_eq!(object.version, version);
}

#[test]
fn concurrent_create_has_one_winner() {
    let root = tempfile::tempdir().expect("root should exist");
    let store = Arc::new(store(&root));
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let location = Location::new("logs/events.json").expect("location should be valid");
                barrier.wait();
                store.create(&location, b"events")
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread should finish"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(CreateError::AlreadyExists { .. })))
            .count(),
        7
    );
}

#[test]
fn rejects_traversal_and_corrupt_managed_files() {
    let root = tempfile::tempdir().expect("root should exist");
    let store = store(&root);
    let traversal =
        Location::new("../outside").expect("generic location permits provider-specific values");
    assert!(matches!(
        store.get(&traversal),
        Err(StoreError::InvalidLocation)
    ));

    std::fs::create_dir_all(root.path().join("maps")).expect("directory should be created");
    std::fs::write(root.path().join("maps/broken.json"), b"broken")
        .expect("file should be written");
    let broken = Location::new("maps/broken.json").expect("location should be valid");
    assert!(matches!(
        store.get(&broken),
        Err(StoreError::InvalidObjectFormat)
    ));
}
