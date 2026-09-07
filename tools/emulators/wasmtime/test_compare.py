import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import compare


class Comparison(unittest.TestCase):
    def test_missing_result_and_summary_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            (folder / "interrupted").mkdir()
            with self.assertRaises(OSError):
                compare.check_results(folder)
        with self.assertRaises(ValueError):
            compare.test_count(b"test example ... ok\n")

    def test_aggregate_rejects_malformed_records(self):
        for record in [{"result": "unknown", "passed": 1},
                       {"result": "empty", "passed": 1},
                       {"result": "passed", "passed": 1}, []]:
            with self.subTest(record=record), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                (folder / "invocation").mkdir()
                (folder / "invocation/result.json").write_text(json.dumps(record), encoding="utf-8")
                with self.assertRaises((ValueError, AttributeError)):
                    compare.check_results(folder)

    def test_preflight_failure_records_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            with patch.object(compare.shutil, "which", return_value=None):
                self.assertEqual(compare.compare(folder / "absent.wasm", [], folder), 1)
            self.assertEqual(json.loads(next(folder.glob("*/result.json")).read_text(encoding="utf-8"))["result"], "failed")

    def test_artifact_change_during_second_execution_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            wasm = folder / "input.wasm"
            wasm.write_bytes(b"original")
            binary = folder / "wasmtime"
            binary.write_bytes(b"fixture")
            calls = 0

            def run(command, **kwargs):
                nonlocal calls
                calls += 1
                if calls == 2:
                    wasm.write_bytes(b"changed")
                return subprocess.CompletedProcess(command, 0, b"", b"")

            with patch.object(compare.shutil, "which", return_value=str(binary)), \
                    patch.object(compare.subprocess, "check_output", return_value=b"wasmtime 44.0.1 (fixture)"), \
                    patch.object(compare.subprocess, "run", side_effect=run):
                self.assertEqual(compare.compare(wasm, [], folder), 1)
            self.assertEqual(calls, 2)
            record = json.loads(next(folder.glob("*/result.json")).read_text(encoding="utf-8"))
            self.assertIn("changed during", record["error"])

    def test_normalization_changes_only_elapsed_field(self):
        output = b"test x ... ok\ntest result: ok. 1 passed; 0 failed; finished in 0.04s\n"
        self.assertEqual(compare.normalized(output), output.replace(b"0.04", b"<TIME>"))
        self.assertEqual(compare.normalized(b"arbitrary 0.04s\n"), b"arbitrary 0.04s\n")


if __name__ == "__main__":
    unittest.main()
