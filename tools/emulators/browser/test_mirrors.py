"""Checks for the fixed runner mirror-priority edit, not network speed."""
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest

from tools.emulators.browser.mirrors import (
    MirrorConfigurationError,
    PRIMARY_MIRRORS,
    RUNNER_MIRRORS,
    prefer_primary,
    update,
)


@unittest.skipUnless(sys.platform == "linux", "Ubuntu runner-only file operation")
class MirrorTests(unittest.TestCase):
    def test_only_existing_priorities_change(self) -> None:
        result = prefer_primary(RUNNER_MIRRORS)
        self.assertEqual(
            result.splitlines(),
            [
                b"http://azure.archive.ubuntu.com/ubuntu/\tpriority:2",
                b"https://archive.ubuntu.com/ubuntu/\tpriority:1",
                b"https://security.ubuntu.com/ubuntu/\tpriority:3",
            ],
        )
        self.assertEqual(len(result), len(RUNNER_MIRRORS))
        self.assertEqual(
            [line.split(b"\t")[0] for line in result.splitlines()],
            [line.split(b"\t")[0] for line in RUNNER_MIRRORS.splitlines()],
        )

    def test_idempotent(self) -> None:
        self.assertEqual(prefer_primary(PRIMARY_MIRRORS), PRIMARY_MIRRORS)

    def test_unknown_lists_rejected_without_writes(self) -> None:
        invalid = [
            b"",
            RUNNER_MIRRORS + b"https://untrusted.invalid/\tpriority:0\n",
            RUNNER_MIRRORS.replace(b"archive.ubuntu.com/", b"archive.ubuntu.com.evil/"),
            RUNNER_MIRRORS.replace(b"priority:1", b"priority:9"),
            RUNNER_MIRRORS[:-1],
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "mirrors.txt"
            for value in invalid:
                with self.subTest(value=value):
                    _ = path.write_bytes(value)
                    with self.assertRaises(MirrorConfigurationError):
                        _ = update(path)
                    self.assertEqual(path.read_bytes(), value)

    def test_update_preserves_mode_and_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "mirrors.txt"
            _ = path.write_bytes(RUNNER_MIRRORS)
            path.chmod(0o640)
            before = path.stat()
            self.assertTrue(update(path))
            self.assertEqual(path.read_bytes(), PRIMARY_MIRRORS)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), stat.S_IMODE(before.st_mode))
            self.assertEqual(path.stat().st_ino, before.st_ino)
            self.assertFalse(update(path))

    def test_symlink_rejected_without_target_change(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target"
            link = Path(directory) / "link"
            _ = target.write_bytes(RUNNER_MIRRORS)
            link.symlink_to(target)
            with self.assertRaises(OSError):
                _ = update(link)
            self.assertEqual(target.read_bytes(), RUNNER_MIRRORS)

    def test_missing_file_not_created(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "missing"
            with self.assertRaises(OSError):
                _ = update(path)
            self.assertFalse(path.exists())

    def test_nonregular_file_rejected_without_blocking(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pipe"
            os.mkfifo(path)
            with self.assertRaises(MirrorConfigurationError):
                _ = update(path)


if __name__ == "__main__":
    _ = unittest.main()
