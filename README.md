# objsds

Blocking, leaderless `Map` and append-only `Log` data structures, plus a
brokerless single-object work queue, for object storage.

`objsds` is a small Rust workspace for applications that need coherent shared
state without operating a database or coordination service. Each data structure
is stored as one JSON object and updated with conditional creation and
compare-and-swap replacement. Adapters are included for memory, local
filesystems, and S3-compatible stores.

> **Status:** experimental. The public API and stored JSON format are not yet
> stable.

## Development setup

The repository uses [mise](https://mise.jdx.dev/) to provide the Rust toolchain
and development utilities. Install the pinned tools and repository hooks with:

```console
mise install
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution and pull-request
guidance.

## Design

Each data-structure instance occupies exactly one object:

```text
<namespace>/maps/<name>.json
<namespace>/logs/<name>.json
<namespace>/queues/<name>.json
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

## Queue

The publishable `objsds-queue` crate provides direct `publish`, `claim`, and
`ack` operations. Queue is not available on the `Objsds` client or in
`@objsds/client`; use the Rust crate against an `ObjectStore`. A claim grants a time-bounded lease; if it is not
acknowledged, the message becomes claimable again at the lease deadline. Each
reclaim receives a new opaque lease token, so an old worker cannot acknowledge
a newer claim.

Delivery is **at least once**, not exactly once. Workers must make handlers
idempotent because processing may complete before a failed acknowledgement and
because lease expiry permits concurrent duplicate processing. Only a successful
`Ack::Acknowledged` is durable; `NotFound`, `LeaseMismatch`, and `LeaseExpired`
are snapshot classifications and are not written back. Lease safety
across processes depends on clock synchronization. There is no broker,
consumer group, group commit, long polling, automatic CAS retry, or dead-letter
queue. Like Map and Log, the complete queue is one bounded JSON object and each
mutation rewrites it with compare-and-swap.

```rust,ignore
let queue = objsds_queue::QueueBuilder::<_, Job>::new(store, "production", "jobs")
    .schema("job-json-v1")
    .open_or_create()?;
let id = queue.publish(job)?;
if let Some(claim) = queue.claim(std::time::Duration::from_secs(30))? {
    process(&claim.value)?;
    queue.ack(id, claim.lease_token)?;
}
```

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
- `objsds-queue`: brokerless single-object queue with leased at-least-once delivery
- `objsds-store`: the minimal blocking object-store capability interface
- `objsds-store-memory`: deterministic in-memory reference/test adapter
- `objsds-store-filesystem`: persistent blocking local-filesystem adapter
- `objsds-store-s3`: blocking S3-compatible adapter
- `objsds-napi`: unpublished Node-API adapter and `@objsds/client` TypeScript package
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

## Testing and validation

Use the mise tasks so local validation uses the same pinned tools as the
project. Run the complete suite before opening a pull request:

```console
mise run ci
```

The available validation tasks are:

| Command | Purpose |
| --- | --- |
| `mise run pre-commit` | Run the checks configured for the pre-commit hook |
| `mise run check` | Run applicable project checks through hk |
| `mise run test` | Run Rust tests for all targets |
| `mise run fmt` | Check Rust formatting |
| `mise run lint` | Run Clippy for all targets and features with warnings denied |
| `mise run deny` | Check dependency advisories, licenses, bans, and sources |
| `mise run package` | Validate all publishable workspace packages |
| `mise run ci` | Run formatting, linting, tests, dependency policy, and packaging |
| `mise run test:e2e` | Start RustFS and run the S3 end-to-end suite |
| `mise run test:perf` | Measure RustFS Map and Log throughput under CAS contention |

### S3 end-to-end tests

`mise run test:e2e` starts the RustFS daemon defined in `pitchfork.toml`, creates
the test bucket if needed, and exercises lifecycle, Map, Log, Queue
publish/claim/ack and lease reclaim, conditional creation, and stale-version
conflicts exclusively through public crate APIs.
It is separate from `mise run ci` because it requires a local RustFS service.

### RustFS performance evaluation

`mise run test:perf` starts RustFS and runs both CAS contention and Queue
work-item throughput evaluations. The contention evaluation concurrently
attempts writes to one shared Map and one shared Log without retrying conflicts
or introducing a broker. Its output is one stable, key-value line per structure
containing attempted operations, successes, CAS conflicts, elapsed time, operation
throughput, and conflict percentage. `attempt_ops_per_sec` measures complete
`Map::insert` or `Log::append` attempts, not individual HTTP requests.

The Queue evaluation prepublishes work outside the timed interval, then measures
complete claim-and-ack work items against one shared queue. Benchmark workers
explicitly retry CAS conflicts; the Queue API itself still does not retry. Its
stable output reports clients, items, completed items, elapsed milliseconds,
work items per second, CAS conflicts, and transient responses. See the
[one-off RustFS Queue report](QUEUE_PERFORMANCE.md) for a 1–50 client matrix.
The default is 100 work items and 8 clients. Override it with
`OBJSDS_QUEUE_PERF_ITEMS` and `OBJSDS_QUEUE_PERF_CLIENTS`.

The Map and Log defaults are 8 workers and 25 operations per worker. Override
them to make repeatable comparisons with:

```console
OBJSDS_PERF_WORKERS=16 OBJSDS_PERF_OPERATIONS_PER_WORKER=100 mise run test:perf
```

The performance test is ignored by default, so normal test runs remain fast and
do not require RustFS. Use the same worker and operation counts when comparing
runs; results depend on the machine and object-store environment.

Runnable examples live under `crates/objsds/examples`. Run the persistent local
example with `cargo run -p objsds --example filesystem`.

## TypeScript

The isolated [`@objsds/client`](crates/objsds-napi/npm) package provides typed
Map and Log APIs for Node.js 22 or newer. Queue remains Rust-only via
`objsds-queue`. It supports the filesystem,
S3-compatible, and in-memory stores, preserves explicit conflict behavior, and runs every blocking
Rust operation away from the JavaScript event-loop thread.

Develop and test the bindings with:

```console
cd crates/objsds-napi/npm
npm ci
npm test
npm run check
npm run build
```

## Non-goals

- Drop-in replacements for `HashMap`, `BTreeMap`, or `Vec`
- Cross-structure transactions
- Unbounded structures or write scaling independent of structure size
- Automatic conflict retries
- Exact real-time ordering of concurrent Log appends
- Hiding storage-provider consistency limitations
