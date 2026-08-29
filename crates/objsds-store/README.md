# objsds-store

Minimal blocking object-store capability traits used by [`objsds`](https://crates.io/crates/objsds).

The `ObjectStore` contract contains only the operations required by single-object data structures:

- read an object and its opaque version token;
- create an object if it is absent;
- replace an object if its version still matches.

```rust
use objsds_store::{CreateError, Location, Object, ObjectStore, ReplaceError, Version};

pub trait ObjectStore: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error>;
    fn create(&self, location: &Location, bytes: &[u8])
        -> Result<Version, CreateError<Self::Error>>;
    fn replace(&self, location: &Location, expected: &Version, bytes: &[u8])
        -> Result<Version, ReplaceError<Self::Error>>;
}
```

Listing, deletion, range reads, and multi-object transactions are intentionally outside the contract.

See the [repository](https://github.com/gurronen/objsds) for the protocol design and adapter implementations.

## License

MIT
