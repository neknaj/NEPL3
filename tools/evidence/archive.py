"""Bounded deterministic transport of raw collector data; no acceptance decision.

The expected archive digest and source revision are caller-selected inputs.
Nothing in this module downloads artifacts or executes their recorded commands.
"""
import argparse
from collections.abc import Mapping
import io
import os
from pathlib import Path
import re
import stat
import sys
from typing import Literal
import zipfile

from tools.evidence.records import Report
from tools.evidence.runner import digest, verify_files

MAX_FILES = 256
MAX_FILE_BYTES = 1024 * 1024
MAX_TOTAL_BYTES = 32 * 1024 * 1024
MAX_NAME_BYTES = 96
MAX_ARCHIVE_BYTES = MAX_TOTAL_BYTES + MAX_FILES * (2 * MAX_NAME_BYTES + 128)


def checked(condition: object, message: str) -> None:
    if not condition:
        raise ValueError(message)


def name(value: str) -> str:
    checked(0 < len(value) <= MAX_NAME_BYTES and re.fullmatch(r'[a-z0-9][a-z0-9.-]*', value), 'invalid archive name')
    checked(not value.endswith('.') and '..' not in value, 'invalid archive segment')
    stem = value.split('.')[0].upper()
    checked(stem not in {'CON', 'PRN', 'AUX', 'NUL'} and not re.fullmatch(r'(COM|LPT)[1-9]', stem), 'reserved archive name')
    checked(value in {'spec.json', 'manifest.json'} or value.endswith(('.stdout', '.stderr')), 'unexpected raw bundle file')
    return value


def linked(metadata: os.stat_result) -> bool:
    return stat.S_ISLNK(metadata.st_mode) or bool(
        getattr(metadata, 'st_file_attributes', 0) & getattr(stat, 'FILE_ATTRIBUTE_REPARSE_POINT', 0x400))


def directory(path: Path) -> None:
    absolute = path.absolute()
    for item in [*reversed(absolute.parents), absolute]:
        metadata = item.lstat()
        checked(stat.S_ISDIR(metadata.st_mode) and not linked(metadata), 'linked or non-directory ancestor')


def unchanged(before: os.stat_result, after: os.stat_result) -> bool:
    """Compare observations made by the same metadata API, never stat to fstat."""
    return not linked(after) and (
        after.st_mode, after.st_nlink, after.st_dev, after.st_ino,
        after.st_size, after.st_mtime_ns, after.st_ctime_ns) == (
        before.st_mode, before.st_nlink, before.st_dev, before.st_ino,
        before.st_size, before.st_mtime_ns, before.st_ctime_ns)


def read_file(path: Path, limit: int) -> bytes:
    metadata = path.lstat()
    checked(stat.S_ISREG(metadata.st_mode) and not linked(metadata) and metadata.st_nlink == 1, 'linked or special input')
    checked(0 <= metadata.st_size <= limit, 'input byte limit')
    with path.open('rb') as stream:
        opened = os.fstat(stream.fileno())
        checked(stat.S_ISREG(opened.st_mode) and not linked(opened) and opened.st_nlink == 1 and
                (opened.st_dev, opened.st_ino, opened.st_size) ==
                (metadata.st_dev, metadata.st_ino, metadata.st_size), 'input changed before read')
        # CPython 3.13 Windows lstat exposes birth time as ctime, whereas fstat
        # exposes ChangeTime (python/cpython#157671). Check each API against
        # itself, retaining path identity/link checks around the open and read.
        checked(unchanged(metadata, path.lstat()), 'input path changed before read')
        data = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
        checked(unchanged(opened, after), 'input handle changed while reading')
        checked(unchanged(metadata, path.lstat()), 'input path changed while reading')
    checked(len(data) <= limit and len(data) == metadata.st_size, 'input changed while reading')
    return data


def validate(files: Mapping[str, bytes], source_revision: str) -> Report:
    checked(re.fullmatch(r'[0-9a-f]{40}', source_revision), 'invalid selected source revision')
    checked(0 < len(files) <= MAX_FILES, 'archive file count')
    checked(sum(map(len, files.values())) <= MAX_TOTAL_BYTES, 'archive total byte limit')
    for key, data in files.items():
        _ = name(key)
        checked(len(data) <= MAX_FILE_BYTES, 'archive member byte limit')
    report = verify_files(files)
    checked(report.source_revision == source_revision, 'archive source revision mismatch')
    expected = {'manifest.json', 'spec.json'}
    for row in report.commands:
        expected.update({row.command.id + '.stdout', row.command.id + '.stderr'})
    checked(set(files) == expected, 'unexpected collector data')
    return report


def snapshot(root: Path, source_revision: str) -> dict[str, bytes]:
    directory(root)
    files: dict[str, bytes] = {}
    total = 0
    for path in root.iterdir():
        checked(len(files) < MAX_FILES, 'bundle file count')
        key = name(path.name)
        data = read_file(path, min(MAX_FILE_BYTES, MAX_TOTAL_BYTES - total))
        total += len(data)
        files[key] = data
    _ = validate(files, source_revision)
    return files


