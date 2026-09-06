#!/usr/bin/env python3

import json
import subprocess
from io import BytesIO
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import release


class ReleaseTests(unittest.TestCase):
    def test_crates_are_in_dependency_order(self) -> None:
        self.assertEqual(
            release.CRATES,
            (
                "objsds-store",
                "objsds-store-filesystem",
                "objsds-store-memory",
                "objsds-store-s3",
                "objsds",
                "objsds-queue",
            ),
        )

    def test_publish_skips_existing_crates(self) -> None:
        commands = []

        def status(crate: str, _version: str) -> int:
            return 200 if crate == "objsds-store" else 404

        release.publish(
            "1.2.3",
            status=status,
            runner=lambda command: commands.append(tuple(command)) or "",
            delay=0,
        )

        self.assertEqual(
            [command[-1] for command in commands],
            [
                "objsds-store-filesystem",
                "objsds-store-memory",
                "objsds-store-s3",
                "objsds",
                "objsds-queue",
            ],
        )

    def test_publish_rechecks_registry_after_failure(self) -> None:
        statuses = iter((404, 200, 200, 200, 200, 200, 200))
        attempts = []

        def fail_once(command) -> str:
            attempts.append(tuple(command))
            raise subprocess.CalledProcessError(1, command)

        release.publish(
            "1.2.3",
            status=lambda _crate, _version: next(statuses),
            runner=fail_once,
            attempts=2,
            delay=0,
        )

        self.assertEqual(len(attempts), 1)

    def test_validate_rejects_invalid_semver_before_commands(self) -> None:
        with patch.object(release, "run") as runner:
            with self.assertRaisesRegex(RuntimeError, "invalid SemVer"):
                release.validate("01.2.3")
        runner.assert_not_called()

    def test_validate_rejects_existing_git_tag(self) -> None:
        with patch.object(release, "run") as runner:
            with self.assertRaisesRegex(RuntimeError, "tag v1.2.3 already exists"):
                release.validate("1.2.3", status=lambda *_: 404, tag_exists=lambda _: True)
        runner.assert_not_called()

    def test_registry_requests_retry_transient_timeouts(self) -> None:
        with patch.object(
            release.urllib.request,
            "urlopen",
            side_effect=(TimeoutError(), BytesIO(b'{"version":"1.2.3"}')),
        ) as request:
            self.assertEqual(
                release.request_json("https://registry.example/package", attempts=2, delay=0),
                {"version": "1.2.3"},
            )
        self.assertEqual(request.call_count, 2)

    def test_resolve_defaults_to_patch_after_matching_registries(self) -> None:
        self.assertEqual(
            release.resolve_version(None, current="0.1.0", published=("1.2.3", "1.2.3")),
            "1.2.4",
        )

    def test_resolve_uses_local_baseline_for_first_release(self) -> None:
        self.assertEqual(
            release.resolve_version(None, current="0.1.0", published=(None, None)),
            "0.1.1",
        )

    def test_resolve_rejects_registry_drift(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "registry versions differ"):
            release.resolve_version(None, current="0.1.0", published=("1.2.3", "1.2.2"))

    def test_prepare_versions_cargo_and_npm_staging_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            npm = root / "crates/objsds-napi/npm"
            npm.mkdir(parents=True)
            (root / "Cargo.toml").write_text(
                '[workspace.package]\nversion = "0.1.0"\n\n[workspace.dependencies]\n'
                + "\n".join(
                    f'{crate} = {{ version = "0.1.0", path = "crates/{crate}" }}'
                    for crate in release.WORKSPACE_DEPENDENCIES
                )
                + '\nunrelated = { version = "0.1.0" }\n'
            )
            (npm / "package.json").write_text(json.dumps({"version": "0.1.0"}))
            (npm / "package-lock.json").write_text(
                json.dumps({"version": "0.1.0", "packages": {"": {"version": "0.1.0"}}})
            )

            release.prepare(root, "0.1.1")

            cargo = (root / "Cargo.toml").read_text()
            self.assertIn('version = "0.1.1"', cargo)
            self.assertIn('objsds = { version = "0.1.1"', cargo)
            self.assertIn('unrelated = { version = "0.1.0" }', cargo)
            self.assertNotIn('[workspace.package]\nversion = "0.1.0"', cargo)
            self.assertEqual(json.loads((npm / "package.json").read_text())["version"], "0.1.1")
            self.assertEqual(json.loads((npm / "package-lock.json").read_text())["version"], "0.1.1")

    def test_dry_run_does_not_publish(self) -> None:
        runner = unittest.mock.Mock()
        release.publish("1.2.3", status=lambda *_: 404, runner=runner, dry_run=True)
        runner.assert_not_called()


if __name__ == "__main__":
    unittest.main()
