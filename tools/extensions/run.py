"""Run the public-API consumer outside the repository, without private test imports."""
import argparse
from collections.abc import MutableMapping
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import tomlkit

if __package__:
    from .distribution import export
else:
    from distribution import export

ROOT = Path(__file__).resolve().parents[2]
FOUNDATION = ROOT / "crates" / "foundation"
PACKAGES = {f"nepl3-{name}": FOUNDATION / name for name in ("core", "reader", "engine", "wire")}


def external_manifest(source, packages):
    """Change dependency path values in a parsed TOML document."""
    manifest = tomlkit.parse(source)
    dependencies = manifest.get("dependencies")
    if not isinstance(dependencies, MutableMapping):
        raise ValueError("consumer manifest requires a dependencies table")
    for name, path in packages.items():
        dependency = dependencies.get(name)
        if not isinstance(dependency, MutableMapping) or not isinstance(dependency.get("path"), str):
            raise ValueError(f"consumer dependency {name} requires an explicit path")
        dependency["path"] = path.as_posix()
    return tomlkit.dumps(manifest)


def fingerprint():
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "crates/foundation"], cwd=ROOT
    ).decode("utf-8").split("\0")
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths if p}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "dist" / "external-extension")
    parser.add_argument("--distribution", action="store_true",
                        help="build a standalone foundation workspace and use only its crate paths")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    before = fingerprint()
    record = {"scope": "external workspace with public path dependencies; not an independent release or process provider",
              "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
              "foundation": before, "commands": [], "result": "failed"}
    if args.distribution:
        record["scope"] = "external consumer of an extracted foundation source workspace; publication and process provider remain unverified"
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
    cargo = ["cargo", f"+{toolchain}"]

    def run(command, cwd, name):
        try:
            result = subprocess.run(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    timeout=600, check=False)
        except (subprocess.TimeoutExpired, OSError) as failure:
            partial = getattr(failure, "stdout", None) or b""
            log = partial + ("\n" + str(failure) + "\n").encode("utf-8")
            (output / name).write_bytes(log)
            stderr = getattr(failure, "stderr", None) or b""
            (output / (name + ".stderr.log")).write_bytes(stderr)
            record["commands"].append({"command": command, "exit_code": None, "error": type(failure).__name__,
                                       "log": name, "sha256": hashlib.sha256(log).hexdigest(),
                                       "stderr": name + ".stderr.log", "stderr_sha256": hashlib.sha256(stderr).hexdigest()})
            raise
        (output / name).write_bytes(result.stdout)
        stderr = result.stderr or b""
        (output / (name + ".stderr.log")).write_bytes(stderr)
        record["commands"].append({"command": command, "exit_code": result.returncode, "log": name,
                                   "sha256": hashlib.sha256(result.stdout).hexdigest(),
                                   "stderr": name + ".stderr.log", "stderr_sha256": hashlib.sha256(stderr).hexdigest()})
        if result.returncode:
            raise RuntimeError(f"{name}: exit {result.returncode}; see {output}")
        return result.stdout

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
                run(cargo + ["test", "--locked", "--workspace"], extracted, "foundation-test.log")
                directory = directory / "consumer"
                directory.mkdir()
            fixture = ROOT / "conformance/extensions/hello"
            shutil.copytree(fixture / "src", directory / "src")
            shutil.copytree(fixture / "examples", directory / "examples")
            shutil.copytree(fixture / "tests", directory / "tests")
            shutil.copyfile(fixture / "Cargo.lock", directory / "Cargo.lock")
            manifest = (fixture / "Cargo.toml").read_text(encoding="utf-8")
            manifest = external_manifest(manifest, packages)
            (directory / "Cargo.toml").write_text(manifest, encoding="utf-8", newline="\n")
            record["consumer"] = {str(p.relative_to(directory)): hashlib.sha256(p.read_bytes()).hexdigest()
                                  for p in directory.rglob("*") if p.is_file()}
            run(["rustc", f"+{toolchain}", "--version", "--verbose"], directory, "rustc.log")
            run(cargo + ["fmt", "--all", "--", "--check"], directory, "format.log")
            metadata = json.loads(run(cargo + ["metadata", "--locked", "--format-version", "1"], directory, "metadata.json"))
            consumers = [p for p in metadata["packages"] if p["name"] == "external-hello-language"]
            if len(consumers) != 1:
                raise RuntimeError("expected exactly one external consumer")
            consumer = consumers[0]
            if (metadata["workspace_members"] != [consumer["id"]]
                    or metadata["resolve"]["root"] != consumer["id"]
                    or Path(metadata["workspace_root"]).resolve() != directory
                    or Path(consumer["manifest_path"]).resolve() != directory / "Cargo.toml"):
                raise RuntimeError("consumer shares a workspace with foundation")
            foundation_packages = [p for p in metadata["packages"] if p["name"].startswith("nepl3-")]
            if len(foundation_packages) != len(PACKAGES):
                raise RuntimeError("unexpected or duplicate foundation packages")
            actual = {p["name"]: Path(p["manifest_path"]).parent.resolve() for p in foundation_packages}
            if actual != {name: path.resolve() for name, path in packages.items()}:
                raise RuntimeError("unexpected domain, app, tools, or substituted foundation dependency")
            run(cargo + ["clippy", "--locked", "--all-targets", "--", "-D", "warnings"], directory, "clippy.log")
            run(cargo + ["test", "--locked"], directory, "test.log")
            for name, arguments in [
                ("unicode", ["hello 世界"]),
                ("recipient", ["hello NEPL3"]),
                ("unknown-head", ["goodbye 世界"]),
                ("missing-child", ["hello"]),
                ("partial", ["--partial", "hello "]),
            ]:
                run(cargo + ["run", "--locked", "--example", "inspect", "--"] + arguments,
                    directory, f"inspect-{name}.log")
            for name, text in [
                ("nested", "add 1 mul 2 3"),
                ("unary", "neg 7"),
                ("missing-child", "add 1"),
            ]:
                run(cargo + ["run", "--locked", "--example", "miniexpr", "--", text],
                    directory, f"miniexpr-{name}.log")
            for name, text in [
                ("recursive", "add framed frame neg 7 2"),
                ("unknown-guest", "framed unknown"),
                ("missing-guest", "framed frame"),
            ]:
                run(cargo + ["run", "--locked", "--example", "composition", "--", text],
                    directory, f"composition-{name}.log")
        completed = True
    finally:
        record["foundation_unchanged"] = before == fingerprint()
        record["result"] = "passed" if completed and record["foundation_unchanged"] else "failed"
        (output / "result.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8", newline="\n")
        if not record["foundation_unchanged"]:
            raise RuntimeError("foundation sources changed during extension execution")
    print(f"external public-API workspace passed: {output}")


if __name__ == "__main__":
    main()