def encode(files: Mapping[str, bytes]) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_STORED, allowZip64=False) as bundle:
        for key in sorted(files):
            info = zipfile.ZipInfo(key, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            bundle.writestr(info, files[key])
    return output.getvalue()


def unpack(raw: bytes, expected_digest: str, source_revision: str) -> dict[str, bytes]:
    checked(re.fullmatch(r'[0-9a-f]{64}', expected_digest), 'invalid selected archive digest')
    checked(22 <= len(raw) <= MAX_ARCHIVE_BYTES, 'archive byte limit')
    checked(digest(raw) == expected_digest, 'archive digest mismatch')
    # Canonical archives have a final 22-byte EOCD, no comments, volumes or
    # ZIP64. Bound the central directory before ZipFile allocates its entries.
    end = raw[-22:]
    checked(end[:4] == b'PK\x05\x06' and end[4:8] == b'\0' * 4 and end[20:22] == b'\0\0', 'unsupported archive footer')
    checked(len(raw) < 42 or raw[-42:-38] != b'PK\x06\x07', 'ZIP64 locator is unsupported')
    count = int.from_bytes(end[10:12], 'little')
    checked(int.from_bytes(end[8:10], 'little') == count and 0 < count <= MAX_FILES, 'archive entry limit')
    size = int.from_bytes(end[12:16], 'little')
    offset = int.from_bytes(end[16:20], 'little')
    checked(size <= MAX_FILES * (46 + MAX_NAME_BYTES) and offset + size == len(raw) - 22, 'archive directory limit')
    # ZipFile walks the directory bytes rather than trusting the EOCD count.
    # Establish its actual record count and widths before constructing ZipInfo.
    cursor = offset
    boundary = offset + size
    for _ in range(count):
        checked(cursor + 46 <= boundary, 'truncated archive directory')
        header = raw[cursor:cursor + 46]
        checked(header[:4] == b'PK\x01\x02', 'invalid archive directory record')
        width = int.from_bytes(header[28:30], 'little')
        extra = int.from_bytes(header[30:32], 'little')
        comment = int.from_bytes(header[32:34], 'little')
        checked(0 < width <= MAX_NAME_BYTES and extra == 0 and comment == 0, 'archive directory record limit')
        cursor += 46 + width
        checked(cursor <= boundary, 'truncated archive directory name')
    checked(cursor == boundary, 'archive directory count mismatch')
    files: dict[str, bytes] = {}
    total = 0
    with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
        members = bundle.infolist()
        checked(len(members) == count, 'archive entry count mismatch')
        for entry in members:
            key = name(entry.filename)
            checked(entry.orig_filename == key and key not in files, 'duplicate or truncated archive name')
            checked(not entry.is_dir() and stat.S_IFMT(entry.external_attr >> 16) == stat.S_IFREG, 'non-regular archive member')
            checked(entry.compress_type == zipfile.ZIP_STORED and entry.flag_bits == 0 and not entry.extra and not entry.comment,
                    'unsupported archive member encoding')
            checked(0 <= entry.file_size <= MAX_FILE_BYTES and entry.compress_size == entry.file_size, 'archive member byte limit')
            total += entry.file_size
            checked(total <= MAX_TOTAL_BYTES, 'archive total byte limit')
            with bundle.open(entry) as stream:
                data = stream.read(entry.file_size + 1)
            checked(len(data) == entry.file_size, 'archive member size mismatch')
            files[key] = data
    checked(encode(files) == raw, 'noncanonical archive bytes')
    _ = validate(files, source_revision)
    return files


def pack(root: Path, output: Path, source_revision: str) -> str:
    files = snapshot(root, source_revision)
    raw = encode(files)
    checked(len(raw) <= MAX_ARCHIVE_BYTES, 'archive byte limit')
    directory(output.parent)
    with output.open('xb') as stream:
        _ = stream.write(raw)
    checked(read_file(output, MAX_ARCHIVE_BYTES) == raw, 'written archive bytes differ')
    return digest(raw)


def restore(archive: Path, output: Path, expected_digest: str, source_revision: str) -> Report:
    directory(archive.parent)
    files = unpack(read_file(archive, MAX_ARCHIVE_BYTES), expected_digest, source_revision)
    directory(output.parent)
    checked(not output.exists() and not output.is_symlink(), 'restore destination exists')
    # Validate every incoming byte before claiming an exclusive destination.
    # A failed write/verification is not a restored bundle. Preserve incomplete
    # output rather than deleting a path another process may have replaced.
    output.mkdir()
    for key, data in files.items():
        with (output / key).open('xb') as stream:
            _ = stream.write(data)
    restored = snapshot(output, source_revision)
    checked(restored == files, 'restored bytes differ')
    return validate(restored, source_revision)


class Arguments(argparse.Namespace):
    operation: Literal['pack', 'restore'] = 'pack'
    source: str = ''
    output: str = ''
    source_revision: str = ''
    expected_sha256: str = ''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument('operation', choices=['pack', 'restore'])
    _ = parser.add_argument('source')
    _ = parser.add_argument('output')
    _ = parser.add_argument('--source-revision', required=True)
    _ = parser.add_argument('--expected-sha256', default='')
    args = parser.parse_args(namespace=Arguments())
    try:
        if args.operation == 'pack':
            checked(not args.expected_sha256, 'pack does not accept an expected archive digest')
            print(pack(Path(args.source), Path(args.output), args.source_revision))
        else:
            _ = restore(Path(args.source), Path(args.output), args.expected_sha256, args.source_revision)
            print('Raw collector bundle restored and verified; acceptance decision unchanged.')
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        print(type(error).__name__ + ': evidence archive operation failed; incomplete output may remain; use a new destination', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
