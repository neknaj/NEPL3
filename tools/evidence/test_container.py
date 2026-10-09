"""Synthetic streamed outer ZIPs exercise transport, paths and raw equality."""
from dataclasses import replace
from datetime import datetime
import io
from pathlib import Path
import stat
import tempfile
from typing import override
import unittest
from unittest.mock import patch
import warnings
import zipfile
import zlib

from tools.evidence import archive, container, test_actions, test_archive
from tools.evidence.actions import Selection
from tools.evidence.runner import digest


class Stream(io.BytesIO):
    @override
    def seekable(self) -> bool:
        return False

    @override
    def seek(self, offset: int, whence: int = 0) -> int:
        raise io.UnsupportedOperation('streamed ZIP fixture')


def encode(files: list[tuple[str, bytes]], mode: int = stat.S_IFREG,
           compression: int = zipfile.ZIP_DEFLATED, streamed: bool = True) -> bytes:
    output = Stream() if streamed else io.BytesIO()
    with warnings.catch_warnings():
        warnings.simplefilter('ignore', UserWarning)
        with zipfile.ZipFile(output, 'w', compression=compression) as result:
            for key, data in files:
                info = zipfile.ZipInfo(key)
                info.compress_type = compression
                info.external_attr = (mode | 0o644) << 16
                result.writestr(info, data)
    return output.getvalue()


