#!/usr/bin/env python3
"""Validate and publish an objsds workspace release."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Sequence

CRATES = ("objsds-store", "objsds-store-memory", "objsds-store-s3", "objsds")
SEMVER = re.compile(
    r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"
)
USER_AGENT = "objsds-release-workflow (https://github.com/gurronen/objsds)"


def run(command: Sequence[str], *, capture: bool = False) -> str:
    result = subprocess.run(command, check=True, text=True, capture_output=capture)
    return result.stdout.strip() if capture else ""


def registry_status(crate: str, version: str) -> int:
    request = urllib.request.Request(
        f"https://crates.io/api/v1/crates/{crate}/{version}",
        headers={"User-Agent": USER_AGENT},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status
    except urllib.error.HTTPError as error:
        return error.code


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
            "workflow version does not match committed package versions: " + ", ".join(mismatches)
        )

    tag = f"v{version}"
    tag_exists = subprocess.run(
        ("git", "rev-parse", "--verify", "--quiet", f"refs/tags/{tag}"),
        stdout=subprocess.DEVNULL,
    ).returncode == 0
    if tag_exists:
        raise RuntimeError(f"tag {tag} already exists")

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
                runner(("cargo", "publish", "--locked", "-p", crate))
                break
            except subprocess.CalledProcessError:
                if attempt == attempts:
                    raise
                print(f"publish of {crate} is not ready; retrying in {delay} seconds")
                time.sleep(delay)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "publish"))
    parser.add_argument("--version", required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    if args.command == "validate":
        validate(args.version)
    else:
        publish(args.version, dry_run=args.dry_run)


if __name__ == "__main__":
    main()
