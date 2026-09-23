"""Export standalone Foundation and dependent Doc source workspaces."""
import argparse
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePath
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.extensions import manifests as manifest_adapter
from tools.extensions import cargo
from tools.serialization.json import JsonValue, decode
MEMBERS = tuple(f"crates/foundation/{name}" for name in ("core", "wire", "reader", "engine"))
SUPPORT = ("Cargo.lock", "rust-toolchain.toml", "LICENSE")
DOC_MEMBERS = ("crates/languages/doc/core", "crates/languages/doc/html", "crates/output/markup")


def check_lock(original: str, extracted: str) -> None:
    """Cargo may remove unused packages; retained package records must stay identical."""
    before = cargo.lock_packages(original)
    after = cargo.lock_packages(extracted)
    for package in after:
        if package not in before:
            raise ValueError(f"extraction changed a locked package: {package.name}")


def workspace_manifest(
    source: str, manifests: Sequence[str], *, members: Sequence[str] = MEMBERS,
    external: Mapping[str, PurePath] | None = None,
) -> str:
    """Retain inherited settings and the dependencies actually used by the crates."""
    return manifest_adapter.workspace_manifest(
        source, manifests, members=members, external=external if external is not None else {},
    )


def export(root: Path, destination: Path) -> Path:
    return _export(root, destination, MEMBERS, (), {})


def check_foundation(root: Path, foundation: Path) -> None:
    """Require the unchanged crate files exported from this source revision."""
    tracked = subprocess.check_output(
        ["git", "ls-files", "-z", "--", *MEMBERS], cwd=root,
    ).decode("utf-8").split("\0")
    expected = {Path(path) for path in tracked if path}
    actual: set[Path] = set()
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


def check_metadata(metadata: JsonValue, allowed: Sequence[Path]) -> None:
    """Check resolved local manifests and Cargo target entry sources."""
    for package in cargo.local_packages(metadata):
        for path in (package.manifest, *package.targets):
            resolved = path.resolve()
            if not any(resolved.is_relative_to(root) for root in allowed):
                raise ValueError(f"resolved dependency escapes distribution: {resolved}")


def export_doc(root: Path, destination: Path, foundation: Path) -> Path:
    """Extract Doc/HTML/markup against a separately exported Foundation.

    Includes the Doc language definition and crate-local tests/assets. The
    monorepo development-host parser/Math composition is not copied implicitly.
    Foundation must retain the exact crate files of this source revision.
    """
    root = root.resolve()
    foundation = foundation.resolve()
    if foundation.is_relative_to(root) or root.is_relative_to(foundation):
        raise ValueError("Foundation distribution must be outside the source repository")
    external: dict[str, Path] = {}
    for member in MEMBERS:
        directory = foundation / member
        manifest = directory / "Cargo.toml"
        if not directory.resolve().is_relative_to(foundation) or not manifest.is_file():
            raise ValueError(f"missing or escaped Foundation crate: {member}")
        if cargo.package_name(manifest.read_text(encoding="utf-8")) != f"nepl3-{Path(member).name}":
            raise ValueError(f"unexpected Foundation package: {member}")
        external[member] = directory
    check_foundation(root, foundation)
    return _export(root, destination, DOC_MEMBERS, ("languages/doc/syntax.neplg",), external)


def _export(
    root: Path, destination: Path, members: Sequence[str], extra: Sequence[str], external: Mapping[str, Path],
) -> Path:
    """Copy tracked crate files; refuse an existing destination and source symlinks."""
    root = root.resolve()
    destination = destination.resolve()
    names = subprocess.check_output(
        ["git", "ls-files", "-z", "--", *members], cwd=root
    ).decode("utf-8").split("\0")
    paths = [Path(p) for p in names if p]
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
        _ = shutil.copyfile(root / path, target)
    _ = (destination / "Cargo.toml").write_text(manifest, encoding="utf-8", newline="\n")
    # Let Cargo prune the workspace lockfile using the pinned toolchain and
    # cached dependencies. Verify that resolution introduced no version changes.
    toolchain = cargo.toolchain((root / "rust-toolchain.toml").read_text(encoding="utf-8"))
    metadata = decode(subprocess.check_output(
        ["cargo", f"+{toolchain}", "metadata", "--offline", "--format-version", "1"],
        cwd=destination,
    ))
    allowed = [destination, *(path.resolve() for path in external.values())]
    check_metadata(metadata, allowed)
    check_lock((root / "Cargo.lock").read_text(encoding="utf-8"),
               (destination / "Cargo.lock").read_text(encoding="utf-8"))
    return destination


class Arguments(argparse.Namespace):
    output: Path = Path()
    doc: Path | None = None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("output", type=Path)
    _ = parser.add_argument("--doc", type=Path, metavar="FOUNDATION",
                        help="extract Doc against this existing standalone Foundation workspace")
    args = parser.parse_args(namespace=Arguments())
    print(export_doc(ROOT, args.output, args.doc) if args.doc else export(ROOT, args.output))


if __name__ == "__main__":
    main()
