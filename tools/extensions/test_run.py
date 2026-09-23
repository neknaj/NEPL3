"""Orchestration fault injection; these tests do not claim Rust execution."""
import json
from pathlib import Path, PurePosixPath
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from collections.abc import Sequence
from typing import Literal

from tools.extensions import run
from tools.serialization import json as wire, toml


class ManifestTests(unittest.TestCase):
    def test_paths_are_toml_values_and_other_values_are_preserved(self) -> None:
        source = '''[package]
name = "consumer"
description = "../../../crates/foundation/core"
[workspace]
[dependencies]
# The same text in a comment is not a path field: "../../../crates/foundation/core"
nepl3-core = { path = '../../../crates/foundation/core', features = ["example"], default-features = false }
[dependencies.nepl3-reader]
path = "../../../crates/foundation/reader"
optional = true
'''
        # JSON surrogate pairs are not TOML Unicode scalars. Also cover the
        # quoting/control characters that require TOML-specific serialization.
        for name in ["ASCII", "日本語", "𠮷田", 'quote"apostrophe\'back\\slash', "tab\tline\nreturn\r", "control\x00\x1f\x7f"]:
            with self.subTest(name=name):
                paths = {package: PurePosixPath("/tmp") / name / package
                         for package in ("nepl3-core", "nepl3-reader")}
                output = run.external_manifest(source, paths)
                expected = dict(toml.decode(source))
                dependencies = dict(toml.table(expected["dependencies"]))
                for package, path in paths.items():
                    entry = dict(toml.table(dependencies[package]))
                    entry["path"] = path.as_posix()
                    dependencies[package] = entry
                expected["dependencies"] = dependencies
                self.assertEqual(toml.decode(output), expected)
                self.assertIn('# The same text in a comment is not a path field: "../../../crates/foundation/core"', output)

    def test_missing_or_non_path_dependencies_are_rejected(self) -> None:
        for source in ['[package]\nname="consumer"', '[dependencies]',
                       '[dependencies]\nnepl3-core="1"',
                       '[dependencies]\nnepl3-core={version="1"}',
                       '[dependencies]\nnepl3-core={path=1}']:
            with self.subTest(source=source), self.assertRaises(ValueError):
                _ = run.external_manifest(source, {"nepl3-core": PurePosixPath("/tmp/core")})


type Fault = Literal["duplicate", "wrong-workspace", "changed", "timeout", "failed-test", "metadata-stderr"]


class RunnerFailures(unittest.TestCase):
    def exercise(self, fault: Fault) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "evidence"

            def command(args: Sequence[str], cwd: Path, **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                if "metadata" in args:
                    self.assertTrue((Path(cwd) / "tests/packages.rs").is_file())
                    consumer: dict[str, wire.JsonValue] = {"id": "hello", "name": "external-hello-language",
                                "manifest_path": str(Path(cwd) / "Cargo.toml")}
                    packages: list[wire.JsonValue] = [{"id": name, "name": name, "manifest_path": str(path / "Cargo.toml")}
                                for name, path in run.PACKAGES.items()]
                    if fault == "duplicate":
                        packages.insert(0, {"id": "other-core", "name": "nepl3-core",
                                            "manifest_path": str(Path(cwd) / "other" / "Cargo.toml")})
                    metadata: dict[str, wire.JsonValue] = {"packages": [consumer, *packages], "workspace_members": ["hello"],
                                "resolve": {"root": "hello"}, "workspace_root": str(cwd)}
                    if fault == "wrong-workspace":
                        metadata["workspace_root"] = str(run.ROOT)
                    stderr = b"Downloading crates ...\n" if fault == "metadata-stderr" else b""
                    if kwargs["stderr"] == subprocess.STDOUT:
                        return subprocess.CompletedProcess(args, 0, stderr + json.dumps(metadata).encode())
                    return subprocess.CompletedProcess(args, 0, json.dumps(metadata).encode(), stderr)
                if "test" in args and fault == "timeout":
                    raise subprocess.TimeoutExpired(args, 600, output=b"partial test output")
                if "test" in args and fault == "failed-test":
                    return subprocess.CompletedProcess(args, 1, b"test failure")
                return subprocess.CompletedProcess(args, 0, b"orchestration fixture only")

            fingerprints = [{"source": "before"}, {"source": "after" if fault == "changed" else "before"}]
            with patch("sys.argv", ["run.py", "--output", str(output)]), \
                    patch.object(run, "fingerprint", side_effect=fingerprints), \
                    patch.object(subprocess, "check_output", return_value=b"fixture-commit"), \
                    patch.object(subprocess, "run", side_effect=command):
                if fault == "metadata-stderr":
                    run.main()
                else:
                    with self.assertRaises((RuntimeError, subprocess.TimeoutExpired)):
                        run.main()
            record = wire.object_value(wire.decode((output / "result.json").read_text(encoding="utf-8")))
            self.assertEqual(record["result"], "passed" if fault == "metadata-stderr" else "failed")
            if fault == "metadata-stderr":
                self.assertIn(str(Path("tests") / "packages.rs"), wire.object_value(record["consumer"]))
                self.assertEqual((output / "metadata.json.stderr.log").read_bytes(), b"Downloading crates ...\n")
                _ = wire.decode((output / "metadata.json").read_bytes())
            if fault == "timeout":
                self.assertIn(b"partial test output", (output / "test.log").read_bytes())
                terminal = wire.object_value(wire.array(record["commands"])[-1])
                self.assertEqual(terminal["error"], "TimeoutExpired")
                self.assertIsNone(terminal["exit_code"])
            if fault == "changed":
                self.assertFalse(record["foundation_unchanged"])

    def test_duplicate_package_cannot_hide_behind_same_name(self) -> None:
        self.exercise("duplicate")

    def test_wrong_workspace_is_rejected(self) -> None:
        self.exercise("wrong-workspace")

    def test_changed_source_cannot_publish_passed(self) -> None:
        self.exercise("changed")

    def test_timeout_preserves_partial_output_and_command(self) -> None:
        self.exercise("timeout")

    def test_failed_test_cannot_publish_passed(self) -> None:
        self.exercise("failed-test")

    def test_dependency_download_progress_does_not_corrupt_metadata(self) -> None:
        self.exercise("metadata-stderr")


if __name__ == "__main__":
    _ = unittest.main()
