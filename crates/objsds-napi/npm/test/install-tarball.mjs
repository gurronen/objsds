import { spawnSync } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const temporary = await mkdtemp(join(tmpdir(), "objsds-package-"));
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

try {
  const packed = run(npm, ["pack", "--ignore-scripts", "--json", "--pack-destination", temporary], packageRoot, true);
  const [{ filename }] = JSON.parse(packed);
  const tarball = join(temporary, filename);

  await writeFile(
    join(temporary, "package.json"),
    JSON.stringify({ name: "objsds-consumer-smoke", private: true, type: "module" }),
  );
  run(npm, ["install", "--ignore-scripts", "--no-package-lock", tarball], temporary);
  await writeFile(
    join(temporary, "consumer.mjs"),
    `import assert from "node:assert/strict";
import { Objsds } from "@objsds/client";
const map = await Objsds.memory({ namespace: "installed" })
  .map("values", { schema: "json-v1" }).create();
await map.insert("answer", 42);
assert.equal(await map.get("answer"), 42);
map.close();
`,
  );
  run(process.execPath, ["consumer.mjs"], temporary);
} finally {
  await rm(temporary, { recursive: true, force: true });
}

function run(command, args, cwd, capture = false) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status}`);
  return result.stdout ?? "";
}
