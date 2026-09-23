import json
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest
from unittest.mock import patch

import compare
from tools.serialization.json import JsonValue, decode, object_value, string


class Comparison(unittest.TestCase):
    def test_missing_result_and_summary_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            (folder / "interrupted").mkdir()
            with self.assertRaises(OSError):
                compare.check_results(folder)
        with self.assertRaises(ValueError):
            _ = compare.test_count(b"test example ... ok\n")

    def test_aggregate_rejects_malformed_records(self) -> None:
        records: list[JsonValue] = [{"result": "unknown", "passed": 1},
                       {"result": "empty", "passed": 1},
                       {"result": "passed", "passed": 1}, []]
        for record in records:
            with self.subTest(record=record), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                (folder / "invocation").mkdir()
                _ = (folder / "invocation/result.json").write_text(json.dumps(record), encoding="utf-8")
                with self.assertRaises((ValueError, AttributeError)):
                    compare.check_results(folder)

    def test_preflight_failure_records_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            with patch.object(shutil, "which", return_value=None):
                self.assertEqual(compare.compare(folder / "absent.wasm", [], folder), 1)
            self.assertEqual(object_value(decode(next(folder.glob("*/result.json")).read_text(encoding="utf-8")))["result"], "failed")

    def test_artifact_change_during_second_execution_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            wasm = folder / "input.wasm"
            _ = wasm.write_bytes(b"original")
            binary = folder / "wasmtime"
            _ = binary.write_bytes(b"fixture")
            calls = 0

            def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                self.assertEqual(kwargs, {"capture_output": True, "timeout": 180})
                nonlocal calls
                calls += 1
                if calls == 2:
                    _ = wasm.write_bytes(b"changed")
                return subprocess.CompletedProcess(command, 0, b"", b"")

            with patch.object(shutil, "which", return_value=str(binary)), \
                    patch.object(subprocess, "check_output", return_value=b"wasmtime 44.0.1 (fixture)"), \
                    patch.object(subprocess, "run", side_effect=run):
                self.assertEqual(compare.compare(wasm, [], folder), 1)
            self.assertEqual(calls, 2)
            record = object_value(decode(next(folder.glob("*/result.json")).read_text(encoding="utf-8")))
            self.assertIn("changed during", string(record["error"]))

    def test_normalization_changes_only_elapsed_field(self) -> None:
        output = b"test x ... ok\ntest result: ok. 1 passed; 0 failed; finished in 0.04s\n"
        self.assertEqual(compare.normalized(output), output.replace(b"0.04", b"<TIME>"))
        self.assertEqual(compare.normalized(b"arbitrary 0.04s\n"), b"arbitrary 0.04s\n")

    def test_success_empty_and_timeout_keep_distinct_records(self) -> None:
        cases: tuple[tuple[bytes | subprocess.TimeoutExpired, str, int], ...] = (
            (b"test x ... ok\ntest result: ok. 1 passed; 0 failed; finished in 0.04s\n", "passed", 0),
            (b"test result: ok. 0 passed; 0 failed; finished in 0.04s\n", "empty", 0),
            (subprocess.TimeoutExpired("fixture", 180, output=b"partial", stderr=b"stopped"), "failed", 1),
        )
        for output, state, code in cases:
            with self.subTest(state=state), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                wasm, binary, records = root / "input.wasm", root / "wasmtime", root / "records"
                _ = wasm.write_bytes(b"component")
                _ = binary.write_bytes(b"runner")

                def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                    self.assertEqual(kwargs, {"capture_output": True, "timeout": 180})
                    if isinstance(output, subprocess.TimeoutExpired):
                        raise output
                    return subprocess.CompletedProcess(command, 0, output, b"")

                with patch.object(shutil, "which", return_value=str(binary)), \
                        patch.object(subprocess, "check_output", return_value=b"wasmtime 44.0.1 (fixture)"), \
                        patch.object(subprocess, "run", side_effect=run):
                    self.assertEqual(compare.compare(wasm, [], records), code)
                path = next(records.glob("*/result.json"))
                record = object_value(decode(path.read_text(encoding="utf-8")))
                self.assertEqual(record["result"], state)
                if state == "passed":
                    compare.check_results(records)
                    _ = (path.parent / "native.stderr.log").write_bytes(b"tampered")
                    with self.assertRaisesRegex(ValueError, "log identity mismatch"):
                        compare.check_results(records)
                elif state == "empty":
                    with self.assertRaisesRegex(ValueError, "missing or empty"):
                        compare.check_results(records)
                else:
                    self.assertEqual((path.parent / "native.stdout.log").read_bytes(), b"partial")
                    self.assertEqual((path.parent / "native.stderr.log").read_bytes(), b"stopped")
                    self.assertFalse((path.parent / "pulley64.stdout.log").exists())
                    with self.assertRaises(ValueError):
                        compare.check_results(records)


if __name__ == "__main__":
    _ = unittest.main()
