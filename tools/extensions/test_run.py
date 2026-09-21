"""Orchestration fault injection; these tests do not claim Rust execution."""
import json
from pathlib import Path, PurePosixPath
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from tools.extensions import run


class ManifestTests(unittest.TestCase):
    def test_paths_are_toml_values_and_other_values_are_preserved(self):
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
                expected = tomllib.loads(source)
                for package, path in paths.items():
                    expected["dependencies"][package]["path"] = path.as_posix()
                self.assertEqual(tomllib.loads(output), expected)
                self.assertIn('# The same text in a comment is not a path field: "../../../crates/foundation/core"', output)

    def test_missing_or_non_path_dependencies_are_rejected(self):
        for source in ['[package]\nname="consumer"', '[dependencies]',
                       '[dependencies]\nnepl3-core="1"',
                       '[dependencies]\nnepl3-core={version="1"}',
                       '[dependencies]\nnepl3-core={path=1}']:
            with self.subTest(source=source), self.assertRaises(ValueError):
                run.external_manifest(source, {"nepl3-core": PurePosixPath("/tmp/core")})


class RunnerFailures(unittest.TestCase):
    def exercise(self, fault):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "evidence"

            def command(args, cwd, **kwargs):
                if "metadata" in args:
                    consumer = {"id": "hello", "name": "external-hello-language",
                                "manifest_path": str(Path(cwd) / "Cargo.toml")}
                    packages = [{"id": name, "name": name, "manifest_path": str(path / "Cargo.toml")}
                                for name, path in run.PACKAGES.items()]
                    if fault == "duplicate":
                        packages.insert(0, {"id": "other-core", "name": "nepl3-core",
                                            "manifest_path": str(Path(cwd) / "other" / "Cargo.toml")})
                    metadata = {"packages": [consumer] + packages, "workspace_members": ["hello"],
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
                    patch.object(run.subprocess, "check_output", return_value=b"fixture-commit"), \
                    patch.object(run.subprocess, "run", side_effect=command):
                if fault == "metadata-stderr":
                    run.main()
                else:
                    with self.assertRaises((RuntimeError, subprocess.TimeoutExpired)):
                        run.main()
            record = json.loads((output / "result.json").read_text(encoding="utf-8"))
            self.assertEqual(record["result"], "passed" if fault == "metadata-stderr" else "failed")
            if fault == "metadata-stderr":
                self.assertEqual((output / "metadata.json.stderr.log").read_bytes(), b"Downloading crates ...\n")
                json.loads((output / "metadata.json").read_bytes())
            if fault == "timeout":
                self.assertIn(b"partial test output", (output / "test.log").read_bytes())
                self.assertEqual(record["commands"][-1]["error"], "TimeoutExpired")
                self.assertIsNone(record["commands"][-1]["exit_code"])
            if fault == "changed":
                self.assertFalse(record["foundation_unchanged"])

    def test_duplicate_package_cannot_hide_behind_same_name(self):
        self.exercise("duplicate")

    def test_wrong_workspace_is_rejected(self):
        self.exercise("wrong-workspace")

    def test_changed_source_cannot_publish_passed(self):
        self.exercise("changed")

    def test_timeout_preserves_partial_output_and_command(self):
        self.exercise("timeout")

    def test_failed_test_cannot_publish_passed(self):
        self.exercise("failed-test")

    def test_dependency_download_progress_does_not_corrupt_metadata(self):
        self.exercise("metadata-stderr")


if __name__ == "__main__":
    unittest.main()
