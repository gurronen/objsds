import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { releaseTargets, validateRelease } from "../scripts/validate-release.mjs";

function fixture(version = "1.2.3") {
  const root = mkdtempSync(join(tmpdir(), "objsds-release-"));
  writeFileSync(join(root, "package.json"), JSON.stringify({ version }));
  for (const target of releaseTargets) {
    writeFileSync(join(root, `objsds-napi.${target}.node`), target);
  }
  return root;
}

test("accepts a complete native release", () => {
  assert.doesNotThrow(() => validateRelease(fixture(), "1.2.3"));
});

test("rejects a missing target artifact", () => {
  const root = fixture();
  const missing = releaseTargets.at(-1);
  writeFileSync(join(root, `objsds-napi.${missing}.node.moved`), "missing");
  rmSync(join(root, `objsds-napi.${missing}.node`));
  assert.throws(() => validateRelease(root, "1.2.3"), /Missing native release artifacts/);
});

test("rejects a version that differs from package.json", () => {
  assert.throws(() => validateRelease(fixture(), "1.2.4"), /does not match package.json/);
});

test("forwards the selected cross-compilation mode to napi", () => {
  const bin = mkdtempSync(join(tmpdir(), "objsds-napi-bin-"));
  const output = join(bin, "arguments");
  const napi = join(bin, "napi");
  writeFileSync(napi, `#!/bin/sh\nprintf '%s\\n' "$@" > "$OBJSDS_NAPI_ARGS"\n`);
  chmodSync(napi, 0o755);

  const result = spawnSync(
    process.execPath,
    ["scripts/build-native.mjs", "--target", "x86_64-unknown-linux-musl", "--cross-compile"],
    {
      cwd: join(import.meta.dirname, ".."),
      env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, OBJSDS_NAPI_ARGS: output },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(readFileSync(output, "utf8"), /--cross-compile/);
});
