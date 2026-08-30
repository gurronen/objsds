# objsds

Minimal, blocking, leaderless data structures backed by one object each.

`objsds` provides typed `Map` and append-only `Log` APIs over object stores. It
uses read-after-write consistency, conditional creation, and compare-and-swap
replacement rather than a database, coordinator, leader, or continuously
running service.

> **Status:** experimental. The public API and stored JSON format are not yet
> stable.

## Design

Each data-structure instance occupies exactly one object:

```text
<namespace>/maps/<name>.json
<namespace>/logs/<name>.json
```

The object contains both metadata and every record. A mutation reads and
decodes the object, changes it locally, then conditionally replaces the object
using its version token. Concurrent replacement returns a conflict; the
library does not retry automatically.

This deliberately favors a minimal protocol and coherent snapshots over
scale. Writes are proportional to the complete encoded structure size, all
writers to one structure contend on one object, and the structure must fit in
memory. Different structures remain independent.

## Map

A `Map<V>` uses canonical UTF-8 string keys and Serde JSON values. Its initial
operations are:

- `get`
- `insert`
- `insert_if_absent`
- `remove`
- `entries`

Entries are stored and returned in lexicographic key order. A read observes one
complete object version, so `entries` is a coherent snapshot.

## Log

A `Log<T>` is an append-only sequence of immutable records. Its initial
operations are:

- `append`
- `get`
- `records`
- `records_after`

Records receive opaque UUIDv7 identifiers. IDs are unique and sortable, but do
not promise exact wall-clock ordering between concurrent writers. The initial
Log has no mutation, deletion, truncation, contiguous offsets, or consumer
groups.

## Lifecycle

Structure builders expose explicit lifecycle operations:

- `create`: create the object and fail if it exists
- `open`: validate the object and fail if it is absent
- `open_or_create`: atomically create it if absent or validate it if present

Validation checks the structure kind, storage-format version, and stable
application-provided schema identifier. Rust type names are not persistent
schema identifiers.

## Construction

Storage configuration belongs to adapter crates. The intended API is:

```rust,ignore
let store = S3Store::builder()
    .bucket("application-data")
    .region("us-east-1")
    .endpoint("http://localhost:9000")
    .credentials(credentials)
    .path_style(true)
    .build()?;

let client = Objsds::builder()
    .store(store)
    .namespace("production")
    .build()?;

let users = client
    .map::<User>("users")
    .schema("user-json-v1")
    .open_or_create()?;
```

The API is blocking. The core crate has no async runtime or futures dependency.
A storage adapter may have additional internal requirements.

## Crates

- `objsds`: `Map`, `Log`, lifecycle APIs, JSON documents, and public errors
- `objsds-store`: the minimal blocking object-store capability interface
- `objsds-store-memory`: deterministic in-memory reference/test adapter
- `objsds-store-filesystem`: persistent blocking local-filesystem adapter
- `objsds-store-s3`: blocking S3-compatible adapter
- `objsds-tests`: unpublished backend-contract and end-to-end test suite

## Object-store requirements

The protocol needs only three primitive operations:

- get an object and opaque version token
- create an object if absent
- replace an object if its version still matches

Listing, range reads, multipart writes, and multi-object transactions are not
part of the initial protocol.

## Provider notes

AWS constructors conventionally accept bucket, region, endpoint, credentials,
and addressing style. AWS deployments can use provider defaults. Cloudflare R2
uses its account endpoint and `auto` region. Local RustFS commonly uses its S3
endpoint on port 9000, `us-east-1`, explicit credentials, and path-style
addressing.

The official AWS SDK for Rust is async-first. The S3 adapter therefore uses the
blocking API from the lean `s3` client crate and exposes only the three
capabilities required by the core protocol.

### Filesystem adapter

The filesystem adapter maps object locations beneath one configured root. It
uses a per-location advisory lock, an embedded fresh opaque revision, synced
temporary files, and atomic rename to provide the same coherent-read,
create-if-absent, and compare-and-swap promises as other adapters. Lock and
temporary files are physical adapter metadata; each data structure remains one
logical object.

Correctness requires every access to managed files to use the adapter. External
modification is unsupported. The adapter is intended for local filesystems with
reliable advisory locks, atomic same-directory rename, and file and directory
sync. Network or distributed filesystems are unsupported unless they provide
those semantics. Host filesystem and hardware behavior ultimately bound crash
durability.

## Testing

Run the memory-backed suite with:

```console
cargo test --workspace
```

Run the full S3-compatible experience against the RustFS daemon defined in
`pitchfork.toml` with:

```console
mise run test:e2e
```

The task starts RustFS through Pitchfork, creates the test bucket if needed,
and exercises lifecycle, Map, Log, conditional creation, and stale-version
conflicts exclusively through public crate APIs.

### RustFS CAS contention evaluation

Run the opt-in performance evaluation with:

```console
mise run test:perf
```

It starts RustFS, then concurrently attempts writes to one shared Map and one
shared Log without retrying conflicts or introducing a broker. The output is
one stable, key-value line per structure containing attempted operations,
successes, CAS conflicts, elapsed time, operation throughput, and conflict
percentage. `attempt_ops_per_sec` measures complete `Map::insert` or
`Log::append` attempts, not individual HTTP requests.

The defaults are 8 workers and 25 operations per worker. Override them to make
repeatable comparisons with:

```console
OBJSDS_PERF_WORKERS=16 OBJSDS_PERF_OPERATIONS_PER_WORKER=100 mise run test:perf
```

The test is ignored by default, so normal test runs remain fast and do not
require RustFS. Use the same worker and operation counts when comparing runs;
results depend on the machine and object-store environment.

Runnable examples live under `crates/objsds/examples`. Run the persistent local
example with `cargo run -p objsds --example filesystem`.

## Non-goals

- Drop-in replacements for `HashMap`, `BTreeMap`, or `Vec`
- Cross-structure transactions
- Unbounded structures or write scaling independent of structure size
- Automatic conflict retries
- Exact real-time ordering of concurrent Log appends
- Hiding storage-provider consistency limitations