class ContainerTests(unittest.TestCase):
    def fixture(self, root: Path) -> tuple[Selection, list[tuple[str, bytes]], datetime]:
        _, revision, files = test_archive.ArchiveTests().fixture(root)
        inner = archive.encode(files)
        selected, _, _, _, now = test_actions.fixture()
        selected = replace(selected, source_revision=revision, inner_sha256=digest(inner))
        outer = [('a01.zip', inner), ('a01.zip.sha256', (digest(inner) + '\n').encode())]
        outer.extend(('a01-command/' + key, value) for key, value in files.items())
        return selected, outer, now

    def inspect(self, raw: bytes, selected: Selection, now: datetime) -> container.InspectedBundle:
        _, artifact, run, _, _ = test_actions.fixture()
        artifact['name'] = f'a01-command-{selected.source_revision}-2-3'
        artifact['size_in_bytes'] = len(raw); artifact['digest'] = 'sha256:' + digest(raw)
        test_actions.change(artifact, ('workflow_run', 'head_sha'), selected.source_revision)
        run['head_sha'] = selected.source_revision
        return container.inspect(test_actions.encoded(artifact), test_actions.encoded(run), raw, selected, now)

    def test_streamed_deflate_preserves_inner_bytes_without_filesystem_extraction(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            selected, files, now = self.fixture(Path(temp).resolve())
            raw = encode(files)
            with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
                self.assertTrue(all(item.flag_bits & 8 for item in bundle.infolist()))
            with patch('zipfile.ZipFile.extract', side_effect=AssertionError('extract')), \
                 patch('zipfile.ZipFile.extractall', side_effect=AssertionError('extractall')):
                result = self.inspect(raw, selected, now)
            self.assertEqual(result.inner_archive, dict(files)['a01.zip'])

    def test_missing_extra_duplicate_and_changed_raw_files_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            selected, files, now = self.fixture(Path(temp).resolve())
            cases = [files[1:], files[:-1], files + [files[-1]],
                     files + [('a01-command/extra.stdout', b'x')],
                     files[:-1] + [(files[-1][0], b'changed')],
                     [(key, b'0' * 64 + b'\n' if key.endswith('.sha256') else data) for key, data in files]]
            for case in cases:
                with self.subTest(names=[key for key, _ in case]), self.assertRaises(ValueError):
                    _ = self.inspect(encode(case), selected, now)

    def test_inner_digest_is_independent_of_outer_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            selected, files, now = self.fixture(Path(temp).resolve())
            changed = replace(selected, inner_sha256='0' * 64)
            files[1] = ('a01.zip.sha256', b'0' * 64 + b'\n')
            with self.assertRaisesRegex(ValueError, 'inner digest mismatch'):
                _ = self.inspect(encode(files), changed, now)

    def test_paths_links_and_directory_entries_fail(self) -> None:
        selected, _, _, _, now = test_actions.fixture()
        for key in ['../a01.zip', '/a01.zip', 'a01-command/../probe.stdout', 'a01-command\\probe.stdout',
                    'a01-command/probe.stdout\0suffix', 'a01-command/', 'a01-command/NUL.stdout']:
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = self.inspect(encode([(key, b'')]), selected, now)
        for mode in [stat.S_IFLNK, stat.S_IFCHR, stat.S_IFDIR]:
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                _ = self.inspect(encode([('a01.zip', b'x')], mode), selected, now)

    def test_actual_directory_count_is_bounded_before_zipfile(self) -> None:
        raw = bytearray(encode([(f'a01-command/p{i}.stdout', b'') for i in range(3)]))
        raw[-14:-12] = (1).to_bytes(2, 'little'); raw[-12:-10] = (1).to_bytes(2, 'little')
        selected, _, _, _, now = test_actions.fixture()
        with patch('zipfile.ZipFile', side_effect=AssertionError('unbounded parser')):
            with self.assertRaisesRegex(ValueError, 'actual directory count'):
                _ = self.inspect(bytes(raw), selected, now)

    def test_expansion_limits_precede_member_open(self) -> None:
        raw = encode([('a01-command/probe.stdout', b'x' * 2048)])
        selected, _, _, _, now = test_actions.fixture()
        with patch.object(archive, 'MAX_FILE_BYTES', 1024), \
             patch.object(container, 'contents', side_effect=AssertionError('unbounded member')):
            with self.assertRaisesRegex(ValueError, 'member byte limit'):
                _ = self.inspect(raw, selected, now)

    def test_total_limit_and_container_footer_reject(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            selected, files, now = self.fixture(Path(temp).resolve())
            raw = encode(files)
            total = sum(len(data) for _, data in files)
            with patch.object(container, 'MAX_EXPANDED_BYTES', total):
                _ = self.inspect(raw, selected, now)
            with patch.object(container, 'MAX_EXPANDED_BYTES', total - 1), self.assertRaises(ValueError):
                _ = self.inspect(raw, selected, now)
            for changed in [raw + b'junk', raw[:-1], b'junk' + raw]:
                with self.assertRaises(ValueError):
                    _ = self.inspect(changed, selected, now)

    def test_local_header_descriptor_and_overlap_disagreements_fail(self) -> None:
        raw = encode([('a01-command/probe.stdout', b'evidence')])
        with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
            entry = bundle.infolist()[0]
        boundary = int.from_bytes(raw[-6:-2], 'little')
        start = 30 + len(entry.filename)
        end = start + entry.compress_size
        self.assertEqual(container.contents(raw, entry, 0, boundary)[0], b'evidence')
        for offset in [0, 4, 5, 6, 8, 14, 26, 28, 30, start, end, end + 4, end + 8, end + 12]:
            changed = bytearray(raw); changed[offset] ^= 1
            with self.subTest(offset=offset), self.assertRaises((ValueError, zlib.error)):
                _ = container.contents(bytes(changed), entry, 0, boundary)
        with self.assertRaisesRegex(ValueError, 'overlap or gap'):
            _ = container.contents(raw, entry, 1, boundary)

    def test_actual_deflate_output_cannot_hide_behind_claimed_size(self) -> None:
        raw = encode([('a01-command/probe.stdout', b'x' * 100000)])
        with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
            entry = bundle.infolist()[0]
        boundary = int.from_bytes(raw[-6:-2], 'little')
        entry.file_size = 1
        with self.assertRaisesRegex(ValueError, 'deflate size or framing'):
            _ = container.contents(raw, entry, 0, boundary)

    def test_zip64_locator_extra_comment_and_count_rejected_before_parser(self) -> None:
        raw = encode([('a01-command/probe.stdout', b'')])
        offset = int.from_bytes(raw[-6:-2], 'little')
        variants: list[bytes] = []
        for index in [offset, offset + 28, offset + 30, offset + 32, offset + 34]:
            modified = bytearray(raw); modified[index] ^= 1; variants.append(bytes(modified))
        variants.append(raw[:-22] + b'PK\x06\x07' + b'\0' * 16 + raw[-22:])
        selected, _, _, _, now = test_actions.fixture()
        for modified in variants:
            with patch('zipfile.ZipFile', side_effect=AssertionError('unbounded parser')):
                with self.assertRaises(ValueError):
                    _ = self.inspect(modified, selected, now)

    def test_unsupported_central_version_fails(self) -> None:
        raw = encode([('a01-command/probe.stdout', b'x')])
        offset = int.from_bytes(raw[-6:-2], 'little')
        selected, _, _, _, now = test_actions.fixture()
        for version in [9, 21, 45, 255]:
            changed = bytearray(raw)
            changed[4:6] = version.to_bytes(2, 'little')
            changed[offset + 6:offset + 8] = version.to_bytes(2, 'little')
            with self.subTest(version=version), self.assertRaises(ValueError):
                _ = self.inspect(bytes(changed), selected, now)

    def test_stored_deflate_streamed_and_seekable_positive_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            selected, files, now = self.fixture(Path(temp).resolve())
            for compression in [zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED]:
                for streamed in [False, True]:
                    with self.subTest(compression=compression, streamed=streamed):
                        result = self.inspect(encode(files, compression=compression, streamed=streamed), selected, now)
                        self.assertEqual(result.inner_archive, dict(files)['a01.zip'])

    def test_hidden_gap_before_directory_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            selected, files, now = self.fixture(Path(temp).resolve())
            raw = encode(files)
            offset = int.from_bytes(raw[-6:-2], 'little')
            changed = bytearray(raw[:offset] + b'hidden' + raw[offset:])
            changed[-6:-2] = (offset + 6).to_bytes(4, 'little')
            with self.assertRaisesRegex(ValueError, 'hidden entry or trailing payload'):
                _ = self.inspect(bytes(changed), selected, now)

    def test_raw_nul_name_is_rejected_before_missing_bundle_checks(self) -> None:
        raw = bytearray(encode([('a01-command/probe.stdout', b'x')]))
        offset = int.from_bytes(raw[-6:-2], 'little')
        raw[30 + len('a01-command/probe')] = 0
        raw[offset + 46 + len('a01-command/probe')] = 0
        selected, _, _, _, now = test_actions.fixture()
        with self.assertRaisesRegex(ValueError, 'duplicate or truncated Actions member'):
            _ = self.inspect(bytes(raw), selected, now)

    def test_trailing_or_concatenated_deflate_payload_fails(self) -> None:
        raw = encode([('a01-command/probe.stdout', b'evidence')])
        with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
            entry = bundle.infolist()[0]
        boundary = int.from_bytes(raw[-6:-2], 'little')
        original_size = entry.compress_size
        end = 30 + len(entry.filename) + original_size
        encoder = zlib.compressobj(wbits=-15)
        second_stream = encoder.compress(b'hidden') + encoder.flush()
        for suffix in [b'junk', second_stream]:
            entry.compress_size = original_size + len(suffix)
            changed = raw[:end] + suffix + raw[end:]
            with self.assertRaisesRegex(ValueError, 'deflate size or framing'):
                _ = container.contents(changed, entry, 0, boundary + len(suffix))
