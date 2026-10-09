"""Bounded outer Actions ZIP inspection; no extraction or acceptance decision."""
from dataclasses import dataclass
from datetime import datetime
import io
import stat
import zipfile
import zlib

from tools.evidence import archive
from tools.evidence.actions import MAX_DOWNLOAD_BYTES, SelectedDownload, Selection, select_download
from tools.evidence.archive import checked
from tools.evidence.runner import digest

MAX_MEMBERS = archive.MAX_FILES + 2
MAX_PATH_BYTES = archive.MAX_NAME_BYTES + len('a01-command/')
MAX_EXPANDED_BYTES = archive.MAX_ARCHIVE_BYTES + archive.MAX_TOTAL_BYTES + 65


@dataclass(frozen=True, slots=True)
class InspectedBundle:
    transport: SelectedDownload
    # Preserve the original inner bytes. No canonical re-encoding is returned.
    inner_archive: bytes


def member_limit(path: str) -> int:
    if path == 'a01.zip':
        return archive.MAX_ARCHIVE_BYTES
    if path == 'a01.zip.sha256':
        return 65
    prefix = 'a01-command/'
    checked(path.startswith(prefix), 'unexpected Actions member')
    _ = archive.name(path[len(prefix):])
    return archive.MAX_FILE_BYTES


def directory(raw: bytes) -> int:
    """Bound actual directory records before ZipFile allocates its member list."""
    checked(22 <= len(raw) <= MAX_DOWNLOAD_BYTES, 'Actions container byte limit')
    end = raw[-22:]
    checked(end[:4] == b'PK\x05\x06' and end[4:8] == b'\0' * 4 and end[20:] == b'\0\0',
            'unsupported Actions ZIP footer')
    checked(len(raw) < 42 or raw[-42:-38] != b'PK\x06\x07', 'Actions ZIP64 is unsupported')
    count = int.from_bytes(end[10:12], 'little')
    checked(0 < count <= MAX_MEMBERS and int.from_bytes(end[8:10], 'little') == count,
            'Actions member count limit')
    size = int.from_bytes(end[12:16], 'little')
    offset = int.from_bytes(end[16:20], 'little')
    checked(size <= MAX_MEMBERS * (46 + MAX_PATH_BYTES) and offset + size == len(raw) - 22,
            'Actions directory byte limit')
    cursor = offset
    for _ in range(count):
        checked(cursor + 46 <= offset + size, 'truncated Actions directory')
        header = raw[cursor:cursor + 46]
        checked(header[:4] == b'PK\x01\x02', 'invalid Actions directory record')
        width = int.from_bytes(header[28:30], 'little')
        checked(0 < width <= MAX_PATH_BYTES and header[30:36] == b'\0' * 6,
                'unsupported Actions name/extra/comment/disk')
        cursor += 46 + width
        checked(cursor <= offset + size, 'truncated Actions member name')
    checked(cursor == offset + size, 'Actions actual directory count mismatch')
    return count


