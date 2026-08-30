#!/usr/bin/env python3

import subprocess
import unittest
from unittest.mock import patch

import release


class ReleaseTests(unittest.TestCase):
    def test_crates_are_in_dependency_order(self) -> None:
        self.assertEqual(
            release.CRATES,
            ("objsds-store", "objsds-store-memory", "objsds-store-s3", "objsds"),
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
            ["objsds-store-memory", "objsds-store-s3", "objsds"],
        )

    def test_publish_rechecks_registry_after_failure(self) -> None:
        statuses = iter((404, 200, 200, 200, 200))
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

    def test_dry_run_does_not_publish(self) -> None:
        runner = unittest.mock.Mock()
        release.publish("1.2.3", status=lambda *_: 404, runner=runner, dry_run=True)
        runner.assert_not_called()


if __name__ == "__main__":
    unittest.main()
