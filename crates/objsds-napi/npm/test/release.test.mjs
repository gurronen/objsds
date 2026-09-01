import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
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
