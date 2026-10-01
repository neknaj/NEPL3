"""Materialize a fixed public consumer from Git objects, never the working tree."""
from pathlib import Path, PurePosixPath
import os
import re
import subprocess

PREFIX = "conformance/extensions/hello/"
MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_TOTAL_BYTES = 32 * 1024 * 1024
MAX_FILES = 500


def revision(value: str) -> str:
    if re.fullmatch(r"[0-9a-f]{40}", value) is None:
        raise ValueError("consumer revision must be a complete lowercase 40-hex commit")
    return value


def git(root: Path, arguments: list[str]) -> bytes:
    environment = dict(os.environ)
    environment.update({"GIT_NO_REPLACE_OBJECTS": "1", "GIT_NO_LAZY_FETCH": "1"})
    return subprocess.run(["git", *arguments], cwd=root, env=environment,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          check=True, timeout=60).stdout


def materialize(root: Path, destination: Path, commit: str) -> Path:
    """Only regular blobs under the fixed consumer prefix are admitted.

    Failure never falls back to a current consumer. The destination must be new;
    caller-owned temporary directories handle cleanup after partial failure.
    """
    commit = revision(commit)
    resolved = git(root, ["rev-parse", "--verify", commit + "^{commit}"]).decode("ascii").strip()
    if resolved != commit:
        raise ValueError("consumer revision must identify the commit itself")
    entries = git(root, ["ls-tree", "-rlz", commit, "--", PREFIX.rstrip("/")]).split(b"\0")
    records = [entry for entry in entries if entry]
    if not records or len(records) > MAX_FILES:
        raise ValueError("consumer revision has no files or exceeds the file limit")
    total = 0
    selected: list[tuple[str, PurePosixPath, int]] = []
    paths: set[PurePosixPath] = set()
    portable_names: set[str] = set()
    for record in records:
        metadata, raw_path = record.split(b"\t", 1)
        mode, kind, oid, raw_size = metadata.split()
        if mode not in (b"100644", b"100755") or kind != b"blob":
            raise ValueError("consumer contains a symlink, submodule or unsupported mode")
        name = raw_path.decode("utf-8")
        if not name.startswith(PREFIX):
            raise ValueError("consumer path escaped its declared prefix")
        relative = name[len(PREFIX):]
        parts = relative.split("/")
        if any(not part or part in (".", "..") or "\\" in part or ":" in part
               or any(ord(char) < 32 for char in part) for part in parts):
            raise ValueError("unsafe consumer path")
        reserved = {"CON", "PRN", "AUX", "NUL", *(f"COM{i}" for i in range(1, 10)), *(f"LPT{i}" for i in range(1, 10))}
        if any(part.endswith((".", " ")) or part.casefold() == ".git"
               or part.split(".", 1)[0].upper() in reserved for part in parts):
            raise ValueError("nonportable consumer path")
        portable_name = relative.casefold()
        if portable_name in portable_names:
            raise ValueError("case-colliding consumer path")
        portable_names.add(portable_name)
        path = PurePosixPath(relative)
        if path in paths:
            raise ValueError("duplicate consumer path")
        paths.add(path)
        size = int(raw_size)
        if size < 0 or size > MAX_FILE_BYTES:
            raise ValueError("consumer blob exceeds the size limit")
        total += size
        if total > MAX_TOTAL_BYTES:
            raise ValueError("consumer exceeds the aggregate size limit")
        blob = oid.decode("ascii")
        if re.fullmatch(r"[0-9a-f]{40}", blob) is None:
            raise ValueError("invalid consumer object id")
        selected.append((blob, path, size))
    required = (PurePosixPath("Cargo.toml"), PurePosixPath("Cargo.lock"), PurePosixPath("src/lib.rs"))
    if not all(path in paths for path in required):
        raise ValueError("consumer revision lacks its manifest, lock or library")
    destination.mkdir(parents=True, exist_ok=False)
    for oid, path, size in selected:
        contents = git(root, ["cat-file", "blob", oid])
        if len(contents) != size:
            raise ValueError("consumer blob size mismatch")
        target = destination.joinpath(*path.parts)
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = target.write_bytes(contents)
    return destination
