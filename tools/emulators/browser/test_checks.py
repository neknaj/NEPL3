"""Failure-path tests; synthetic records here are not browser evidence."""
from dataclasses import replace
from pathlib import Path
import sys
import tempfile
import time
from typing import override
import unittest

from checks import ENGINES, ERRORS, Oracle, observed, oracle, read_json, verify
from processes import snapshot
from run import run, supervise

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue, array, decode, integer, object_value, string

ROOT = Path(__file__).resolve().parents[3]


def record(expected: Oracle) -> dict[str, JsonValue]:
    reverse = {value: key for key, value in ERRORS.items()}
    rows: list[JsonValue] = []
    for case in expected.cases:
        result = case.expected
        if "error" in result:
            value = (1 << 63) | reverse[string(result["error"])]
        elif "offset" in result:
            value = integer(result["offset"])
        else:
            value = (integer(result["line"]) << 32) | integer(result["character"])
        rows.append({"id": case.id, "value": str(value)})
    fixtures: list[JsonValue] = [{"id": f.id, "length": str(f.length), "lines": str(f.lines),
                                 "bytes": [str(b) for b in f.text.encode()]} for f in expected.fixtures]
    return {"version": 1, "engine": "chromium", "engine_version": "synthetic-unit-test",
            "playwright": "1.62.0", "wasm_sha256": "a" * 64, "abi": 1,
            "fixtures": fixtures, "results": rows}


def mutate_row(value: dict[str, JsonValue], collection: str, index: int, key: str, replacement: JsonValue) -> None:
    rows = list(array(value[collection]))
    row = dict(object_value(rows[index]))
    row[key] = replacement
    rows[index] = row
    value[collection] = rows


class Checks(unittest.TestCase):
    expected: Oracle = Oracle((), ())

    @override
    def setUp(self) -> None:
        self.expected = oracle(read_json(ROOT / "conformance/inputs/browser/source-position.json"))

    def check(self, value: JsonValue) -> int:
        return verify(value, "chromium", "a" * 64, self.expected)

    def test_complete_oracle_and_engine_set(self) -> None:
        self.assertEqual(ENGINES, ("chromium", "firefox", "webkit"))
        self.assertEqual(len(self.expected.cases), 173)
        self.assertEqual(self.check(record(self.expected)), 173)
        self.assertEqual(observed("position", str((1 << 63) | 1)), {"error": "Bounds"})

    def test_missing_duplicate_unknown_or_reordered_cases(self) -> None:
        for kind in range(4):
            value = record(self.expected)
            rows = list(array(value["results"]))
            if kind == 0:
                _ = rows.pop()
            elif kind == 1:
                rows[1] = rows[0]
            elif kind == 2:
                rows[0] = {"id": "unknown", "value": "0"}
            else:
                rows.reverse()
            value["results"] = rows
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                _ = self.check(value)

    def test_wrong_value_error_tag_binary_or_engine_fails(self) -> None:
        replacements: tuple[tuple[str, JsonValue], ...] = (("wasm_sha256", "b" * 64), ("engine", "webkit"),
                                                         ("playwright", "0"), ("abi", True), ("version", True))
        for key, replacement in replacements:
            value = record(self.expected)
            value[key] = replacement
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = self.check(value)
        for raw in ("1", str((1 << 63) | 2), str((1 << 63) | 255), "00", "-1", str(1 << 64), True):
            value = record(self.expected)
            mutate_row(value, "results", 0, "value", raw)
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                _ = self.check(value)

    def test_extra_fields_and_boolean_fixture_fail(self) -> None:
        value = record(self.expected)
        value["extra"] = 0
        with self.assertRaises(ValueError):
            _ = self.check(value)
        for collection, index, key, replacement in (("results", 0, "extra", 0), ("fixtures", 1, "id", True)):
            value = record(self.expected)
            mutate_row(value, collection, index, key, replacement)
            with self.assertRaises(ValueError):
                _ = self.check(value)

    def test_changed_expected_coordinate_and_same_width_text_fail(self) -> None:
        raw = record(self.expected)
        changed = replace(self.expected.cases[0], expected={"line": 0, "character": 1})
        self.expected = replace(self.expected, cases=(changed, *self.expected.cases[1:]))
        with self.assertRaises(ValueError):
            _ = self.check(raw)
        self.setUp()
        fixture = replace(self.expected.fixtures[0], text=self.expected.fixtures[0].text.replace("日", "月"))
        self.expected = replace(self.expected, fixtures=(fixture, *self.expected.fixtures[1:]))
        with self.assertRaises(ValueError):
            _ = self.check(raw)

    def test_oracle_rejects_empty_unknown_boolean_and_duplicate(self) -> None:
        for kind in range(4):
            value = dict(object_value(self.expected.representation()))
            if kind == 0:
                value["cases"] = []
            elif kind == 1:
                mutate_row(value, "cases", 0, "operation", "unknown")
            elif kind == 2:
                mutate_row(value, "cases", 0, "encoding", True)
            else:
                mutate_row(value, "cases", 1, "id", self.expected.cases[0].id)
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                _ = oracle(value)

    def test_raw_json_rejects_duplicate_key_trailing_document_and_nonfinite(self) -> None:
        for raw in ('{"a":1,"a":2}', '{} {}', 'NaN', '1e999'):
            with self.assertRaises(ValueError):
                _ = decode(raw, reject_duplicates=True, reject_nonfinite=True)

    def test_supervisor_rejects_failure_timeout_and_stale_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            supervise([sys.executable, "-c", "print('ok')"], root, "success", 10, cwd=root)
            with self.assertRaises(FileExistsError):
                supervise([sys.executable, "-c", "print('stale')"], root, "success", 10)
            for name, script, timeout in (("missing-engine", "raise RuntimeError('missing engine')", 10),
                                          ("trap", "raise RuntimeError('Wasm trap')", 10),
                                          ("timeout", "import time; time.sleep(30)", 0.1)):
                with self.subTest(name=name), self.assertRaises(RuntimeError):
                    supervise([sys.executable, "-c", script], root, name, timeout)
                self.assertTrue((root / f"{name}.stderr").exists())
            self.assertEqual(object_value(read_json(root / "timeout.status.json"))["status"], "timeout")

    def test_timeout_cleans_detached_child(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = ("import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-c','import time,signal; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(60)'],"
                      "start_new_session=True); print(p.pid,flush=True); time.sleep(60)")
            with self.assertRaises(RuntimeError):
                supervise([sys.executable, "-c", script], root, "detached", 1)
            pid = int((root / "detached.stdout").read_text())
            # An orphan may briefly be a zombie until the namespace's init reaps it.
            for _ in range(20):
                path = Path(f"/proc/{pid}/stat")
                if not path.exists() or path.read_text().rsplit(")", 1)[1].split()[0] == "Z":
                    break
                time.sleep(0.05)
            else:
                self.fail(f"detached child {pid} survived")
            self.assertIsInstance(snapshot(), dict)

    def test_foreign_checkout_rejected_and_cwd_respected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, "same checkout"):
                _ = run(root, root / "output", 1)
            supervise([sys.executable, "-c", "import os; print(os.getcwd())"], root, "cwd", 10, cwd=root)
            self.assertEqual((root / "cwd.stdout").read_text().strip(), str(root))


if __name__ == "__main__":
    _ = unittest.main()
