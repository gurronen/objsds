# objsds

Typed, blocking, leaderless `Map` and append-only `Log` data structures backed by one object each.

`objsds` uses read-after-write consistency, conditional creation, and compare-and-swap replacement instead of a database or coordination service. Storage adapters are provided separately.

> **Status:** experimental. The public API and stored JSON format are not yet stable.

## Example

```rust
use objsds::Objsds;
use objsds_store_memory::MemoryStore;

let client = Objsds::builder()
    .store(MemoryStore::default())
    .namespace("example")
    .build()?;

let users = client
    .map::<String>("users")
    .schema("user-name-v1")
    .open_or_create()?;

users.insert("alice", "Alice".to_owned())?;
assert_eq!(users.get("alice")?, Some("Alice".to_owned()));

# Ok::<(), Box<dyn std::error::Error>>(())
```

## Design constraints

Each structure is encoded as one JSON object. A mutation reads and decodes the complete object, changes it locally, and conditionally replaces the observed version. Concurrent replacement returns a conflict; the library does not retry automatically.

This favors a minimal protocol and coherent snapshots over unbounded size or write scaling. See the [repository](https://github.com/gurronen/objsds) for architecture, provider setup, examples, and testing instructions.

## License

MIT
