"""Typed projections of Cargo metadata and lock records used by extraction."""

from dataclasses import dataclass
from collections.abc import Mapping
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


@dataclass(frozen=True, slots=True)
class Package:
    identity: str
    name: str
    manifest: Path


@dataclass(frozen=True, slots=True)
class ConsumerMetadata:
    packages: tuple[Package, ...]
    members: tuple[str, ...]
    workspace: Path
    root: str | None


def consumer_metadata(source: bytes) -> ConsumerMetadata:
    value = json.object_value(json.decode(source))
    packages: list[Package] = []
    for item in json.array(value["packages"]):
        package = json.object_value(item)
        packages.append(Package(json.string(package["id"]), json.string(package["name"]),
                                Path(json.string(package["manifest_path"]))))
    root = json.object_value(value["resolve"])["root"]
    return ConsumerMetadata(tuple(packages), tuple(json.string(item) for item in json.array(value["workspace_members"])),
                            Path(json.string(value["workspace_root"])), None if root is None else json.string(root))


def check_consumer(metadata: ConsumerMetadata, directory: Path, packages: Mapping[str, Path]) -> None:
    consumers = [package for package in metadata.packages if package.name == "external-hello-language"]
    if len(consumers) != 1:
        raise RuntimeError("expected exactly one external consumer")
    consumer = consumers[0]
    if (metadata.members != (consumer.identity,) or metadata.root != consumer.identity
            or metadata.workspace.resolve() != directory
            or consumer.manifest.resolve() != directory / "Cargo.toml"):
        raise RuntimeError("consumer shares a workspace with foundation")
    foundation = [package for package in metadata.packages if package.name.startswith("nepl3-")]
    if len(foundation) != len(packages):
        raise RuntimeError("unexpected or duplicate foundation packages")
    actual = {package.name: package.manifest.parent.resolve() for package in foundation}
    if actual != {name: path.resolve() for name, path in packages.items()}:
        raise RuntimeError("unexpected domain, app, tools, or substituted foundation dependency")
