"""Run the isolated native allocation probe without changing workspace lint policy."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib


def execute(arguments, root):
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository", type=Path,
        default=Path(__file__).resolve().parents[3],
        help="checkout whose production core is measured (default: this repository)",
    )
    args = parser.parse_args()
    root = args.repository.resolve(strict=True)
    probe = Path(__file__).with_name("probe.rs").resolve(strict=True)
    with (root / "rust-toolchain.toml").open("rb") as source:
        channel = tomllib.load(source)["toolchain"]["channel"]
    if not isinstance(channel, str) or not channel or channel in ("stable", "beta", "nightly"):
        raise RuntimeError("the measured checkout must pin a concrete Rust toolchain")
    rustc = ["rustup", "run", channel, "rustc"]
    cargo = ["rustup", "run", channel, "cargo"]
    version = execute([*rustc, "-vV"], root)
    print(version, end="")
    host = next((line[6:] for line in version.splitlines() if line.startswith("host: ")), None)
    if not host:
        raise RuntimeError("rustc did not report its native host target")
    execute(["rustup", "run", channel, "rustfmt", "--check", str(probe)], root)
    output = execute([
        *cargo, "build", "--locked", "--target", host, "-p", "nepl3-core",
        "--message-format=json",
    ], root)
    candidates = []
    dependency_dirs = set()
    for line in output.splitlines():
        message = json.loads(line)
        if message.get("reason") != "compiler-artifact":
            continue
        dependency_dirs.update(
            str(Path(name).resolve(strict=True).parent)
            for name in message["filenames"] if name.endswith((".rlib", ".rmeta"))
        )
        target = message.get("target", {})
        if target.get("name") != "nepl3_core" or "lib" not in target.get("kind", []):
            continue
        if Path(target["src_path"]).resolve() != root / "crates/foundation/core/src/lib.rs":
            continue
        candidates.extend(Path(name) for name in message["filenames"] if name.endswith(".rlib"))
    if len(candidates) != 1:
        raise RuntimeError(f"expected one Cargo-selected core rlib, found {candidates!r}")
    library = candidates[0].resolve(strict=True)
    print(f"measured core artifact: {library}")
    with tempfile.TemporaryDirectory(prefix="nepl3-allocation-") as temporary:
        binary = Path(temporary) / ("probe.exe" if os.name == "nt" else "probe")
        search_paths = [part for directory in sorted(dependency_dirs)
                        for part in ("-L", f"dependency={directory}")]
        execute([
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
