import { spawnSync } from "node:child_process";

export const supportedTargets = [
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "x86_64-unknown-linux-musl",
  "aarch64-unknown-linux-musl",
  "aarch64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "aarch64-pc-windows-msvc",
];

const forwarded = process.argv.slice(2);
const targetIndex = forwarded.indexOf("--target");
const target = targetIndex === -1 ? process.env.OBJSDS_NAPI_TARGET : forwarded[targetIndex + 1];
if (target && !supportedTargets.includes(target)) {
  console.error(`Unsupported native target ${target}. Expected one of:\n${supportedTargets.join("\n")}`);
  process.exit(2);
}
if (targetIndex !== -1 && !target) {
  console.error("--target requires a Rust target triple");
  process.exit(2);
}

const buildFlags = forwarded.filter(
  (_, index) => targetIndex === -1 || (index !== targetIndex && index !== targetIndex + 1),
);
const supportedBuildFlags = new Set(["--cross-compile", "-x", "--use-napi-cross"]);
const unsupportedFlag = buildFlags.find((flag) => !supportedBuildFlags.has(flag));
if (unsupportedFlag) {
  console.error(`Unsupported native build flag ${unsupportedFlag}`);
  process.exit(2);
}

const args = [
  "build",
  "--manifest-path", "../Cargo.toml",
  "--package", "objsds-napi",
  "--platform",
  "--release",
  "--output-dir", ".",
  "--no-js",
  "--dts", "native.d.ts",
];
if (target) args.push("--target", target);
args.push(...buildFlags);

const result = spawnSync("napi", args, { stdio: "inherit", shell: process.platform === "win32" });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
