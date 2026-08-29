use objsds_store::Location;
use objsds_store_memory::MemoryStore;
use objsds_tests::assert_backend_contract;

#[test]
fn memory_store_satisfies_the_backend_contract() {
    assert_backend_contract(
        &MemoryStore::default(),
        Location::new("contract/object.json").expect("location should be valid"),
    );
}
