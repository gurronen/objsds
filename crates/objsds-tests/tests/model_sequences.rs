use std::collections::BTreeMap;

use objsds::{InsertIfAbsent, Objsds, Record};
use objsds_store_memory::MemoryStore;

const SEEDS: [u64; 4] = [
    0x243f_6a88_85a3_08d3,
    0x1319_8a2e_0370_7344,
    0xa409_3822_299f_31d0,
    0x082e_fa98_ec4e_6c89,
];

struct Sequence(u64);

impl Sequence {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        // SplitMix64 gives a stable, well-distributed sequence without adding a test dependency.
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

#[test]
fn map_matches_btree_map_across_operation_sequences() {
    for seed in SEEDS {
        let client = Objsds::builder()
            .store(MemoryStore::default())
            .namespace(format!("map-model-{seed:016x}"))
            .build()
            .expect("model client should build");
        let map = client
            .map::<u64>("values")
            .schema("model-v1")
            .create()
            .expect("model map should be created");
        let mut model = BTreeMap::new();
        let mut sequence = Sequence::new(seed);

        for step in 0..128 {
            let operation = sequence.next();
            let key = format!("key-{}", sequence.next() % 24);
            let value = sequence.next();

            match operation % 5 {
                0 => {
                    map.insert(key.clone(), value)
                        .expect("model insert should succeed");
                    model.insert(key, value);
                }
                1 => {
                    let actual = map.remove(&key).expect("model remove should succeed");
                    let expected = model.remove(&key);
                    assert_eq!(actual, expected, "seed={seed:#018x}, step={step}, remove");
                }
                2 => {
                    let actual = map.get(&key).expect("model get should succeed");
                    assert_eq!(
                        actual,
                        model.get(&key).copied(),
                        "seed={seed:#018x}, step={step}, get"
                    );
                }
                3 => {
                    let actual = map
                        .insert_if_absent(key.clone(), value)
                        .expect("conditional insert should succeed");
                    match model.entry(key) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            assert!(
                                matches!(actual, InsertIfAbsent::Inserted(_)),
                                "seed={seed:#018x}, step={step}, vacant insert_if_absent"
                            );
                            entry.insert(value);
                        }
                        std::collections::btree_map::Entry::Occupied(entry) => assert_eq!(
                            actual,
                            InsertIfAbsent::Occupied(*entry.get()),
                            "seed={seed:#018x}, step={step}, occupied insert_if_absent"
                        ),
                    }
                }
                _ => {}
            }

            let actual = map.entries().expect("model entries should succeed");
            let expected: Vec<_> = model
                .iter()
                .map(|(key, value)| (key.clone(), *value))
                .collect();
            assert_eq!(
                actual, expected,
                "seed={seed:#018x}, step={step}, complete map state"
            );
        }
    }
}

#[test]
fn log_preserves_records_and_records_after_invariants() {
    for seed in SEEDS {
        let client = Objsds::builder()
            .store(MemoryStore::default())
            .namespace(format!("log-model-{seed:016x}"))
            .build()
            .expect("model client should build");
        let log = client
            .log::<u64>("events")
            .schema("model-v1")
            .create()
            .expect("model log should be created");
        let mut model: Vec<Record<u64>> = Vec::new();
        let mut sequence = Sequence::new(seed);

        for step in 0..64 {
            let value = sequence.next();
            let id = log.append(value).expect("model append should succeed");
            model.push(Record { id, value });
            model.sort_by_key(|record| record.id);

            let records = log.records().expect("model records should succeed");
            assert_eq!(
                records, model,
                "seed={seed:#018x}, step={step}, complete log state"
            );
            assert!(
                records.windows(2).all(|pair| pair[0].id < pair[1].id),
                "seed={seed:#018x}, step={step}, IDs must be strictly ordered"
            );

            let pivot = model[(sequence.next() as usize) % model.len()].id;
            let expected: Vec<_> = model
                .iter()
                .filter(|record| record.id > pivot)
                .cloned()
                .collect();
            assert_eq!(
                log.records_after(pivot)
                    .expect("records_after should succeed"),
                expected,
                "seed={seed:#018x}, step={step}, records_after"
            );

            let expected_record = model.iter().find(|record| record.id == pivot).cloned();
            assert_eq!(
                log.get(pivot).expect("model get should succeed"),
                expected_record,
                "seed={seed:#018x}, step={step}, get"
            );
        }
    }
}
