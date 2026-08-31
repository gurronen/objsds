# `@objsds/client`

Typed Node.js bindings for [`objsds`](https://github.com/gurronen/objsds).

The package requires Node.js 22 or newer. All persistent operations return
Promises because the underlying Rust API is blocking and executes on native
worker threads rather than Node's event-loop thread.

## Example

```ts
import { Objsds } from "@objsds/client";

interface User {
  name: string;
  active: boolean;
}

const client = Objsds.s3({
  namespace: "production",
  bucket: "application-data",
  region: "us-east-1",
  endpoint: "http://localhost:9000",
  pathStyle: true,
  credentials: {
    accessKeyId: "access",
    secretAccessKey: "secret",
  },
});

const users = await client
  .map<User>("users", { schema: "user-json-v1" })
  .openOrCreate();

await users.insert("alice", { name: "Alice", active: true });
console.log(await users.get("alice"));
```

`Objsds.memory()` creates an in-process store for tests and local use. Handles
created by one memory client share its store. `Objsds.filesystem({ namespace,
root })` persists structures beneath a local directory and can be opened by
multiple clients or processes using the adapter's locking and atomic-replacement
semantics.

## Semantics

- Values must be JSON-compatible. `undefined`, `bigint`, functions, symbols,
  cycles, and other values without a JSON representation are rejected.
- Each read observes one coherent object version.
- Mutations make one compare-and-swap attempt. The binding does not retry or
  serialize concurrent calls.
- A `null` value is distinct from an absent map entry or log record.
- `Version` and `LogId` are opaque branded strings.
- Map and Log handles support `close()` and `Symbol.dispose`; otherwise a
  `FinalizationRegistry` releases their native registry entries after garbage collection.

Expected failures reject with `ObjsdsError`. Its `code` and `details` fields are
stable for programmatic handling. In particular, `ERR_OBJSDS_CONFLICT` carries
`expectedVersion` and `observedVersion` details.

## Native execution model

Every persistent call uses napi-rs `AsyncTask`. The blocking Rust operation runs
in libuv's shared native worker pool and resolves a JavaScript Promise when it
finishes, so `await map.get(...)` does not run storage I/O on the event-loop
thread. This is deliberately lighter than creating a JavaScript
[`Worker`](https://nodejs.org/api/worker_threads.html), but high-concurrency
applications should still bound outstanding calls because the libuv pool is
shared with other Node facilities. Every lifecycle and data operation accepts
`{ signal?: AbortSignal }`. Aborting rejects queued work promptly; if blocking
storage I/O has already started, the Promise is rejected but that native call
may continue in the background and may already have applied a mutation. Treat
cancellation of a mutation as an ambiguous outcome and read fresh state before
retrying. See the [native crate design](https://github.com/gurronen/objsds/blob/main/crates/objsds-napi/README.md#why-asynctask)
for alternatives and links.

## Native development builds

Set `OBJSDS_NATIVE_BINARY` to an explicit `.node` file to test a custom build.
The override is strict and never falls back silently. Normal installations load
the binary bundled with the package or the matching optional platform package.
Use `npm run build:native -- --target <rust-triple>` for an explicit target; the
build entry point validates the seven supported targets: Windows x64/arm64,
Apple Silicon macOS, and Linux glibc/musl x64/arm64. Intel Macs are explicitly
unsupported and fail with an actionable loader error.

Run the real S3 binding journey locally with Docker available:

```console
mise run test:typescript:s3
```
