# Vision

## Purpose

`objsds` explores the smallest useful distributed data-structure protocol that
can be built directly on modern object-store guarantees.

Object stores provide durable objects, read-after-write consistency, opaque
version identifiers, conditional creation, and conditional replacement. Those
primitives are sufficient for useful leaderless structures without deploying a
database or coordination service.

The project chooses comprehensibility and a minimal dependency footprint over
large-object performance or horizontal scaling within one structure.

## Principles

1. **One structure, one object.** Metadata and records are encoded together.
2. **One write, one CAS.** A mutation replaces exactly one observed object
   version and never spans objects.
3. **Conflicts are explicit.** The library performs one CAS attempt and leaves
   retry policy to its caller.
4. **Blocking by design.** Public I/O methods block and require no async runtime.
5. **Stable storage contracts.** Stored objects carry a format version,
   structure kind, and application-defined schema identifier.
6. **Provider details stay outside the core.** Small capability traits separate
   data-structure logic from AWS S3, R2, RustFS, and test adapters.
7. **Claims remain honest.** APIs do not imitate local collections where remote
   semantics materially differ.

## Why Map rather than `HashMap`

Rust's `HashMap` and `BTreeMap` APIs provide useful vocabulary: `get`, `insert`,
`remove`, and iteration. Their borrowed references, closure-based `Entry` API,
and in-place mutation do not cross an object-store boundary.

`Map<V>` therefore uses owned values and explicit fallible I/O. UTF-8 keys keep
object data inspectable and avoid introducing a separate canonical key codec.
A `BTreeMap` representation makes JSON deterministic and snapshot iteration
lexicographic.

Because the whole Map is one object, every write contends on the Map's version.
This is an intentional serialization point, not a hidden scalability claim.

## Why Log rather than `Vec`

`Vec` assumes cheap indexing, in-place changes, and positional insertion.
Those operations are misleading for remote concurrent state. The useful subset
is an append-only Log with immutable records and snapshot traversal.

UUIDv7 gives each append a decentralized, sortable identifier without a shared
tail counter. It does not establish exact real-time order across concurrent
clients with different clocks. IDs remain opaque so the implementation can
evolve.

## Consistency model

A successful mutation is a conditional replacement of one previously observed
object version. If another client replaces the object first, the operation
returns a conflict with the observed current version when available.

A complete read decodes one immutable object version. Map entry traversal and
Log traversal are therefore coherent snapshots. There are no atomic operations
across structures.

The first implementation does not retry conflicts automatically. Even when an
operation appears replayable, retry limits, backoff, and business intent belong
to the caller.

## Serialization

Serde JSON is the sole initial format. It is inspectable, broadly understood,
and sufficient for validating the protocol before optimizing representation.
Each structure requires an application-owned schema identifier such as
`user-json-v1`. Rust type names are not stable storage contracts.

A future format must be introduced through an explicit stored-format version or
migration design, never by silently changing encoded bytes.

## Construction research

Rust collection constructors separate cheap construction from mutation. S3
SDKs instead use builders because clients need provider-specific configuration:
bucket, region, endpoint, credentials, and path-style versus virtual-hosted
addressing.

`objsds` follows both patterns at separate layers:

- a storage-adapter builder handles S3-compatible configuration;
- an `Objsds` builder combines a store with a namespace;
- typed structure builders select a name and schema, then explicitly `create`,
  `open`, or `open_or_create` persistent state.

AWS can use its standard region and credential discovery. Cloudflare R2 uses an
account endpoint and `auto` region. RustFS generally uses the S3 API endpoint,
`us-east-1`, explicit credentials, and path-style addressing.

The official AWS Rust SDK is asynchronous and normally uses Tokio. Since this
project chooses blocking public I/O, the S3 adapter uses the blocking API from
the lean `s3` client crate rather than hiding a runtime in the core. The
adapter exposes only get, create-if-absent, and conditional replacement.

## Dependency policy

The core accepts dependencies that directly implement its contract: Serde,
JSON, and UUIDv7 generation. Runtime frameworks and provider SDKs do not belong
there. Storage adapters are separate crates so applications pay only for the
backends they select.

## Deliberate limitations

For an encoded structure of size *n*, mutation performs O(*n*) transfer,
decoding, encoding, and replacement. Structures must fit in memory and within
the provider's object-size limit. High-contention structures will experience
frequent CAS conflicts.

These limitations are features of the initial experiment: they keep the
protocol auditable. Sharding, manifests, segments, multipart updates,
compaction, consumer groups, and multi-object protocols require separate future
designs and must not erode the one-object guarantee silently.

## Milestones

1. Define blocking storage capabilities and a deterministic memory adapter.
2. Implement versioned single-object lifecycle operations.
3. Implement UTF-8/JSON `Map` snapshots and CAS mutations.
4. Implement UUIDv7 append-only `Log` snapshots and CAS appends.
5. Validate behavior under deterministic concurrent conflicts.
6. Implement and test a blocking S3-compatible adapter against local RustFS.
7. Run backend-contract and full Map/Log journeys through a dedicated,
   unpublished end-to-end test crate managed by Pitchfork.
