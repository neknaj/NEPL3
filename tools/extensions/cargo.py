"""Typed projections of Cargo metadata and lock records used by extraction."""

from dataclasses import dataclass
from pathlib import Path

from tools.serialization import json, toml


@dataclass(frozen=True, slots=True)
class LocalPackage:
    manifest: Path
    targets: tuple[Path, ...]


def local_packages(value: json.JsonValue) -> tuple[LocalPackage, ...]:
    result: list[LocalPackage] = []
    for item in json.array(json.object_value(value)["packages"]):
        package = json.object_value(item)
        source = package["source"]
        if source is not None:
            _ = json.string(source)
            continue
        targets = tuple(Path(json.string(json.object_value(target)["src_path"]))
                        for target in json.array(package["targets"]))
        result.append(LocalPackage(Path(json.string(package["manifest_path"])), targets))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class LockedPackage:
    name: str
    version: str
    source: str | None
    checksum: str | None
    dependencies: tuple[str, ...] | None
    replace: str | None


def lock_packages(source: str) -> tuple[LockedPackage, ...]:
    result: list[LockedPackage] = []
    for item in toml.array(toml.decode(source)["package"]):
        package = toml.table(item)
        if set(package) - {"name", "version", "source", "checksum", "dependencies", "replace"}:
            raise ValueError("unsupported Cargo lock package fields")
        result.append(LockedPackage(
            toml.string(package["name"]), toml.string(package["version"]),
            toml.string(package["source"]) if "source" in package else None,
            toml.string(package["checksum"]) if "checksum" in package else None,
            tuple(toml.string(item) for item in toml.array(package["dependencies"])) if "dependencies" in package else None,
            toml.string(package["replace"]) if "replace" in package else None,
        ))
    return tuple(result)


def toolchain(source: str) -> str:
    return toml.string(toml.table(toml.decode(source)["toolchain"])["channel"])


def package_name(source: str) -> str:
    return toml.string(toml.table(toml.decode(source)["package"])["name"])
