"""Command failures retain binary output and explicit terminal outcomes."""

import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools.extensions.execution import Execution, Exited, Interrupted, Record, Scope


class ExecutionTests(unittest.TestCase):
    def test_temporary_target_overrides_shared_cache_without_mutating_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            execution = Execution(directory)
            execution.target_directory = directory / "isolated-target"
            def command(_arguments: object, *, env: dict[str, str], **_kwargs: object) -> subprocess.CompletedProcess[bytes]:
                self.assertEqual(env["CARGO_TARGET_DIR"], str(execution.target_directory))
                self.assertEqual(env["CARGO_BUILD_BUILD_DIR"], str(directory / "isolated-target/intermediate"))
                return subprocess.CompletedProcess(["cargo"], 0, b"ok", b"")
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": "/shared-cache", "CARGO_BUILD_BUILD_DIR": "/shared-intermediate"}), patch.object(subprocess, "run", side_effect=command):
                _ = execution.run(["cargo", "check"], directory, "check.log")
                self.assertEqual(os.environ["CARGO_TARGET_DIR"], "/shared-cache")
                self.assertEqual(os.environ["CARGO_BUILD_BUILD_DIR"], "/shared-intermediate")

    def test_nonzero_exit_keeps_both_output_streams(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            execution = Execution(directory)
            reply = subprocess.CompletedProcess(["fixture"], 7, b"output", b"diagnostic")
            with patch.object(subprocess, "run", return_value=reply), self.assertRaisesRegex(RuntimeError, "exit 7"):
                _ = execution.run(["fixture"], directory, "test.log")
            self.assertEqual((directory / "test.log").read_bytes(), b"output")
            self.assertEqual((directory / "test.log.stderr.log").read_bytes(), b"diagnostic")
            self.assertEqual(execution.commands[0].outcome, Exited(7))
            self.assertEqual(execution.commands[0].representation(), {
                "command": ["fixture"], "exit_code": 7, "log": "test.log",
                "sha256": hashlib.sha256(b"output").hexdigest(),
                "stderr": "test.log.stderr.log", "stderr_sha256": hashlib.sha256(b"diagnostic").hexdigest(),
            })

    def test_timeout_and_os_failure_keep_distinct_outcomes(self) -> None:
        failures = (subprocess.TimeoutExpired(["fixture"], 600, output=b"partial", stderr=b"warning"),
                    FileNotFoundError("missing fixture"))
        for failure in failures:
            with self.subTest(failure=type(failure).__name__), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                execution = Execution(directory)
                with patch.object(subprocess, "run", side_effect=failure), self.assertRaises(type(failure)):
                    _ = execution.run(["fixture"], directory, "test.log")
                self.assertEqual(execution.commands[0].outcome, Interrupted(type(failure).__name__))
                self.assertIsNone(execution.commands[0].representation()["exit_code"])
                if isinstance(failure, subprocess.TimeoutExpired):
                    self.assertTrue((directory / "test.log").read_bytes().startswith(b"partial\n"))
                    self.assertEqual((directory / "test.log.stderr.log").read_bytes(), b"warning")
                else:
                    self.assertEqual((directory / "test.log.stderr.log").read_bytes(), b"")

    def test_pass_requires_completion_and_unchanged_source(self) -> None:
        for completed, unchanged in ((False, False), (False, True), (True, False), (True, True)):
            record = Record(Scope.DIRECT, "commit", {}, (), completed, unchanged, None).representation()
            self.assertEqual(record["result"], "passed" if completed and unchanged else "failed")
            self.assertNotIn("consumer", record)
            self.assertEqual(record["foundation_unchanged"], unchanged)


if __name__ == "__main__":
    _ = unittest.main()
