"""Run the isolated native allocation probe without changing workspace lint policy."""

import argparse
from collections.abc import Sequence
from dataclasses import dataclass
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from tools.serialization import json, toml


def execute(arguments: Sequence[str], root: Path) -> str:
    result = subprocess.run(
        arguments, cwd=root, text=True, encoding="utf-8", capture_output=True
    )
    if result.stderr:
        print(result.stderr, end="")
    if result.returncode:
        if result.stdout:
            print(result.stdout, end="")
        raise RuntimeError(f"command failed ({result.returncode}): {arguments!r}")
    return result.stdout


@dataclass(frozen=True, slots=True)
class Artifact:
    name: str
    kinds: tuple[str, ...]
    source: Path
    filenames: tuple[str, ...]


def artifact(source: str) -> Artifact | None:
    message = json.object_value(json.decode(source))
    if message.get("reason") != "compiler-artifact":
        return None
    target = json.object_value(message["target"])
    return Artifact(json.string(target["name"]),
                    tuple(json.string(kind) for kind in json.array(target["kind"])),
                    Path(json.string(target["src_path"])),
                    tuple(json.string(name) for name in json.array(message["filenames"])))


class Arguments(argparse.Namespace):
    repository: Path = ROOT


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument(
        "--repository", type=Path,
        default=Path(__file__).resolve().parents[3],
        help="checkout whose production core is measured (default: this repository)",
    )
    args = parser.parse_args(namespace=Arguments())
    root = args.repository.resolve(strict=True)
    probe = Path(__file__).with_name("probe.rs").resolve(strict=True)
    channel = toml.string(toml.table(toml.decode((root / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"])["channel"])
    if not channel or channel in ("stable", "beta", "nightly"):
        raise RuntimeError("the measured checkout must pin a concrete Rust toolchain")
    rustc = ["rustup", "run", channel, "rustc"]
    cargo = ["rustup", "run", channel, "cargo"]
    version = execute([*rustc, "-vV"], root)
    print(version, end="")
    host = next((line[6:] for line in version.splitlines() if line.startswith("host: ")), None)
    if not host:
        raise RuntimeError("rustc did not report its native host target")
    _ = execute(["rustup", "run", channel, "rustfmt", "--check", str(probe)], root)
    output = execute([
        *cargo, "build", "--locked", "--target", host, "-p", "nepl3-core",
        "--message-format=json",
    ], root)
    candidates: list[Path] = []
    dependency_dirs: set[str] = set()
    for line in output.splitlines():
        message = artifact(line)
        if message is None:
            continue
        dependency_dirs.update(
            str(Path(name).resolve(strict=True).parent)
            for name in message.filenames if name.endswith((".rlib", ".rmeta"))
        )
        if message.name != "nepl3_core" or "lib" not in message.kinds:
            continue
        if message.source.resolve() != root / "crates/foundation/core/src/lib.rs":
            continue
        candidates.extend(Path(name) for name in message.filenames if name.endswith(".rlib"))
    if len(candidates) != 1:
        raise RuntimeError(f"expected one Cargo-selected core rlib, found {candidates!r}")
    library = candidates[0].resolve(strict=True)
    print(f"measured core artifact: {library}")
    with tempfile.TemporaryDirectory(prefix="nepl3-allocation-") as temporary:
        binary = Path(temporary) / ("probe.exe" if os.name == "nt" else "probe")
        search_paths = [part for directory in sorted(dependency_dirs)
                        for part in ("-L", f"dependency={directory}")]
        _ = execute([
            *rustc, "--edition=2024", "--target", host, "--crate-name", "allocation_probe",
            "-D", "warnings", "-D", "unsafe-op-in-unsafe-fn", "-C", "opt-level=0",
            str(probe), "--extern", f"nepl3_core={library}",
            *search_paths, "-o", str(binary),
        ], root)
        print(execute([str(binary)], root), end="")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        raise SystemExit(str(error)) from error
