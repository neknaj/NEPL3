"""Bounded decoding of pinned GitHub Actions raw-log ZIPs.

This byte boundary does not download, authenticate, select an artifact, restore
files, or grant acceptance. The caller must independently verify the repository,
run/attempt, source identity, expiry and archive pin before using these bytes.
"""
import io
import re
import stat
import zipfile
import zlib

from tools.evidence.archive import checked
from tools.evidence.runner import digest

MAX_FILES = 256
MAX_NAME_BYTES = 96
MAX_FILE_BYTES = 1024 * 1024
MAX_TOTAL_BYTES = 32 * 1024 * 1024
MAX_ARCHIVE_BYTES = MAX_TOTAL_BYTES


def member_name(value: str) -> str:
    checked(0 < len(value) <= MAX_NAME_BYTES and
            re.fullmatch(r'[a-z0-9][a-z0-9.-]*', value), 'invalid artifact member name')
    checked(not value.endswith('.') and '..' not in value, 'invalid artifact member segment')
    stem = value.split('.')[0].upper()
    checked(stem not in {'CON', 'PRN', 'AUX', 'NUL'} and
            not re.fullmatch(r'(COM|LPT)[1-9]', stem), 'reserved artifact member name')
    return value


def directory_count(raw: bytes) -> int:
    """Bound actual central-directory records before ZipFile allocates them."""
    end = raw[-22:]
    checked(end[:4] == b'PK\x05\x06' and end[4:8] == b'\0' * 4 and
            end[20:22] == b'\0\0', 'unsupported artifact footer')
    checked(len(raw) < 42 or raw[-42:-38] != b'PK\x06\x07', 'ZIP64 artifact unsupported')
    count = int.from_bytes(end[10:12], 'little')
    checked(int.from_bytes(end[8:10], 'little') == count and
            0 < count <= MAX_FILES, 'artifact entry limit')
    size = int.from_bytes(end[12:16], 'little')
    offset = int.from_bytes(end[16:20], 'little')
    checked(size <= MAX_FILES * (46 + MAX_NAME_BYTES) and
            offset + size == len(raw) - 22, 'artifact directory limit')
    cursor = offset
    boundary = offset + size
    for _ in range(count):
        checked(cursor + 46 <= boundary, 'truncated artifact directory')
        header = raw[cursor:cursor + 46]
        checked(header[:4] == b'PK\x01\x02', 'invalid artifact directory record')
        width = int.from_bytes(header[28:30], 'little')
        extra = int.from_bytes(header[30:32], 'little')
        comment = int.from_bytes(header[32:34], 'little')
        checked(0 < width <= MAX_NAME_BYTES and extra == 0 and comment == 0,
                'artifact directory record limit')
        cursor += 46 + width
        checked(cursor <= boundary, 'truncated artifact directory name')
    checked(cursor == boundary, 'artifact directory count mismatch')
    return count


def local_records(raw: bytes, entries: list[zipfile.ZipInfo]) -> None:
    """Reject gaps, overlapping payloads and inconsistent local descriptors."""
    boundary = int.from_bytes(raw[-6:-2], 'little')
    cursor = 0
    for entry in sorted(entries, key=lambda item: item.header_offset):
        checked(entry.header_offset == cursor and cursor + 30 <= boundary,
                'artifact local record offset')
        header = raw[cursor:cursor + 30]
        checked(header[:4] == b'PK\x03\x04', 'invalid artifact local record')
        checked(int.from_bytes(header[4:6], 'little') == entry.extract_version and
                int.from_bytes(header[6:8], 'little') == entry.flag_bits and
                int.from_bytes(header[8:10], 'little') == entry.compress_type,
                'artifact local encoding mismatch')
        name = entry.filename.encode('ascii')
        width = int.from_bytes(header[26:28], 'little')
        checked(width == len(name) and header[28:30] == b'\0\0' and
                raw[cursor + 30:cursor + 30 + width] == name,
                'artifact local member name mismatch')
        declared = (int.from_bytes(header[14:18], 'little'),
                    int.from_bytes(header[18:22], 'little'),
                    int.from_bytes(header[22:26], 'little'))
        expected = (entry.CRC, entry.compress_size, entry.file_size)
        cursor += 30 + width + entry.compress_size
        checked(cursor <= boundary, 'truncated artifact local payload')
        if entry.flag_bits == 8:
            checked(declared == (0, 0, 0), 'artifact deferred sizes must be zero')
            checked(cursor + 16 <= boundary and raw[cursor:cursor + 4] == b'PK\x07\x08',
                    'missing artifact data descriptor')
            descriptor = tuple(int.from_bytes(raw[cursor + start:cursor + start + 4], 'little')
                               for start in (4, 8, 12))
            checked(descriptor == expected, 'artifact data descriptor mismatch')
            cursor += 16
        else:
            checked(declared == expected, 'artifact local sizes or CRC mismatch')
    checked(cursor == boundary, 'artifact unowned local bytes')


def unpack(raw: bytes, expected_sha256: str) -> dict[str, bytes]:
    checked(re.fullmatch(r'[0-9a-f]{64}', expected_sha256), 'invalid artifact digest pin')
    checked(22 <= len(raw) <= MAX_ARCHIVE_BYTES, 'artifact archive byte limit')
    checked(digest(raw) == expected_sha256, 'artifact archive digest mismatch')
    count = directory_count(raw)
    files: dict[str, bytes] = {}
    with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
        entries = bundle.infolist()
        checked(len(entries) == count, 'artifact entry count mismatch')
        total = 0
        names: set[str] = set()
        # Check every declaration before decompressing any member.
        for entry in entries:
            name = member_name(entry.filename)
            checked(entry.orig_filename == name and name not in names,
                    'duplicate or truncated artifact member')
            names.add(name)
            checked(not entry.is_dir() and
                    stat.S_IFMT(entry.external_attr >> 16) == stat.S_IFREG,
                    'non-regular artifact member')
            checked(entry.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED) and
                    entry.flag_bits in (0, 8) and not entry.extra and not entry.comment and
                    entry.extract_version <= 20 and entry.volume == 0, 'unsupported artifact member encoding')
            checked(0 <= entry.file_size <= MAX_FILE_BYTES and
                    0 <= entry.compress_size <= MAX_ARCHIVE_BYTES, 'artifact member byte limit')
            total += entry.file_size
            checked(total <= MAX_TOTAL_BYTES, 'artifact expanded byte limit')
        local_records(raw, entries)
        for entry in entries:
            offset = entry.header_offset + 30 + len(entry.filename)
            payload = raw[offset:offset + entry.compress_size]
            if entry.compress_type == zipfile.ZIP_STORED:
                checked(entry.compress_size == entry.file_size, 'stored artifact size mismatch')
                data = payload
            else:
                stream = zlib.decompressobj(-15)
                data = stream.decompress(payload, MAX_FILE_BYTES + 1)
                checked(stream.eof and not stream.unused_data and not stream.unconsumed_tail,
                        'incomplete or trailing artifact deflate stream')
            checked(len(data) == entry.file_size and zlib.crc32(data) == entry.CRC,
                    'artifact member length or CRC mismatch')
            files[entry.filename] = data
    return files
