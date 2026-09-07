import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import firmware


class Wrapper(unittest.TestCase):
    def fixture(self, folder):
        for name in ["firmware.elf", "firmware.uf2"]:
            (folder / name).write_bytes(name.encode())
        record = {"format": "nepl3-rp2040-build/1", "target": "thumbv6m-none-eabi",
                  "rustc": "fixture", "firmware_sha256": firmware.sha(folder / "firmware.uf2"),
                  "elf_sha256": firmware.sha(folder / "firmware.elf")}
        (folder / "build.json").write_text(json.dumps(record), encoding="utf-8")
        (folder / "emulator.json").write_text(json.dumps({"result": "passed",
            "firmware_sha256": record["firmware_sha256"]}), encoding="utf-8")
        return record

    def test_exit_zero_cannot_reuse_old_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            self.fixture(folder)
            with patch.object(firmware.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, b"", b"")):
                self.assertEqual(firmware.execute(folder), 1)
            self.assertFalse((folder / "emulator.json").exists())

    def test_fresh_result_required_and_hash_bound(self):
        for correct in [True, False]:
            with self.subTest(correct=correct), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                record = self.fixture(folder)

                def run(command, **kwargs):
                    self.assertEqual(kwargs["timeout"], 30)
                    self.assertFalse(Path(command[-1]).exists())
                    Path(command[-1]).write_text(json.dumps({"result": "passed", "firmware_sha256":
                        record["firmware_sha256"] if correct else "wrong"}), encoding="utf-8")
                    return subprocess.CompletedProcess(command, 0, b"UART", b"")

                with patch.object(firmware.subprocess, "run", side_effect=run):
                    self.assertEqual(firmware.execute(folder), 0 if correct else 1)

    def test_timeout_nonzero_and_corrupted_input_fail(self):
        for failure in ["timeout", "nonzero", "corrupt"]:
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                self.fixture(folder)
                if failure == "corrupt":
                    (folder / "firmware.uf2").write_bytes(b"changed")
                with patch.object(firmware.subprocess, "run") as run:
                    if failure == "timeout":
                        run.side_effect = subprocess.TimeoutExpired("node", 30, output=b"partial")
                    else:
                        run.return_value = subprocess.CompletedProcess([], 1, b"", b"failed")
                    self.assertEqual(firmware.execute(folder), 1)
                    if failure == "corrupt":
                        run.assert_not_called()

    def test_invalid_json_shapes_replace_prior_success(self):
        for shape in [[], None, "invalid", 1]:
            with self.subTest(shape=shape), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                self.fixture(folder)
                (folder / "build.json").write_text(json.dumps(shape), encoding="utf-8")
                (folder / "execution.json").write_text('{"result":"passed"}', encoding="utf-8")
                self.assertEqual(firmware.execute(folder), 1)
                self.assertEqual(json.loads((folder / "execution.json").read_text(encoding="utf-8"))["result"], "failed")
                self.assertFalse((folder / "emulator.json").exists())


if __name__ == "__main__":
    unittest.main()
