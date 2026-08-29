# objsds-store-memory

Deterministic in-memory implementation of [`objsds_store::ObjectStore`](https://crates.io/crates/objsds-store).

Use it for tests, examples, and local experimentation with [`objsds`](https://crates.io/crates/objsds) without an external object store.

```rust
use objsds::Objsds;
use objsds_store_memory::MemoryStore;

let client = Objsds::builder()
    .store(MemoryStore::default())
    .namespace("test")
    .build()?;

# Ok::<(), Box<dyn std::error::Error>>(())
```

Clones share state through an in-process synchronized store. The adapter implements conditional creation and version-based replacement, making CAS conflicts deterministic to test. It is not persistent and is not intended as a production backend.

See the [repository](https://github.com/gurronen/objsds) for complete examples.

## License

MIT
