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
