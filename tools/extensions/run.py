"""Run the public-API consumer outside the repository, without private test imports."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Mapping
from types import MappingProxyType

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.extensions.manifests import external_manifest as external_manifest
from tools.extensions import cargo as cargo_input
from tools.extensions.execution import Execution, Record, Scope

from tools.extensions.distribution import export

FOUNDATION = ROOT / "crates" / "foundation"
PACKAGES = {f"nepl3-{name}": FOUNDATION / name for name in ("core", "reader", "engine", "wire")}


def fingerprint() -> Mapping[str, str]:
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "crates/foundation"], cwd=ROOT
    ).decode("utf-8").split("\0")
    return MappingProxyType({p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths if p})


class Arguments(argparse.Namespace):
    output: Path = ROOT / "dist" / "external-extension"
    distribution: bool = False


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("--output", type=Path, default=ROOT / "dist" / "external-extension")
    _ = parser.add_argument("--distribution", action="store_true",
                        help="build a standalone foundation workspace and use only its crate paths")
    args = parser.parse_args(namespace=Arguments())
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    before = fingerprint()
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()
    scope = Scope.DISTRIBUTION if args.distribution else Scope.DIRECT
    toolchain = cargo_input.toolchain((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
    cargo = ["cargo", f"+{toolchain}"]

    execution = Execution(output)
    consumer_files: Mapping[str, str] | None = None
    completed = False
    try:
        with tempfile.TemporaryDirectory(prefix="nepl3-external-") as temporary:
            directory = Path(temporary).resolve()
            if directory.is_relative_to(ROOT):
                raise RuntimeError("temporary workspace must be outside the repository")
            packages = PACKAGES
            if args.distribution:
                extracted = export(ROOT, directory / "foundation")
                packages = {name: extracted / path.relative_to(ROOT) for name, path in PACKAGES.items()}
                _ = execution.run(cargo + ["test", "--locked", "--workspace"], extracted, "foundation-test.log")
                directory = directory / "consumer"
                directory.mkdir()
            fixture = ROOT / "conformance/extensions/hello"
            _ = shutil.copytree(fixture / "src", directory / "src")
            _ = shutil.copytree(fixture / "examples", directory / "examples")
            _ = shutil.copytree(fixture / "tests", directory / "tests")
            _ = shutil.copyfile(fixture / "Cargo.lock", directory / "Cargo.lock")
            manifest = (fixture / "Cargo.toml").read_text(encoding="utf-8")
            manifest = external_manifest(manifest, packages)
            _ = (directory / "Cargo.toml").write_text(manifest, encoding="utf-8", newline="\n")
            consumer_files = MappingProxyType({str(p.relative_to(directory)): hashlib.sha256(p.read_bytes()).hexdigest()
                                              for p in directory.rglob("*") if p.is_file()})
            _ = execution.run(["rustc", f"+{toolchain}", "--version", "--verbose"], directory, "rustc.log")
            _ = execution.run(cargo + ["fmt", "--all", "--", "--check"], directory, "format.log")
            metadata = cargo_input.consumer_metadata(execution.run(
                cargo + ["metadata", "--locked", "--format-version", "1"], directory, "metadata.json"))
            cargo_input.check_consumer(metadata, directory, packages)
            _ = execution.run(cargo + ["clippy", "--locked", "--all-targets", "--", "-D", "warnings"], directory, "clippy.log")
            _ = execution.run(cargo + ["test", "--locked"], directory, "test.log")
            for name, arguments in [
                ("unicode", ["hello 世界"]),
                ("recipient", ["hello NEPL3"]),
                ("unknown-head", ["goodbye 世界"]),
                ("missing-child", ["hello"]),
                ("partial", ["--partial", "hello "]),
            ]:
                _ = execution.run(cargo + ["run", "--locked", "--example", "inspect", "--"] + arguments,
                    directory, f"inspect-{name}.log")
            for name, text in [
                ("nested", "add 1 mul 2 3"),
                ("unary", "neg 7"),
                ("missing-child", "add 1"),
            ]:
                _ = execution.run(cargo + ["run", "--locked", "--example", "miniexpr", "--", text],
                    directory, f"miniexpr-{name}.log")
            for name, text in [
                ("recursive", "add framed frame neg 7 2"),
                ("unknown-guest", "framed unknown"),
                ("missing-guest", "framed frame"),
            ]:
                _ = execution.run(cargo + ["run", "--locked", "--example", "composition", "--", text],
                    directory, f"composition-{name}.log")
        completed = True
    finally:
        unchanged = before == fingerprint()
        record = Record(scope, commit, before, tuple(execution.commands), completed, unchanged, consumer_files)
        _ = (output / "result.json").write_text(json.dumps(record.representation(), indent=2) + "\n", encoding="utf-8", newline="\n")
        if not unchanged:
            raise RuntimeError("foundation sources changed during extension execution")
    print(f"external public-API workspace passed: {output}")


if __name__ == "__main__":
    main()
