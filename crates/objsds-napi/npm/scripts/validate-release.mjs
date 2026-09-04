import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const releaseTargets = [
  "darwin-arm64",
  "linux-arm64-gnu",
  "linux-arm64-musl",
  "linux-x64-gnu",
  "linux-x64-musl",
  "win32-arm64-msvc",
  "win32-x64-msvc",
];

export function validateRelease(packageRoot, version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Invalid npm release version: ${version}`);
  }

  const packageJson = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
  if (packageJson.version !== version) {
    throw new Error(
      `Release version ${version} does not match package.json version ${packageJson.version}`,
    );
  }

  const missing = releaseTargets
    .map((target) => `objsds-napi.${target}.node`)
    .filter((filename) => !existsSync(join(packageRoot, filename)));
  if (missing.length > 0) {
    throw new Error(`Missing native release artifacts:\n${missing.join("\n")}`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
  const version = process.argv[2];
  if (!version) throw new Error("Usage: node scripts/validate-release.mjs <version>");
  validateRelease(packageRoot, version);
  console.log(`Validated @objsds/client ${version} with ${releaseTargets.length} native targets`);
}
