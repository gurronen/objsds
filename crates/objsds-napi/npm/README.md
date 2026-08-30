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

Expected failures reject with `ObjsdsError`. Its `code` and `details` fields are
stable for programmatic handling. In particular, `ERR_OBJSDS_CONFLICT` carries
`expectedVersion` and `observedVersion` details.

## Native development builds

Set `OBJSDS_NATIVE_BINARY` to an explicit `.node` file to test a custom build.
The override is strict and never falls back silently. Normal installations load
the binary bundled with the package or the matching optional platform package.
