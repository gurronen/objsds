import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

function detectLibc(): "gnu" | "musl" | undefined {
  if (process.platform !== "linux") return undefined;
  try {
    const report = process.report?.getReport() as
      | { header?: { glibcVersionRuntime?: string } }
      | undefined;
    return report?.header?.glibcVersionRuntime ? "gnu" : "musl";
  } catch {
    return undefined;
  }
}

function targetNames(): string[] {
  if (process.platform === "darwin") {
    if (process.arch !== "arm64") {
      throw new Error("Intel macOS is not supported; @objsds/client requires Apple Silicon");
    }
    return ["darwin-arm64"];
  }
  if (process.platform === "win32") return [`win32-${process.arch}-msvc`];
  if (process.platform !== "linux") return [`${process.platform}-${process.arch}`];
  const detected = detectLibc();
  return detected === "musl"
    ? [`linux-${process.arch}-musl`, `linux-${process.arch}-gnu`]
    : [`linux-${process.arch}-gnu`, `linux-${process.arch}-musl`];
}

function load(): unknown {
  const explicit = process.env.OBJSDS_NATIVE_BINARY;
  if (explicit) {
    if (!explicit.endsWith(".node")) {
      throw new Error(`OBJSDS_NATIVE_BINARY must point to a .node file, got: ${explicit}`);
    }
    return require(resolve(explicit));
  }

  const errors: unknown[] = [];
  for (const target of targetNames()) {
    for (const candidate of [
      `@objsds/client-${target}`,
      join(packageRoot, `objsds-napi.${target}.node`),
      join(packageRoot, "objsds-napi.node"),
    ]) {
      try {
        if (candidate.endsWith(".node") && !existsSync(candidate)) continue;
        return require(candidate) as unknown;
      } catch (error) {
        if (
          error instanceof Error &&
          "code" in error &&
          (error.code === "MODULE_NOT_FOUND" || error.code === "ERR_DLOPEN_FAILED")
        ) {
          errors.push(error);
          continue;
        }
        throw error;
      }
    }
  }

  throw new Error(
    `Unable to load the objsds native binding for ${process.platform}-${process.arch}. ` +
      "Install the matching platform package or set OBJSDS_NATIVE_BINARY.",
    { cause: errors[0] },
  );
}

export default load();
