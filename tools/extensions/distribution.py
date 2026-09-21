"""Export the four foundation crates as a standalone source workspace."""
import argparse
from collections.abc import Mapping
from pathlib import Path
import shutil
import subprocess
import tomllib

import tomlkit

ROOT = Path(__file__).resolve().parents[2]
MEMBERS = tuple(f"crates/foundation/{name}" for name in ("core", "wire", "reader", "engine"))


def check_lock(original, extracted):
    """Cargo may remove unused packages; retained package records must stay identical."""
    before = tomllib.loads(original)["package"]
    after = tomllib.loads(extracted)["package"]
    for package in after:
        if package not in before:
            raise ValueError(f"extraction changed a locked package: {package['name']}")


def workspace_manifest(source, manifests):
    """Retain inherited settings and the dependencies actually used by the crates."""
    document = tomlkit.parse(source)
    workspace = document["workspace"]
    dependencies = workspace["dependencies"]
    required = set()
    for manifest in manifests:
        crate = tomlkit.parse(manifest)
        tables = [crate]
        tables.extend(crate.get("target", {}).values())
        for table in tables:
            for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
                for name, dependency in table.get(kind, {}).items():
                    if isinstance(dependency, Mapping) and dependency.get("workspace") is True:
                        required.add(name)
                    elif isinstance(dependency, Mapping) and "path" in dependency:
                        raise ValueError(f"unreviewed crate-relative dependency: {name}")
    for name in required:
        dependency = dependencies[name]
        if isinstance(dependency, Mapping) and "path" in dependency:
            if dependency["path"] not in MEMBERS:
                raise ValueError(f"foundation dependency escapes distribution: {name}")
    for name in list(dependencies):
        if name not in required:
            del dependencies[name]
    workspace["members"] = list(MEMBERS)
    workspace.pop("default-members", None)
    workspace.pop("exclude", None)
    return tomlkit.dumps(document)


def export(root, destination):
    """Copy tracked crate files; refuse an existing destination and source symlinks."""
    root = root.resolve()
    destination = destination.resolve()
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "--", *MEMBERS], cwd=root
    ).decode("utf-8").split("\0")
    paths = [Path(p) for p in paths if p]
    for path in paths:
        source = root / path
        if source.is_symlink() or not source.resolve().is_relative_to(root):
            raise ValueError(f"source path escapes distribution: {path}")
        if not source.is_file():
            raise ValueError(f"missing tracked source: {path}")
    manifest = workspace_manifest(
        (root / "Cargo.toml").read_text(encoding="utf-8"),
        [(root / member / "Cargo.toml").read_text(encoding="utf-8") for member in MEMBERS],
    )
    destination.mkdir(parents=True, exist_ok=False)
    for path in paths + [Path("Cargo.lock"), Path("rust-toolchain.toml"), Path("LICENSE")]:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / path, target)
    (destination / "Cargo.toml").write_text(manifest, encoding="utf-8", newline="\n")
    # Let Cargo prune the workspace lockfile using the pinned toolchain and
    # cached dependencies. Verify that resolution introduced no version changes.
    toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
    subprocess.run(["cargo", f"+{toolchain}", "metadata", "--offline", "--format-version", "1"],
                   cwd=destination, stdout=subprocess.DEVNULL, check=True)
    check_lock((root / "Cargo.lock").read_text(encoding="utf-8"),
               (destination / "Cargo.lock").read_text(encoding="utf-8"))
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    print(export(ROOT, args.output))


if __name__ == "__main__":
    main()