def contents(raw: bytes, entry: zipfile.ZipInfo, cursor: int, boundary: int) -> tuple[bytes, int]:
    checked(entry.header_offset == cursor and cursor + 30 <= boundary, 'Actions local entry overlap or gap')
    header = raw[cursor:cursor + 30]
    checked(header[:4] == b'PK\x03\x04', 'invalid Actions local header')
    checked(entry.extract_version in (10, 20) and entry.reserved == 0 and
            int.from_bytes(header[4:6], 'little') == entry.extract_version, 'unsupported Actions ZIP version')
    width = int.from_bytes(header[26:28], 'little')
    checked(width == len(entry.filename) and header[28:30] == b'\0\0', 'Actions local name/extra mismatch')
    start = cursor + 30 + width
    checked(start <= boundary and raw[cursor + 30:start] == entry.filename.encode('ascii'), 'Actions local name mismatch')
    checked(int.from_bytes(header[6:8], 'little') == entry.flag_bits and
            int.from_bytes(header[8:10], 'little') == entry.compress_type, 'Actions local encoding mismatch')
    expected = entry.CRC.to_bytes(4, 'little') + entry.compress_size.to_bytes(4, 'little') + entry.file_size.to_bytes(4, 'little')
    streamed = bool(entry.flag_bits & 8)
    checked(header[14:26] == (b'\0' * 12 if streamed else expected), 'Actions local size/CRC mismatch')
    end = start + entry.compress_size
    checked(end <= boundary, 'truncated Actions compressed member')
    compressed = raw[start:end]
    if entry.compress_type == zipfile.ZIP_STORED:
        data = compressed
    else:
        decoder = zlib.decompressobj(-15)
        data = decoder.decompress(compressed, entry.file_size + 1)
        checked(decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail,
                'Actions deflate size or framing mismatch')
    checked(len(data) == entry.file_size and zlib.crc32(data) == entry.CRC, 'Actions expanded size/CRC mismatch')
    if streamed:
        checked(end + 16 <= boundary and raw[end:end + 16] == b'PK\x07\x08' + expected,
                'Actions data descriptor mismatch')
        end += 16
    return data, end


def inspect(artifact_metadata: bytes, run_metadata: bytes, raw: bytes,
            selection: Selection, now: datetime) -> InspectedBundle:
    """Select transport then check all uploaded bytes against the inner bundle."""
    transport = select_download(artifact_metadata, run_metadata, raw, selection, now)
    count = directory(raw)
    files: dict[str, bytes] = {}
    total = 0
    try:
        with zipfile.ZipFile(io.BytesIO(raw)) as container:
            entries = container.infolist()
            checked(len(entries) == count, 'Actions parsed member count mismatch')
            cursor = 0
            boundary = int.from_bytes(raw[-6:-2], 'little')
            for entry in sorted(entries, key=lambda item: item.header_offset):
                path = entry.filename
                checked(entry.orig_filename == path and path not in files, 'duplicate or truncated Actions member')
                limit = member_limit(path)
                kind = stat.S_IFMT(entry.external_attr >> 16)
                checked(not entry.is_dir() and kind in (0, stat.S_IFREG) and not entry.external_attr & 0x10,
                        'non-regular Actions member')
                checked(entry.flag_bits & ~0x808 == 0 and entry.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED)
                        and not entry.extra and not entry.comment, 'unsupported Actions member encoding')
                checked(0 <= entry.file_size <= limit and 0 <= entry.compress_size <= len(raw),
                        'Actions member byte limit')
                total += entry.file_size
                checked(total <= MAX_EXPANDED_BYTES, 'Actions expanded byte limit')
                data, cursor = contents(raw, entry, cursor, boundary)
                files[path] = data
            checked(cursor == boundary, 'Actions hidden entry or trailing payload')
    except (zipfile.BadZipFile, NotImplementedError, RuntimeError, zlib.error) as error:
        raise ValueError('invalid Actions ZIP member') from error
    checked('a01.zip' in files and 'a01.zip.sha256' in files, 'missing Actions inner archive')
    inner = files['a01.zip']
    checked(files['a01.zip.sha256'] == (selection.inner_sha256 + '\n').encode('ascii'),
            'Actions inner digest sidecar mismatch')
    # The independent expected digest, not the sidecar, selects accepted bytes.
    checked(digest(inner) == selection.inner_sha256, 'Actions inner digest mismatch')
    raw_files = archive.unpack(inner, selection.inner_sha256, selection.source_revision)
    expected = {'a01.zip', 'a01.zip.sha256'} | {'a01-command/' + key for key in raw_files}
    checked(set(files) == expected, 'Actions raw file set mismatch')
    for key, data in raw_files.items():
        checked(files['a01-command/' + key] == data, 'Actions raw file bytes mismatch')
    return InspectedBundle(transport, inner)
