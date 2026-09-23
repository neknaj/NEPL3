"""Build an identified UF2, or execute that exact payload with an outer deadline."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from types import MappingProxyType
from typing import Literal

from elf_to_uf2 import convert

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
from tools.emulators.rp2040js.records import Build, Failed, Identity, Passed, build_identity, check_emulator


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(args: Sequence[str], timeout: int = 600, quiet: bool = False) -> str:
    result = subprocess.run(args, cwd=ROOT, capture_output=True, timeout=timeout)
    if not quiet:
        _ = sys.stdout.buffer.write(result.stdout)
    _ = sys.stderr.buffer.write(result.stderr)
    result.check_returncode()
    return result.stdout.decode("utf-8").strip()


def source_inputs() -> Mapping[str, str]:
    paths = command(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], quiet=True).split("\0")
    return MappingProxyType({path: sha(ROOT / path) for path in paths if path and (ROOT / path).is_file()})


def build(folder: Path) -> None:
    (folder / "build.json").unlink(missing_ok=True)
    inputs = source_inputs()
    manifest = ROOT / "conformance/targets/rp2040/Cargo.toml"
    _ = command(["cargo", "build", "--locked", "--release", "--target", "thumbv6m-none-eabi",
             "--manifest-path", str(manifest), "--target-dir", str(manifest.parent / "target")])
    elf = manifest.parent / "target/thumbv6m-none-eabi/release/nepl3-conformance-rp2040"
    _ = (folder / "firmware.elf").write_bytes(elf.read_bytes())
    uf2 = folder / "firmware.uf2"
    _ = uf2.write_bytes(convert(elf.read_bytes()))
    # Includes untracked harness sources during development, not build outputs.
    if source_inputs() != inputs:
        raise ValueError("source inputs changed while building firmware")
    record = Build(command(["git", "rev-parse", "HEAD"]), command(["rustc", "--version", "--verbose"]),
                   command(["cargo", "--version"]), inputs, Identity(sha(uf2), sha(elf)))
    _ = (folder / "build.json").write_text(json.dumps(record.representation(), indent=2) + "\n", encoding="utf-8")


def execute(folder: Path) -> int:
    build_digest: str | None = None
    outcome: Passed | Failed
    try:
        for name in ["execution.json", "emulator.json", "stdout.log", "stderr.log"]:
            (folder / name).unlink(missing_ok=True)
        identity = build_identity((folder / "build.json").read_text(encoding="utf-8"))
        if (sha(folder / "firmware.uf2") != identity.firmware or sha(folder / "firmware.elf") != identity.elf):
            raise ValueError("firmware/build identity mismatch")
        build_digest = sha(folder / "build.json")
        # An exit-zero child must not authenticate evidence left by a prior run.
        with tempfile.TemporaryDirectory(prefix="nepl3-rp2040-") as temporary:
            fresh = Path(temporary) / "emulator.json"
            result = subprocess.run(["node", str(HERE / "run.mjs"), str(folder / "firmware.uf2"),
                                     str(fresh)], cwd=ROOT, capture_output=True, timeout=30)
            _ = (folder / "stdout.log").write_bytes(result.stdout)
            _ = (folder / "stderr.log").write_bytes(result.stderr)
            if fresh.is_file():
                _ = (folder / "emulator.json").write_bytes(fresh.read_bytes())
            result.check_returncode()
            check_emulator(fresh.read_text(encoding="utf-8"), identity)
        outcome = Passed(build_digest, sha(folder / "emulator.json"))
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        outcome = Failed(str(error), build_digest)
        if isinstance(error, subprocess.TimeoutExpired):
            # subprocess.run kills and reaps the child before raising.
            _ = (folder / "stdout.log").write_bytes(error.stdout or b"")
            _ = (folder / "stderr.log").write_bytes(error.stderr or b"")
    record = outcome.representation()
    _ = (folder / "execution.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(record))
    return 0 if isinstance(outcome, Passed) else 1


class Arguments(argparse.Namespace):
    mode: Literal["build", "execute"] = "build"
    folder: Path = Path()


def main() -> int:
    parser = argparse.ArgumentParser()
    _ = parser.add_argument("mode", choices=["build", "execute"])
    _ = parser.add_argument("folder", type=Path)
    args = parser.parse_args(namespace=Arguments())
    folder = args.folder.resolve()
    folder.mkdir(parents=True, exist_ok=True)
    if args.mode == "build":
        build(folder)
        return 0
    else:
        return execute(folder)


if __name__ == "__main__":
    sys.exit(main())
