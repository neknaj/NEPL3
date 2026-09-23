"""Select explicit Cargo dependencies and edit their TOML path values."""

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from enum import Enum
from pathlib import PurePath
from typing import assert_never

from tools.serialization.toml import Table, optional_table, parse


class Dependency(Enum):
    INHERITED = "inherited"
    OTHER = "other"


@dataclass(frozen=True, slots=True)
class PathDependency:
    path: str


def dependency(value: object) -> Dependency | PathDependency:
    table = optional_table(value)
    if table is None:
        return Dependency.OTHER
    if table.contains("workspace") and table.value("workspace") is True:
        return Dependency.INHERITED
    if table.contains("path"):
        path = table.value("path")
        if not isinstance(path, str):
            raise ValueError("Cargo dependency path must be a string")
        return PathDependency(path)
    return Dependency.OTHER


def inherited(source: str) -> frozenset[str]:
    crate = parse(source)
    tables: list[Table] = [crate]
    if crate.contains("target"):
        targets = crate.child("target")
        tables.extend(targets.child(name) for name in targets.keys())
    result: set[str] = set()
    for table in tables:
        for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
            if not table.contains(kind):
                continue
            dependencies = table.child(kind)
            for name in dependencies.keys():
                selected = dependency(dependencies.value(name))
                match selected:
                    case Dependency.INHERITED:
                        result.add(name)
                    case PathDependency():
                        raise ValueError(f"unreviewed crate-relative dependency: {name}")
                    case Dependency.OTHER:
                        pass
                    case _:
                        assert_never(selected)
    return frozenset(result)


def external_manifest(source: str, packages: Mapping[str, PurePath]) -> str:
    """Change dependency path values in a parsed TOML document."""
    document = parse(source)
    if not document.contains("dependencies"):
        raise ValueError("consumer manifest requires a dependencies table")
    dependencies = document.child("dependencies")
    for name, path in packages.items():
        value = dependencies.value(name) if dependencies.contains(name) else None
        entry = optional_table(value)
        if entry is None or not entry.contains("path") or not isinstance(entry.value("path"), str):
            raise ValueError(f"consumer dependency {name} requires an explicit path")
        entry.set("path", path.as_posix())
    return document.render()


def workspace_manifest(
    source: str,
    manifests: Sequence[str],
    *,
    members: Sequence[str],
    external: Mapping[str, PurePath],
) -> str:
    """Retain inherited settings and dependencies actually used by the crates."""
    document = parse(source)
    workspace = document.child("workspace")
    dependencies = workspace.child("dependencies")
    required = {name for manifest in manifests for name in inherited(manifest)}
    for name in required:
        # Inherited workspace entries themselves carry the actual path, if any.
        entry = optional_table(dependencies.value(name))
        if entry is not None and entry.contains("path"):
            path = entry.value("path")
            if not isinstance(path, str):
                raise ValueError("Cargo dependency path must be a string")
            if path in external:
                entry.set("path", external[path].as_posix())
            elif path not in members:
                raise ValueError(f"dependency escapes distribution: {name}")
    for name in dependencies.keys():
        if name not in required:
            dependencies.remove(name)
    workspace.set("members", members)
    for key in ("default-members", "exclude"):
        if workspace.contains(key):
            workspace.remove(key)
    return document.render()
