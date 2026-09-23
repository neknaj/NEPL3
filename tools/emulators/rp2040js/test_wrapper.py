import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from collections.abc import Callable, Sequence

import firmware
from tools.serialization.json import JsonValue, decode, object_value


class Wrapper(unittest.TestCase):
    def test_build_does_not_package_stale_default_output_with_target_override(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "conformance/targets/rp2040/target"
            relative = Path("thumbv6m-none-eabi/release/nepl3-conformance-rp2040")
            old = target / relative
            old.parent.mkdir(parents=True)
            _ = old.write_bytes(b"old firmware")
            folder = root / "bundle"
            folder.mkdir()

            def command(args: Sequence[str], **_kwargs: object) -> str:
                if args[:2] == ["cargo", "build"]:
                    selected = Path(args[args.index("--target-dir") + 1]) if "--target-dir" in args else Path(os.environ["CARGO_TARGET_DIR"])
                    output = selected / relative
                    output.parent.mkdir(parents=True, exist_ok=True)
                    _ = output.write_bytes(b"new firmware")
                return "fixture"

            convert: Callable[[bytes], bytes] = lambda value: b"UF2" + value
            with patch.object(firmware, "ROOT", root), \
                 patch.object(firmware, "command", side_effect=command), \
                 patch.object(firmware, "source_inputs", return_value={}), \
                 patch.object(firmware, "convert", side_effect=convert), \
                 patch.dict(os.environ, {"CARGO_TARGET_DIR": str(root / "other-target")}):
                firmware.build(folder)
            self.assertEqual((folder / "firmware.elf").read_bytes(), b"new firmware")
            self.assertEqual((folder / "firmware.uf2").read_bytes(), b"UF2new firmware")

    def fixture(self, folder: Path) -> dict[str, JsonValue]:
        for name in ["firmware.elf", "firmware.uf2"]:
            _ = (folder / name).write_bytes(name.encode())
        record: dict[str, JsonValue] = {"format": "nepl3-rp2040-build/1", "target": "thumbv6m-none-eabi",
                  "rustc": "fixture", "firmware_sha256": firmware.sha(folder / "firmware.uf2"),
                  "elf_sha256": firmware.sha(folder / "firmware.elf")}
        _ = (folder / "build.json").write_text(json.dumps(record), encoding="utf-8")
        _ = (folder / "emulator.json").write_text(json.dumps({"result": "passed",
            "firmware_sha256": record["firmware_sha256"]}), encoding="utf-8")
        return record

    def test_exit_zero_cannot_reuse_old_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            _ = self.fixture(folder)
            with patch.object(subprocess, "run", return_value=subprocess.CompletedProcess([], 0, b"", b"")):
                self.assertEqual(firmware.execute(folder), 1)
            self.assertFalse((folder / "emulator.json").exists())

    def test_fresh_result_required_and_hash_bound(self) -> None:
        for correct in [True, False]:
            with self.subTest(correct=correct), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                record = self.fixture(folder)

                def run(command: Sequence[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                    self.assertEqual(kwargs["timeout"], 30)
                    self.assertFalse(Path(command[-1]).exists())
                    _ = Path(command[-1]).write_text(json.dumps({"result": "passed", "firmware_sha256":
                        record["firmware_sha256"] if correct else "wrong"}), encoding="utf-8")
                    return subprocess.CompletedProcess(command, 0, b"UART", b"")

                with patch.object(subprocess, "run", side_effect=run):
                    self.assertEqual(firmware.execute(folder), 0 if correct else 1)

    def test_timeout_nonzero_and_corrupted_input_fail(self) -> None:
        for failure in ["timeout", "nonzero", "corrupt"]:
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                _ = self.fixture(folder)
                if failure == "corrupt":
                    _ = (folder / "firmware.uf2").write_bytes(b"changed")
                with patch.object(subprocess, "run") as run:
                    if failure == "timeout":
                        run.side_effect = subprocess.TimeoutExpired("node", 30, output=b"partial")
                    else:
                        run.return_value = subprocess.CompletedProcess([], 1, b"", b"failed")
                    self.assertEqual(firmware.execute(folder), 1)
                    if failure == "corrupt":
                        run.assert_not_called()

    def test_invalid_json_shapes_replace_prior_success(self) -> None:
        shapes: tuple[JsonValue, ...] = ([], None, "invalid", 1)
        for shape in shapes:
            with self.subTest(shape=shape), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                _ = self.fixture(folder)
                _ = (folder / "build.json").write_text(json.dumps(shape), encoding="utf-8")
                _ = (folder / "execution.json").write_text('{"result":"passed"}', encoding="utf-8")
                self.assertEqual(firmware.execute(folder), 1)
                self.assertEqual(object_value(decode((folder / "execution.json").read_text(encoding="utf-8")))["result"], "failed")
                self.assertFalse((folder / "emulator.json").exists())


if __name__ == "__main__":
    _ = unittest.main()
