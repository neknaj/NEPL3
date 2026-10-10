"""Synthetic ZIP boundary tests; no execution or acceptance claims."""
import io
import stat
import unittest
import warnings
import zipfile
import zlib
from unittest.mock import patch

from tools.evidence import artifact
from tools.evidence.runner import digest


def bundle(rows: list[tuple[str, bytes]], *, mode: int = stat.S_IFREG | 0o644,
           method: int = zipfile.ZIP_DEFLATED) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, 'w') as archive:
        for name, data in rows:
            entry = zipfile.ZipInfo(name)
            entry.create_system = 3
            entry.external_attr = mode << 16
            entry.compress_type = method
            archive.writestr(entry, data)
    return output.getvalue()


class ArtifactTests(unittest.TestCase):
    def reject(self, raw: bytes) -> None:
        with self.assertRaises((ValueError, zipfile.BadZipFile, NotImplementedError, zlib.error)):
            _ = artifact.unpack(raw, digest(raw))

    def test_original_raw_bytes_and_empty_channels(self) -> None:
        rows = [('run.stdout', '𠮷\r\n文書'.encode()), ('run.stderr', b''), ('fixture.wasm', b'\0asm\xff')]
        for mode in (stat.S_IFREG | 0o644, stat.S_IFREG | 0o666):
            for method in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
                raw = bundle(rows, mode=mode, method=method)
                self.assertEqual(artifact.unpack(raw, digest(raw)), dict(rows))

    def test_digest_pin(self) -> None:
        raw = bundle([('run.stdout', b'actual')])
        for pin in ('', '0' * 64, 'A' * 64):
            with self.assertRaises(ValueError):
                _ = artifact.unpack(raw, pin)

    def test_unsafe_names_and_member_kinds(self) -> None:
        for name in ('../a', '/a', 'a/b', 'a\\b', 'a..b', 'CON.log', 'con.log', 'lpt1.log',
                     'run.stdout.', 'Run.stdout', 'a' * 97):
            self.reject(bundle([(name, b'x')]))
        raw = bundle([('axb', b'x')]).replace(b'axb', b'a\0b')
        self.reject(raw)
        for mode in (stat.S_IFLNK | 0o777, stat.S_IFDIR | 0o755, stat.S_IFIFO | 0o600):
            self.reject(bundle([('run.stdout', b'x')], mode=mode))
        with warnings.catch_warnings():
            warnings.simplefilter('ignore', UserWarning)
            self.reject(bundle([('run.stdout', b'a'), ('run.stdout', b'b')]))

    def test_limits_before_zip_allocation(self) -> None:
        original = bundle([('run.stdout', b'x')])
        for count in (0, 2, 257, 65535):
            raw = bytearray(original)
            raw[-14:-12] = count.to_bytes(2, 'little')
            raw[-12:-10] = count.to_bytes(2, 'little')
            with patch('tools.evidence.artifact.zipfile.ZipFile', side_effect=AssertionError('allocated')):
                self.reject(bytes(raw))
        self.reject(original[:-1])
        self.reject(original + b'comment')
        self.reject(bundle([('run.stdout', b'x' * (artifact.MAX_FILE_BYTES + 1))]))

    def test_streaming_descriptor_and_inconsistent_local_headers(self) -> None:
        original = bundle([('run.stdout', b'actual')])
        central = original.index(b'PK\x01\x02')
        local = bytearray(original[:central])
        descriptor = b'PK\x07\x08' + bytes(local[14:26])
        local[6:8] = (8).to_bytes(2, 'little')
        local[14:26] = b'\0' * 12
        tail = bytearray(original[central:])
        tail[8:10] = (8).to_bytes(2, 'little')
        tail[-6:-2] = (central + 16).to_bytes(4, 'little')
        raw = bytes(local) + descriptor + bytes(tail)
        self.assertEqual(artifact.unpack(raw, digest(raw)), {'run.stdout': b'actual'})
        corrupt = bytearray(raw)
        corrupt[central + 4] ^= 1
        self.reject(bytes(corrupt))
        corrupt = bytearray(original)
        corrupt[14] ^= 1
        self.reject(bytes(corrupt))
        self.reject(b'unowned' + original)

    def test_lying_uncompressed_size_and_incomplete_deflate(self) -> None:
        for method in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
            raw = bytearray(bundle([('run.stdout', b'actual')], method=method))
            central = raw.index(b'PK\x01\x02')
            crc = zlib.crc32(b'act').to_bytes(4, 'little')
            raw[14:18] = crc
            raw[22:26] = (3).to_bytes(4, 'little')
            raw[central + 16:central + 20] = crc
            raw[central + 24:central + 28] = (3).to_bytes(4, 'little')
            self.reject(bytes(raw))
        original = bundle([('run.stdout', b'actual')])
        central = original.index(b'PK\x01\x02')
        raw = bytearray(original[:central - 1] + original[central:])
        size = int.from_bytes(original[18:22], 'little') - 1
        raw[18:22] = size.to_bytes(4, 'little')
        raw[central - 1 + 20:central - 1 + 24] = size.to_bytes(4, 'little')
        raw[-6:-2] = (central - 1).to_bytes(4, 'little')
        self.reject(bytes(raw))

    def test_trailing_deflate_and_other_volume(self) -> None:
        original = bundle([('run.stdout', b'actual')])
        central = original.index(b'PK\x01\x02')
        raw = bytearray(original[:central] + b'JUNK' + original[central:])
        size = int.from_bytes(original[18:22], 'little') + 4
        raw[18:22] = size.to_bytes(4, 'little')
        raw[central + 4 + 20:central + 4 + 24] = size.to_bytes(4, 'little')
        raw[-6:-2] = (central + 4).to_bytes(4, 'little')
        self.reject(bytes(raw))
        raw = bytearray(original)
        raw[central + 34:central + 36] = (1).to_bytes(2, 'little')
        self.reject(bytes(raw))

    def test_metadata_limits_precede_decompression(self) -> None:
        raw = bundle([('a.stdout', b'abc'), ('b.stdout', b'def')])
        with patch.object(artifact, 'MAX_TOTAL_BYTES', 5), patch(
                'tools.evidence.artifact.zlib.decompressobj', side_effect=AssertionError('decompressed')):
            self.reject(raw)
        original = bytearray(raw)
        first = original.index(b'PK\x01\x02')
        second = original.index(b'PK\x01\x02', first + 4)
        original[second + 42:second + 46] = (0).to_bytes(4, 'little')
        self.reject(bytes(original))
        for index in (28, first + 30):
            changed = bytearray(raw)
            changed[index:index + 2] = (20).to_bytes(2, 'little')
            self.reject(bytes(changed))

    def test_corrupt_crc_and_encryption(self) -> None:
        original = bundle([('run.stdout', b'actual')], method=zipfile.ZIP_STORED)
        raw = bytearray(original)
        at = raw.index(b'actual')
        raw[at] ^= 1
        self.reject(bytes(raw))
        raw = bytearray(original)
        central = raw.index(b'PK\x01\x02')
        raw[central + 8:central + 10] = (1).to_bytes(2, 'little')
        self.reject(bytes(raw))


if __name__ == '__main__':
    _ = unittest.main()
