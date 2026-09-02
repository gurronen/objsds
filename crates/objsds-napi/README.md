# objsds Node bindings

This crate is the isolated Node-API adapter for `objsds`. It is not published to
crates.io and does not add Node-specific dependencies to any core crate.

The public TypeScript package lives in [`npm`](npm). Application values cross
the native boundary as JSON strings so serialization policy and TypeScript
generics remain in the TypeScript facade. Every blocking `objsds` operation is
run through napi-rs `AsyncTask`, off the JavaScript event-loop thread.

## Development

```console
cd crates/objsds-napi/npm
npm ci
npm test
npm run check
npm run build
```

The native build produces a platform-specific `.node` file in `npm/`. Generated
binaries, declarations, and TypeScript output are ignored by git.

## Why `AsyncTask`

The core and its adapters deliberately expose blocking APIs. Running those calls
inside an ordinary Node-API method would block Node's event loop. Each lifecycle,
Map, and Log operation is therefore represented by a napi-rs `AsyncTask`:
`compute` runs in libuv's native worker pool and its result resolves a Promise on
the JavaScript thread. This is not a `node:worker_threads` isolate and it does
not make the underlying request cancellable. It is the smallest adapter that
preserves the blocking Rust contract while supporting normal JavaScript
`async`/`await` without freezing timers or unrelated JavaScript work.

napi-rs wires the optional JavaScript `AbortSignal` to each `AsyncTask`.
Cancellation can reject queued work and suppress delivery of an in-flight
result, but it cannot interrupt the blocking store call once `compute` starts.
That call continues in the background, so cancellation of a mutation has an
ambiguous outcome and callers must read fresh state before deciding to retry.

libuv's pool is process-wide and also serves some filesystem, DNS, and crypto
work. Applications issuing many long-running calls should bound concurrency.
A future genuinely async store adapter could instead await non-blocking network
I/O; a dedicated Worker Thread or sidecar process are alternatives when work
must not share libuv's pool.

References: [napi-rs AsyncTask](https://napi.rs/docs/concepts/async-task),
[Node.js event-loop guidance](https://nodejs.org/en/learn/asynchronous-work/dont-block-the-event-loop),
and [Node.js Worker Threads](https://nodejs.org/api/worker_threads.html).

## Native targets

`scripts/build-native.mjs` is the single, shell-independent native build entry
point. It accepts `--target <rust-triple>`, rejects targets outside the supported
matrix, and works on Windows, Apple Silicon macOS, and Linux. CI builds native
artifacts on Linux x64/arm64, macOS arm64, and Windows x64. The loader and target
allowlist also cover Windows arm64 and Linux musl x64/arm64 for release
cross-compilation. Intel Macs are explicitly unsupported; the native loader
fails with an actionable error on `darwin-x64`.

## npm releases

The manually dispatched `Release` workflow is the single release action for
crates.io, npm, and GitHub. It never runs automatically after a merge. With no
version override it reads the current `objsds` and `@objsds/client` registry
versions and selects the next patch (the initial local `0.1.0` baseline resolves
to `0.1.1`). Source manifests remain at their development baseline, while the
workflow stages the resolved version into Cargo and npm metadata before
packaging, following the tag-derived release pattern used by OpenCode.

By default the action is a non-publishing rehearsal: it packages every Cargo
crate, builds all seven native targets, assembles and validates the npm package,
and retains the resulting artifacts in GitHub Actions. Selecting `publish`
requires current `main`; only after both registries succeed does the final job
create `v<version>` and a GitHub Release containing every `.node` binary and the
exact npm `.tgz`. Keeping the binaries together makes the initial consumer flow
straightforward; per-platform optional packages can be introduced later if
package download size becomes a concern. `prepack` compiles TypeScript but does
not rebuild native code, so the publishing runner cannot overwrite the
collected cross-platform artifacts.

The initial npm publish uses a granular automation token stored only as
`NPM_TOKEN` in the protected GitHub `npm` environment. Do not commit the token
or add it to repository-level configuration. After the package exists on npm,
replace token authentication with npm trusted publishing: configure
`gurronen/objsds`, workflow `release.yml`, environment `npm`, and the
`npm publish` action under the package's Trusted publishing settings; then use
npm 11.5.1 or newer in the publish job and remove `NODE_AUTH_TOKEN`. The job
already grants the required `id-token: write` permission and publishes with
provenance.
