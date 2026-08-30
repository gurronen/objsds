use objsds_store::Location;
use objsds_store_filesystem::FilesystemStore;
use objsds_tests::assert_backend_contract;

#[test]
fn filesystem_backend_contract() {
    let root = tempfile::tempdir().expect("temporary root should be created");
    let store = FilesystemStore::builder()
        .root(root.path())
        .build()
        .expect("filesystem store should build");
    let location = Location::new("contract/object.json").expect("location should be valid");
    assert_backend_contract(&store, location);
}
