"""Build an identified UF2, or execute that exact payload with an outer deadline."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from elf_to_uf2 import convert

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(args, timeout=600, quiet=False):
    result = subprocess.run(args, cwd=ROOT, capture_output=True, timeout=timeout)
    if not quiet:
        sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    result.check_returncode()
    return result.stdout.decode("utf-8").strip()


def build(folder):
    manifest = ROOT / "conformance/targets/rp2040/Cargo.toml"
    command(["cargo", "build", "--locked", "--release", "--target", "thumbv6m-none-eabi",
             "--manifest-path", str(manifest)])
    elf = manifest.parent / "target/thumbv6m-none-eabi/release/nepl3-conformance-rp2040"
    (folder / "firmware.elf").write_bytes(elf.read_bytes())
    uf2 = folder / "firmware.uf2"
    uf2.write_bytes(convert(elf.read_bytes()))
    paths = command(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], quiet=True).split("\0")
    # Includes untracked harness sources during development, not build outputs.
    inputs = {path: sha(ROOT / path) for path in paths if path and (ROOT / path).is_file()}
    record = {"format": "nepl3-rp2040-build/1", "target": "thumbv6m-none-eabi",
              "commit": command(["git", "rev-parse", "HEAD"]),
              "rustc": command(["rustc", "--version", "--verbose"]),
              "cargo": command(["cargo", "--version"]), "inputs": inputs,
              "firmware_sha256": sha(uf2), "elf_sha256": sha(elf)}
    (folder / "build.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def execute(folder):
    record = {"result": "failed", "outer_timeout_seconds": 30}
    try:
        build_record = json.loads((folder / "build.json").read_text(encoding="utf-8"))
        if (build_record["format"] != "nepl3-rp2040-build/1"
                or build_record["target"] != "thumbv6m-none-eabi"
                or not build_record["rustc"]
                or sha(folder / "firmware.uf2") != build_record["firmware_sha256"]
                or sha(folder / "firmware.elf") != build_record["elf_sha256"]):
            raise ValueError("firmware/build identity mismatch")
        record["build_sha256"] = sha(folder / "build.json")
        result = subprocess.run(["node", str(HERE / "run.mjs"), str(folder / "firmware.uf2"),
                                 str(folder / "emulator.json")], cwd=ROOT,
                                capture_output=True, timeout=30)
        (folder / "stdout.log").write_bytes(result.stdout)
        (folder / "stderr.log").write_bytes(result.stderr)
        result.check_returncode()
        evidence = json.loads((folder / "emulator.json").read_text(encoding="utf-8"))
        if evidence["result"] != "passed" or evidence["firmware_sha256"] != build_record["firmware_sha256"]:
            raise ValueError("execution identity/result mismatch")
        record.update(result="passed", emulator_sha256=sha(folder / "emulator.json"))
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        record["error"] = str(error)
        if isinstance(error, subprocess.TimeoutExpired):
            # subprocess.run kills and reaps the child before raising.
            (folder / "stdout.log").write_bytes(error.stdout or b"")
            (folder / "stderr.log").write_bytes(error.stderr or b"")
    (folder / "execution.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(record))
    return 0 if record["result"] == "passed" else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["build", "execute"])
    parser.add_argument("folder", type=Path)
    args = parser.parse_args()
    folder = args.folder.resolve()
    folder.mkdir(parents=True, exist_ok=True)
    if args.mode == "build":
        build(folder)
    else:
        sys.exit(execute(folder))
