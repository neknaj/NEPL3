"""Export standalone Foundation and dependent Doc source workspaces."""
import argparse
from collections.abc import Mapping
import json
from pathlib import Path
import shutil
import subprocess
import tomllib

import tomlkit

ROOT = Path(__file__).resolve().parents[2]
MEMBERS = tuple(f"crates/foundation/{name}" for name in ("core", "wire", "reader", "engine"))
SUPPORT = ("Cargo.lock", "rust-toolchain.toml", "LICENSE")
DOC_MEMBERS = ("crates/languages/doc/core", "crates/languages/doc/html", "crates/output/markup")


def check_lock(original, extracted):
    """Cargo may remove unused packages; retained package records must stay identical."""
    before = tomllib.loads(original)["package"]
    after = tomllib.loads(extracted)["package"]
    for package in after:
        if package not in before:
            raise ValueError(f"extraction changed a locked package: {package['name']}")


def workspace_manifest(source, manifests, *, members=MEMBERS, external=None):
    """Retain inherited settings and the dependencies actually used by the crates."""
    document = tomlkit.parse(source)
    workspace = document["workspace"]
    dependencies = workspace["dependencies"]
    external = external or {}
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
            path = dependency["path"]
            if path in external:
                dependency["path"] = external[path].as_posix()
            elif path not in members:
                raise ValueError(f"dependency escapes distribution: {name}")
    for name in list(dependencies):
        if name not in required:
            del dependencies[name]
    workspace["members"] = list(members)
    workspace.pop("default-members", None)
    workspace.pop("exclude", None)
    return tomlkit.dumps(document)


def export(root, destination):
    return _export(root, destination, MEMBERS, (), {})


def check_foundation(root, foundation):
    """Require the unchanged crate files exported from this source revision."""
    tracked = subprocess.check_output(
        ["git", "ls-files", "-z", "--", *MEMBERS], cwd=root,
    ).decode("utf-8").split("\0")
    expected = {Path(path) for path in tracked if path}
    actual = set()
    for member in MEMBERS:
        for path in (foundation / member).rglob("*"):
            if path.is_symlink() or not path.resolve().is_relative_to(foundation):
                raise ValueError(f"external Foundation source escapes distribution: {path}")
            if path.is_file():
                actual.add(path.relative_to(foundation))
    if actual != expected:
        raise ValueError("external Foundation file set differs from this source revision")
    for path in expected:
        source = root / path
        if source.is_symlink() or not source.resolve().is_relative_to(root):
            raise ValueError(f"source path escapes distribution: {path}")
        if source.read_bytes() != (foundation / path).read_bytes():
            raise ValueError(f"external Foundation source differs: {path}")


def check_metadata(metadata, allowed):
    """Check resolved local manifests and Cargo target entry sources."""
    for package in metadata["packages"]:
        if package["source"] is None:
            paths = [package["manifest_path"], *(t["src_path"] for t in package["targets"])]
            for path in paths:
                resolved = Path(path).resolve()
                if not any(resolved.is_relative_to(root) for root in allowed):
                    raise ValueError(f"resolved dependency escapes distribution: {resolved}")


def export_doc(root, destination, foundation):
    """Extract Doc/HTML/markup against a separately exported Foundation.

    Includes the Doc language definition and crate-local tests/assets. The
    monorepo development-host parser/Math composition is not copied implicitly.
    Foundation must retain the exact crate files of this source revision.
    """
    root = root.resolve()
    foundation = foundation.resolve()
    if foundation.is_relative_to(root) or root.is_relative_to(foundation):
        raise ValueError("Foundation distribution must be outside the source repository")
    external = {}
    for member in MEMBERS:
        directory = foundation / member
        manifest = directory / "Cargo.toml"
        if not directory.resolve().is_relative_to(foundation) or not manifest.is_file():
            raise ValueError(f"missing or escaped Foundation crate: {member}")
        package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]
        if package["name"] != f"nepl3-{Path(member).name}":
            raise ValueError(f"unexpected Foundation package: {member}")
        external[member] = directory
    check_foundation(root, foundation)
    return _export(root, destination, DOC_MEMBERS, ("languages/doc/syntax.neplg",), external)


def _export(root, destination, members, extra, external):
    """Copy tracked crate files; refuse an existing destination and source symlinks."""
    root = root.resolve()
    destination = destination.resolve()
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "--", *members], cwd=root
    ).decode("utf-8").split("\0")
    paths = [Path(p) for p in paths if p]
    copies = paths + [Path(p) for p in (*SUPPORT, *extra)]
    for path in copies + [Path("Cargo.toml")] + [Path(m) / "Cargo.toml" for m in members]:
        source = root / path
        if source.is_symlink() or not source.resolve().is_relative_to(root):
            raise ValueError(f"source path escapes distribution: {path}")
        if not source.is_file():
            raise ValueError(f"missing tracked source: {path}")
    manifest = workspace_manifest(
        (root / "Cargo.toml").read_text(encoding="utf-8"),
        [(root / member / "Cargo.toml").read_text(encoding="utf-8") for member in members],
        members=members, external=external,
    )
    destination.mkdir(parents=True, exist_ok=False)
    for path in copies:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / path, target)
    (destination / "Cargo.toml").write_text(manifest, encoding="utf-8", newline="\n")
    # Let Cargo prune the workspace lockfile using the pinned toolchain and
    # cached dependencies. Verify that resolution introduced no version changes.
    toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
    metadata = json.loads(subprocess.check_output(
        ["cargo", f"+{toolchain}", "metadata", "--offline", "--format-version", "1"],
        cwd=destination,
    ))
    allowed = [destination, *(path.resolve() for path in external.values())]
    check_metadata(metadata, allowed)
    check_lock((root / "Cargo.lock").read_text(encoding="utf-8"),
               (destination / "Cargo.lock").read_text(encoding="utf-8"))
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--doc", type=Path, metavar="FOUNDATION",
                        help="extract Doc against this existing standalone Foundation workspace")
    args = parser.parse_args()
    print(export_doc(ROOT, args.output, args.doc) if args.doc else export(ROOT, args.output))


if __name__ == "__main__":
    main()
