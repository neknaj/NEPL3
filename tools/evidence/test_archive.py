"""Untrusted archive data must never execute or replace an existing destination."""
import io
import os
from pathlib import Path
import stat
import struct
from types import TracebackType
from typing import BinaryIO
import tempfile
import unittest
from unittest.mock import patch
import warnings
import zipfile

from tools.evidence import archive, test_runner
from tools.evidence.records import Exited
from tools.evidence.runner import digest as hash_bytes, run, verify


class ArchiveTests(unittest.TestCase):
    def fixture(self, root: Path, code: str = 'print("evidence")') -> tuple[Path, str, dict[str, bytes]]:
        repo, spec, output = test_runner.RunnerTests().fixture(root, code)
        _ = run(repo, spec, output)
        revision = verify(output).source_revision
        return output, revision, archive.snapshot(output, revision)

    def raw(self, entries: list[tuple[str, bytes]], *, mode: int = stat.S_IFREG,
            compression: int = zipfile.ZIP_STORED) -> bytes:
        output = io.BytesIO()
        with warnings.catch_warnings():
            warnings.simplefilter('ignore', UserWarning)
            with zipfile.ZipFile(output, 'w') as bundle:
                for name, data in entries:
                    info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                    info.create_system = 3
                    info.external_attr = (mode | 0o644) << 16
                    info.compress_type = compression
                    bundle.writestr(info, data)
        return output.getvalue()

    def test_deterministic_roundtrip_preserves_raw_bundle_without_execution(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, files = self.fixture(root, 'import sys; sys.stdout.buffer.write(b"x\\0y"); print("err", file=sys.stderr)')
            first, second = root / 'one.zip', root / 'two.zip'
            digest = archive.pack(source, first, revision)
            self.assertEqual(archive.pack(source, second, revision), digest)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            with patch('subprocess.run', side_effect=AssertionError('restore executed a command')):
                result = archive.restore(first, root / 'restored', digest, revision)
            self.assertEqual(archive.snapshot(root / 'restored', revision), files)
            self.assertEqual(archive.snapshot(source, revision), files)
            self.assertEqual(result.commands[0].outcome, Exited(0))
            self.assertFalse(result.representation()['acceptance_decision'])

    def test_selected_digest_and_revision_are_required_before_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, _ = self.fixture(root)
            path = root / 'bundle.zip'
            digest = archive.pack(source, path, revision)
            for expected, selected in [('0' * 64, revision), (digest, '0' * 40), ('bad', revision), (digest, 'bad')]:
                with self.subTest(expected=expected, selected=selected):
                    with self.assertRaises(ValueError):
                        _ = archive.restore(path, root / 'out', expected, selected)
                    self.assertFalse((root / 'out').exists())

    def test_existing_destinations_are_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, _ = self.fixture(root)
            path = root / 'bundle.zip'
            digest = archive.pack(source, path, revision)
            original = path.read_bytes()
            with self.assertRaises(FileExistsError):
                _ = archive.pack(source, path, revision)
            self.assertEqual(path.read_bytes(), original)
            output = root / 'out'; output.mkdir()
            _ = (output / 'mine').write_bytes(b'preserve')
            with self.assertRaises(ValueError):
                _ = archive.restore(path, output, digest, revision)
            self.assertEqual((output / 'mine').read_bytes(), b'preserve')

    def test_paths_duplicates_links_devices_and_compression_are_rejected(self) -> None:
        for name in ['../probe.stdout', '/probe.stdout', 'dir/probe.stdout', 'dir\\probe.stdout',
                     'C:probe.stdout', 'Probe.stdout', 'con.stdout', 'nul.stderr', 'lpt1.stdout',
                     'probe.stdout.', 'probe.stdout\0hidden']:
            raw = self.raw([(name, b'x')])
            with self.subTest(name=name), self.assertRaises(ValueError):
                _ = archive.unpack(raw, hash_bytes(raw), 'a' * 40)
        variants = [
            self.raw([('probe.stdout', b'a'), ('probe.stdout', b'b')]),
            self.raw([('probe.stdout', b'target')], mode=stat.S_IFLNK),
            self.raw([('probe.stdout', b'')], mode=stat.S_IFDIR),
            self.raw([('probe.stdout', b'')], mode=stat.S_IFCHR),
            self.raw([('probe.stdout', b'x' * 100)], compression=zipfile.ZIP_DEFLATED),
        ]
        for raw in variants:
            with self.assertRaises(ValueError):
                _ = archive.unpack(raw, hash_bytes(raw), 'a' * 40)

    def test_manifest_missing_extra_hash_and_json_failures_leave_no_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            _, revision, files = self.fixture(root)
            cases = [dict(files) for _ in range(5)]
            del cases[0]['probe.stderr']
            cases[1]['extra.stdout'] = b'extra'
            cases[2]['probe.stdout'] = b'changed'
            cases[3]['manifest.json'] = b'{"version":1,"version":1}'
            cases[4]['spec.json'] = b'{}'
            for i, case in enumerate(cases):
                raw = archive.encode(case)
                path = root / f'bad-{i}.zip'; _ = path.write_bytes(raw)
                with self.assertRaises((ValueError, KeyError)):
                    _ = archive.restore(path, root / 'out', hash_bytes(raw), revision)
                self.assertFalse((root / 'out').exists())

    def test_central_directory_is_bounded_before_zip_parsing(self) -> None:
        raw = bytearray(self.raw([('probe.stdout', b'x')]))
        for field, value, width in [(-12, archive.MAX_FILES + 1, 2), (-10, 1_000_000, 4)]:
            changed = bytearray(raw)
            changed[field:field + width] = value.to_bytes(width, 'little')
            data = bytes(changed)
            with patch('zipfile.ZipFile', side_effect=AssertionError('unbounded central directory')):
                with self.assertRaises(ValueError):
                    _ = archive.unpack(data, hash_bytes(data), 'a' * 40)

    def test_limits_are_inclusive_and_noncanonical_bytes_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            _, revision, files = self.fixture(Path(temp).resolve())
            raw = archive.encode(files)
            digest = hash_bytes(raw)
            total = sum(map(len, files.values()))
            with patch.object(archive, 'MAX_FILE_BYTES', max(map(len, files.values()))), patch.object(archive, 'MAX_TOTAL_BYTES', total):
                self.assertEqual(archive.unpack(raw, digest, revision), files)
            for attr, value in [('MAX_FILES', len(files) - 1), ('MAX_FILE_BYTES', max(map(len, files.values())) - 1), ('MAX_TOTAL_BYTES', total - 1), ('MAX_ARCHIVE_BYTES', len(raw) - 1)]:
                with patch.object(archive, attr, value), self.assertRaises(ValueError):
                    _ = archive.unpack(raw, digest, revision)
            for changed in [raw + b'trailing', b'prefix' + raw]:
                with self.assertRaises(ValueError):
                    _ = archive.unpack(changed, hash_bytes(changed), revision)

    def test_linked_source_and_output_ancestor_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, _ = self.fixture(root)
            path = root / 'bundle.zip'; digest = archive.pack(source, path, revision)
            link = root / 'linked'
            try:
                link.symlink_to(source, target_is_directory=True)
            except OSError:
                self.skipTest('symlink privilege unavailable')
            with self.assertRaises(ValueError):
                _ = archive.pack(link, root / 'bad.zip', revision)
            with self.assertRaises(ValueError):
                _ = archive.restore(path, link / 'out', digest, revision)
            self.assertFalse((source / 'out').exists())
            raw = source / 'probe.stdout'; raw.unlink()
            raw.symlink_to(root / 'bundle.zip')
            with self.assertRaises(ValueError):
                _ = archive.pack(source, root / 'bad.zip', revision)

    def test_restore_verification_failure_preserves_unaccepted_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, files = self.fixture(root)
            path = root / 'bundle.zip'; digest = archive.pack(source, path, revision)
            with patch.object(archive, 'snapshot', side_effect=ValueError('forced restore validation failure')):
                with self.assertRaises(ValueError):
                    _ = archive.restore(path, root / 'out', digest, revision)
            self.assertTrue((root / 'out').is_dir())
            self.assertEqual(archive.snapshot(source, revision), files)

    def test_failed_execution_is_transportable_but_never_promoted(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, _ = self.fixture(root, 'raise SystemExit(7)')
            path = root / 'failed.zip'; digest = archive.pack(source, path, revision)
            report = archive.restore(path, root / 'out', digest, revision)
            self.assertEqual(report.commands[0].outcome, Exited(7))
            self.assertFalse(report.representation()['acceptance_decision'])


    def test_zip64_locator_cannot_override_preparse_bounds(self) -> None:
        original = archive.encode({f'p{i}.stdout': b'' for i in range(300)})
        footer = original[-22:]
        size = int.from_bytes(footer[12:16], 'little')
        offset = int.from_bytes(footer[16:20], 'little')
        base = original[:-22]
        record = struct.pack('<4sQ2H2I4Q', b'PK\x06\x06', 44, 45, 45, 0, 0, 300, 300, size, offset)
        locator = struct.pack('<4sIQI', b'PK\x06\x07', 0, len(base), 1)
        fake = struct.pack('<4s4H2IH', b'PK\x05\x06', 0, 0, 1, 1, 76, len(base), 0)
        raw = base + record + locator + fake
        with patch('zipfile.ZipFile', side_effect=AssertionError('ZIP64 reached parser')):
            with self.assertRaisesRegex(ValueError, 'ZIP64'):
                _ = archive.unpack(raw, hash_bytes(raw), 'a' * 40)

    def test_replaced_destination_is_not_deleted_on_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, files = self.fixture(root)
            path = root / 'bundle.zip'; digest = archive.pack(source, path, revision)
            output = root / 'out'; moved = root / 'original-output'
            def replace_output(_path: Path, _revision: str) -> dict[str, bytes]:
                _ = output.rename(moved)
                output.mkdir()
                _ = (output / 'foreign-sentinel').write_bytes(b'preserve')
                raise ValueError('destination replaced during final verification')
            with patch.object(archive, 'snapshot', side_effect=replace_output):
                with self.assertRaises(ValueError):
                    _ = archive.restore(path, output, digest, revision)
            self.assertEqual((output / 'foreign-sentinel').read_bytes(), b'preserve')
            self.assertEqual(archive.snapshot(moved, revision), files)
            self.assertEqual(archive.snapshot(source, revision), files)

    def test_pack_write_and_close_failures_preserve_unaccepted_output(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, files = self.fixture(root)
            original_open = Path.open
            for phase in ['write', 'close']:
                output = root / f'{phase}.zip'
                class FaultingWriter:
                    def __init__(self, stream: BinaryIO) -> None:
                        self.stream: BinaryIO = stream
                    def __enter__(self) -> 'FaultingWriter':
                        return self
                    def write(self, data: bytes) -> int:
                        if phase == 'write':
                            _ = self.stream.write(data[:5])
                            raise OSError('simulated full device')
                        return self.stream.write(data)
                    def __exit__(self, _kind: type[BaseException] | None, _error: BaseException | None,
                                 _trace: TracebackType | None) -> None:
                        self.stream.close()
                        if phase == 'close':
                            raise OSError('simulated close failure')
                def failing_open(path: Path, mode: str = 'r') -> object:
                    if path == output and mode == 'xb':
                        return FaultingWriter(original_open(path, 'xb'))
                    return original_open(path, 'rb')
                with patch.object(Path, 'open', failing_open):
                    with self.assertRaises(OSError):
                        _ = archive.pack(source, output, revision)
                self.assertTrue(output.is_file())
                self.assertEqual(archive.snapshot(source, revision), files)
                _ = archive.pack(source, root / f'{phase}-retry.zip', revision)


    def test_hardlinked_inputs_are_rejected_without_modification(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            source, revision, _ = self.fixture(root)
            path = root / 'bundle.zip'; expected = archive.pack(source, path, revision)
            linked = root / 'linked.zip'
            try:
                os.link(path, linked)
            except OSError:
                self.skipTest('hardlink support unavailable')
            original = path.read_bytes()
            with self.assertRaises(ValueError):
                _ = archive.restore(linked, root / 'out', expected, revision)
            self.assertFalse((root / 'out').exists())
            self.assertEqual(path.read_bytes(), original)
            os.link(source / 'probe.stdout', root / 'linked.stdout')
            with self.assertRaises(ValueError):
                _ = archive.pack(source, root / 'bad.zip', revision)
            self.assertEqual((source / 'probe.stdout').read_bytes(), b'evidence\n')

    def test_legacy_verify_keeps_filewise_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            source, revision, _ = self.fixture(Path(temp).resolve())
            with patch('tools.evidence.runner.verify_files', side_effect=AssertionError('materialized whole bundle')):
                self.assertEqual(verify(source).source_revision, revision)

    def test_declared_count_cannot_hide_additional_directory_entries(self) -> None:
        raw = bytearray(archive.encode({f'p{i}.stdout': b'' for i in range(300)}))
        raw[-14:-12] = (1).to_bytes(2, 'little')
        raw[-12:-10] = (1).to_bytes(2, 'little')
        data = bytes(raw)
        with patch('zipfile.ZipFile', side_effect=AssertionError('hidden entries reached parser')):
            with self.assertRaises(ValueError):
                _ = archive.unpack(data, hash_bytes(data), 'a' * 40)


    def test_directory_record_widths_and_signatures_are_checked_before_parser(self) -> None:
        original = archive.encode({'probe.stdout': b'x'})
        offset = int.from_bytes(original[-6:-2], 'little')
        for defect in ['signature', 'name', 'extra', 'comment', 'count']:
            raw = bytearray(original)
            if defect == 'signature':
                raw[offset:offset + 4] = b'BAD!'
            elif defect == 'count':
                raw[-14:-12] = (2).to_bytes(2, 'little')
                raw[-12:-10] = (2).to_bytes(2, 'little')
            else:
                at = {'name': 28, 'extra': 30, 'comment': 32}[defect]
                value = archive.MAX_NAME_BYTES + 1 if defect == 'name' else 1
                raw[offset + at:offset + at + 2] = value.to_bytes(2, 'little')
            data = bytes(raw)
            with self.subTest(defect=defect), patch('zipfile.ZipFile', side_effect=AssertionError('bad directory reached parser')):
                with self.assertRaises(ValueError):
                    _ = archive.unpack(data, hash_bytes(data), 'a' * 40)

def stat_with(value: os.stat_result, changes: dict[str, int]) -> os.stat_result:
    # Tuple fields omit nanosecond and platform-specific attributes. Preserve
    # all named metadata, including Windows reparse flags, then vary one field.
    tuple_fields = {'st_mode', 'st_ino', 'st_dev', 'st_nlink', 'st_uid', 'st_gid', 'st_size'}
    named: dict[str, object] = {key: getattr(value, key) for key in dir(value) if key.startswith('st_') and key not in tuple_fields}
    named.update(changes)
    result = os.stat_result(value, named)
    for key in dir(value):
        if key.startswith('st_') and key not in changes:
            if getattr(result, key) != getattr(value, key):
                raise AssertionError(f'mock dropped {key}')
    if not archive.unchanged(result, result):
        raise AssertionError('mock metadata is not self-consistent')
    return result


class ReadFileMetadataTests(unittest.TestCase):
    def test_path_and_handle_timestamps_may_use_different_windows_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'probe.stdout'
            _ = path.write_bytes(b'evidence')
            original_fstat = os.fstat
            def handle_metadata(fd: int) -> os.stat_result:
                value = original_fstat(fd)
                return stat_with(value, {'st_ctime_ns': value.st_ctime_ns + 1000})
            with patch('os.fstat', side_effect=handle_metadata):
                self.assertEqual(archive.read_file(path, 8), b'evidence')

    def test_handle_mutation_during_read_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'probe.stdout'
            _ = path.write_bytes(b'evidence')
            value = path.stat()
            for field in ['st_mtime_ns', 'st_ctime_ns']:
                changed = stat_with(value, {field: getattr(value, field) + 1})
                with self.subTest(field=field), patch('os.fstat', side_effect=[value, changed]):
                    with self.assertRaises(ValueError):
                        _ = archive.read_file(path, 8)

    def test_path_mutation_after_open_or_read_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'probe.stdout'
            _ = path.write_bytes(b'evidence')
            value = path.stat()
            for field in ['st_mtime_ns', 'st_ctime_ns']:
                changed = stat_with(value, {field: getattr(value, field) + 1})
                for observations in [[value, changed], [value, value, changed]]:
                    with self.subTest(field=field, count=len(observations)), patch.object(Path, 'lstat', side_effect=observations):
                        with self.assertRaises(ValueError):
                            _ = archive.read_file(path, 8)
