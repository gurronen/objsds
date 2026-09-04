#!/usr/bin/env python3
"""Resolve, prepare, validate, and publish an objsds workspace release."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Sequence
from pathlib import Path

CRATES = (
    "objsds-store",
    "objsds-store-filesystem",
    "objsds-store-memory",
    "objsds-store-s3",
    "objsds",
    "objsds-queue",
)
SEMVER = re.compile(
    r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"
)
STABLE_SEMVER = re.compile(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)")
USER_AGENT = "objsds-release-workflow (https://github.com/gurronen/objsds)"
NPM_PACKAGE = "@objsds/client"


def run(command: Sequence[str], *, capture: bool = False) -> str:
    result = subprocess.run(command, check=True, text=True, capture_output=capture)
    return result.stdout.strip() if capture else ""


def request_json(url: str, *, attempts: int = 4, delay: int = 2) -> dict | None:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(1, attempts + 1):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            if error.code < 500 or attempt == attempts:
                raise
        except (TimeoutError, urllib.error.URLError):
            if attempt == attempts:
                raise
        print(f"registry request failed; retrying {url} in {delay} seconds", file=sys.stderr)
        time.sleep(delay)
    raise AssertionError("unreachable")


def registry_status(crate: str, version: str) -> int:
    data = request_json(f"https://crates.io/api/v1/crates/{crate}/{version}")
    return 200 if data is not None else 404


def published_versions() -> tuple[str | None, str | None]:
    crate = request_json("https://crates.io/api/v1/crates/objsds")
    npm = request_json("https://registry.npmjs.org/@objsds%2fclient/latest")
    crate_version = crate["crate"].get("max_stable_version") if crate else None
    npm_version = npm.get("version") if npm else None
    return crate_version, npm_version


def workspace_version(root: Path = Path(".")) -> str:
    cargo = (root / "Cargo.toml").read_text()
    workspace = cargo.split("[workspace.package]", 1)[1].split("[", 1)[0]
    match = re.search(r'^version = "([^"]+)"$', workspace, re.MULTILINE)
    if not match:
        raise RuntimeError("workspace.package.version is missing")
    return match.group(1)


def resolve_version(
    override: str | None,
    *,
    current: str | None = None,
    published: tuple[str | None, str | None] | None = None,
) -> str:
    if override:
        if not SEMVER.fullmatch(override):
            raise RuntimeError(f"invalid SemVer version: {override!r}")
        return override

    crate_version, npm_version = published if published is not None else published_versions()
    released = {version for version in (crate_version, npm_version) if version is not None}
    if len(released) > 1:
        raise RuntimeError(
            f"registry versions differ: crates.io={crate_version}, npm={npm_version}; pass --version"
        )
    baseline = next(iter(released), current or workspace_version())
    match = STABLE_SEMVER.fullmatch(baseline)
    if not match:
        raise RuntimeError(f"cannot patch-bump non-stable version: {baseline!r}; pass --version")
    major, minor, patch = (int(part) for part in match.groups())
    return f"{major}.{minor}.{patch + 1}"


def prepare(root: Path, version: str) -> None:
    if not SEMVER.fullmatch(version):
        raise RuntimeError(f"invalid SemVer version: {version!r}")

    cargo_path = root / "Cargo.toml"
    cargo = cargo_path.read_text()
    old = workspace_version(root)
    cargo = cargo.replace(f'version = "{old}"', f'version = "{version}"')
    cargo_path.write_text(cargo)

    npm_root = root / "crates/objsds-napi/npm"
    package_path = npm_root / "package.json"
    package = json.loads(package_path.read_text())
    package["version"] = version
    package_path.write_text(json.dumps(package, indent=2) + "\n")

    lock_path = npm_root / "package-lock.json"
    lock = json.loads(lock_path.read_text())
    lock["version"] = version
    lock["packages"][""]["version"] = version
    lock_path.write_text(json.dumps(lock, indent=2) + "\n")


def validate(version: str, *, status: Callable[[str, str], int] = registry_status) -> None:
    if not SEMVER.fullmatch(version):
        raise RuntimeError(f"invalid SemVer version: {version!r}")

    metadata = json.loads(run(("cargo", "metadata", "--no-deps", "--format-version", "1"), capture=True))
    publishable = [package for package in metadata["packages"] if package["publish"] != []]
    mismatches = [
        f'{package["name"]}={package["version"]}'
        for package in publishable
        if package["version"] != version
    ]
    if mismatches:
        raise RuntimeError(
            "prepared release version does not match package versions: " + ", ".join(mismatches)
        )

    for crate in CRATES:
        code = status(crate, version)
        if code == 200:
            print(f"{crate} {version} is already published; it will be skipped")
        elif code != 404:
            raise RuntimeError(f"crates.io returned HTTP {code} for {crate} {version}")


def publish(
    version: str,
    *,
    status: Callable[[str, str], int] = registry_status,
    runner: Callable[..., str] = run,
    attempts: int = 12,
    delay: int = 10,
    dry_run: bool = False,
) -> None:
    for crate in CRATES:
        for attempt in range(1, attempts + 1):
            code = status(crate, version)
            if code == 200:
                print(f"Skipping {crate} {version}: already published")
                break
            if code != 404:
                raise RuntimeError(f"crates.io returned HTTP {code} for {crate} {version}")
            if dry_run:
                print(f"Would publish {crate} {version}")
                break
            try:
                runner(("cargo", "publish", "--locked", "--allow-dirty", "-p", crate))
                break
            except subprocess.CalledProcessError:
                if attempt == attempts:
                    raise
                print(f"publish of {crate} is not ready; retrying in {delay} seconds")
                time.sleep(delay)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("resolve", "prepare", "validate", "publish"))
    parser.add_argument("--version")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    if args.command == "resolve":
        print(resolve_version(args.version))
        return
    if not args.version:
        parser.error(f"{args.command} requires --version")
    if args.command == "prepare":
        prepare(Path("."), args.version)
    elif args.command == "validate":
        validate(args.version)
    else:
        publish(args.version, dry_run=args.dry_run)


if __name__ == "__main__":
    main()
