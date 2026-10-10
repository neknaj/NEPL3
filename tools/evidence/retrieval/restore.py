"""Restore unchanged pinned log bytes; never change acceptance state."""
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path
import re
import time
import unicodedata

from tools.evidence.archive import checked, directory, read_file
from tools.evidence.artifact import unpack
from tools.evidence.retrieval.metadata import verify
from tools.evidence.retrieval.network import Download, fetch
from tools.evidence.retrieval.pin import Pin
from tools.evidence.runner import digest

MAX_REQUESTS = 256
MAX_DOWNLOAD_BYTES = 64 * 1024 * 1024
MAX_LOG_BYTES = 32 * 1024 * 1024


@dataclass(frozen=True, slots=True)
class Request:
    destination: str
    sha256: str
    artifact: Pin | None


def path_parts(relative: str) -> list[str]:
    checked(0 < len(relative.encode('utf-8')) <= 4096 and not any(c in relative for c in ('\\', '\0', ':')),
            'unsafe repository-relative path')
    parts = relative.split('/')
    for part in parts:
        checked(part not in ('', '.', '..') and len(part.encode('utf-8')) <= 255 and
                not part.endswith(('.', ' ')), 'unsafe repository path component')
        stem = part.split('.')[0].lower()
        checked(stem not in {'con', 'prn', 'aux', 'nul'} and not re.fullmatch(r'(com|lpt)[1-9¹²³]', stem),
                'reserved repository path component')
    return parts


def destination(root: Path, relative: str) -> Path:
    checked(relative.startswith('dist/evidence/'), 'restored log path prefix')
    parts = path_parts(relative)
    checked(len(parts) >= 3, 'restored log path')
    checked(parts[-1].endswith(('.txt', '.log')), 'restored log suffix')
    directory(root)
    path = root
    for part in parts[:-1]:
        path = path / part
        try:
            _ = path.lstat()
        except FileNotFoundError:
            continue
        directory(path)
    return root.joinpath(*parts)


def restore(root: Path, requests: Sequence[Request], token: str,
            check_identity: Callable[[str], None],
            download: Callable[[Pin, str], Download] = fetch) -> int:
    checked(len(requests) <= MAX_REQUESTS, 'restored log request count')
    pending: dict[Path, bytes] = {}
    cached: dict[int, Download] = {}
    decoded: dict[int, dict[str, bytes]] = {}
    total_download = 0
    total_expanded = 0
    # Establish every source identity before any network request or file write.
    for request in requests:
        _ = destination(root, request.destination)
        checked(re.fullmatch(r'[0-9a-f]{64}', request.sha256), 'restored log digest')
        if request.artifact is not None:
            request.artifact.validate()
            check_identity(request.artifact.source_commit)
    for request in requests:
        target = destination(root, request.destination)
        pin = request.artifact
        if pin is None:
            # Explicitly supplied local logs remain compatible; a fresh checkout
            # without them fails. A locator-bearing request never takes this path.
            data = read_file(target, 1024 * 1024)
        else:
            if pin.artifact_id not in cached:
                checked(total_download + pin.archive_bytes <= MAX_DOWNLOAD_BYTES, 'aggregate download bound')
                package = download(pin, token)
                verify(pin, package.artifact, package.run, int(time.time()))
                checked(len(package.archive) == pin.archive_bytes, 'downloaded archive size mismatch')
                files = unpack(package.archive, pin.archive_sha256)
                total_expanded += sum(map(len, files.values()))
                checked(total_expanded <= MAX_LOG_BYTES, 'aggregate expanded artifact bound')
                cached[pin.artifact_id] = package
                decoded[pin.artifact_id] = files
                total_download += len(package.archive)
            package = cached[pin.artifact_id]
            # Recheck all pins even when sharing the same archive, including
            # expiry, source and conflicts in repeated locators. No stale cache.
            verify(pin, package.artifact, package.run, int(time.time()))
            checked(len(package.archive) == pin.archive_bytes and digest(package.archive) == pin.archive_sha256,
                    'conflicting artifact archive pins')
            files = decoded[pin.artifact_id]
            checked(pin.member in files, 'selected artifact member missing')
            data = files[pin.member]
        checked(0 < len(data) <= 1024 * 1024 and digest(data) == request.sha256, 'restored raw log mismatch')
        if target in pending:
            checked(pending[target] == data, 'conflicting restored log destinations')
        else:
            pending[target] = data
        checked(sum(map(len, pending.values())) <= MAX_LOG_BYTES, 'restored log aggregate bound')
        try:
            _ = target.lstat()
        except FileNotFoundError:
            pass
        else:
            checked(read_file(target, 1024 * 1024) == data, 'existing log has conflicting bytes')
    aliases: dict[str, bytes] = {}
    for path, data in pending.items():
        name = unicodedata.normalize('NFC', path.relative_to(root).as_posix()).casefold()
        checked(name not in aliases or aliases[name] == data, 'restored filename alias conflict')
        aliases[name] = data
    for name in aliases:
        parts = name.split('/')
        checked(not any('/'.join(parts[:end]) in aliases for end in range(1, len(parts))),
                'restored file/directory prefix conflict')
    for source in sorted({request.artifact.source_commit for request in requests if request.artifact is not None}):
        check_identity(source)
    for request in requests:
        if request.artifact is not None:
            package = cached[request.artifact.artifact_id]
            verify(request.artifact, package.artifact, package.run, int(time.time()))
    # Validate the whole attempt before any output creation. Failed writes may
    # leave partial output; never delete or replace another process's files.
    for target, data in pending.items():
        relative = target.relative_to(root)
        parent = root
        for part in relative.parts[:-1]:
            parent = parent / part
            try:
                parent.mkdir()
            except FileExistsError:
                pass
            directory(parent)
        try:
            with target.open('xb') as stream:
                _ = stream.write(data)
        except FileExistsError:
            checked(read_file(target, 1024 * 1024) == data, 'log changed before restore')
        checked(read_file(target, 1024 * 1024) == data, 'restored log readback mismatch')
    for request in requests:
        if request.artifact is not None:
            package = cached[request.artifact.artifact_id]
            verify(request.artifact, package.artifact, package.run, int(time.time()))
    return len(pending)
