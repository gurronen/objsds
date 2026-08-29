# objsds-store-s3

Blocking S3-compatible implementation of [`objsds_store::ObjectStore`](https://crates.io/crates/objsds-store) for [`objsds`](https://crates.io/crates/objsds).

```rust
use objsds_store_s3::{Credentials, S3Store};

let store = S3Store::builder()
    .bucket("application-data")
    .region("us-east-1")
    .endpoint("http://localhost:9000")
    .credentials(Credentials::new("access-key", "secret-key"))
    .path_style(true)
    .build()?;

# Ok::<(), Box<dyn std::error::Error>>(())
```

The adapter exposes only get, create-if-absent, and conditional replacement. It uses a blocking S3 client and supports custom endpoints and path-style addressing for compatible providers such as RustFS.

Correctness requires the provider to supply read-after-write consistency and reliable conditional request semantics. Test those guarantees for S3-compatible providers before production use.

See the [repository](https://github.com/gurronen/objsds) for RustFS setup and end-to-end tests.

## License

MIT
