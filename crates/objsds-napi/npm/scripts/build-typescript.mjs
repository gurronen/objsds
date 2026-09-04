import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
rmSync(join(packageRoot, "dist"), { recursive: true, force: true });
const command = process.platform === "win32" ? "tsc.cmd" : "tsc";
const result = spawnSync(command, ["-p", "tsconfig.build.json"], {
  cwd: packageRoot,
  stdio: "inherit",
  shell: process.platform === "win32",
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
