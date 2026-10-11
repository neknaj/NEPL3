"""Synthetic raw-log restoration boundaries, never acceptance evidence."""
from dataclasses import replace
from pathlib import Path
import tempfile
import os
import unittest
from unittest.mock import patch
from tools.evidence.retrieval.metadata import timestamp
from tools.evidence.retrieval.network import Download
from tools.evidence.retrieval.pin import Pin
from tools.evidence.retrieval.restore import Request, restore
from tools.evidence.retrieval.test_metadata import fixture, raw
from tools.evidence.runner import digest
from tools.evidence.test_artifact import bundle


def package(data: bytes = b'raw\r\n\xff') -> tuple[Pin, Download, bytes]:
    pin, artifact, run = fixture()
    archive = bundle([('run.stdout', data)])
    pin = replace(pin, archive_sha256=digest(archive), archive_bytes=len(archive))
    artifact['digest'] = 'sha256:' + pin.archive_sha256
    artifact['size_in_bytes'] = pin.archive_bytes
    return pin, Download(raw(artifact), raw(run), archive), data


class RestoreTests(unittest.TestCase):
    def test_exact_bytes_and_unchanged_status(self) -> None:
        pin, result, data = package()
        calls: list[int] = []
        sources: list[str] = []
        def download(selected: Pin, token: str) -> Download:
            self.assertEqual(token, 'fixture-token')
            calls.append(selected.artifact_id)
            return result
        with tempfile.TemporaryDirectory() as name, patch('tools.evidence.retrieval.restore.time.time',
                                                         return_value=timestamp('2026-10-10T12:03:00Z')):
            root = Path(name).resolve()
            _ = (root / 'implementation-status.json').write_bytes(b'unchanged synthetic status')
            requests = [Request('dist/evidence/a.log', digest(data), pin), Request('dist/evidence/b.log', digest(data), pin)]
            self.assertEqual(restore(root, requests, 'fixture-token', sources.append, download), 2)
            self.assertEqual((root / requests[0].destination).read_bytes(), data)
            self.assertEqual((root / requests[1].destination).read_bytes(), data)
            self.assertEqual(calls, [pin.artifact_id])
            self.assertIn(pin.source_commit, sources)
            self.assertEqual((root / 'implementation-status.json').read_bytes(), b'unchanged synthetic status')
            _ = restore(root, requests, 'fixture-token', sources.append, download)
            self.assertEqual(len(calls), 2)  # Existing matching logs never skip artifact retrieval.

    def test_local_logs_keep_uppercase_underscore_and_unicode_paths(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            for filename in ('E01_native.log', '日本語_01.log', 'a' * 100 + '.log'):
                target = root / 'dist/evidence' / filename
                target.parent.mkdir(parents=True, exist_ok=True)
                _ = target.write_bytes(b'original local log')
                request = Request('dist/evidence/' + filename, digest(target.read_bytes()), None)
                self.assertEqual(restore(root, [request], '', lambda _: None), 1)

    def test_file_directory_prefix_conflict_creates_nothing(self) -> None:
        pin, result, data = package()
        with tempfile.TemporaryDirectory() as name, patch('tools.evidence.retrieval.restore.time.time',
                                                         return_value=timestamp('2026-10-10T12:03:00Z')):
            root = Path(name).resolve()
            requests = [Request('dist/evidence/a.log', digest(data), pin),
                        Request('dist/evidence/a.log/b.log', digest(data), pin)]
            with self.assertRaises(ValueError):
                _ = restore(root, requests, '', lambda _: None, lambda _p, _t: result)
            self.assertFalse((root / 'dist').exists())

    def test_conflicting_aliases_and_repeated_pins_create_nothing(self) -> None:
        first, first_package, first_data = package(b'first')
        second, second_package, second_data = package(b'second')
        second = replace(second, artifact_id=4)
        from tools.serialization.json import decode, object_value
        metadata = dict(object_value(decode(second_package.artifact)))
        metadata['id'] = 4
        metadata['url'] = second.artifact_url
        metadata['archive_download_url'] = second.artifact_url + '/zip'
        second_package = replace(second_package, artifact=raw(metadata))
        def download(pin: Pin, _token: str) -> Download:
            return first_package if pin.artifact_id == first.artifact_id else second_package
        with tempfile.TemporaryDirectory() as name, patch('tools.evidence.retrieval.restore.time.time',
                                                         return_value=timestamp('2026-10-10T12:03:00Z')):
            root = Path(name).resolve()
            for left, right in [('same.log', 'same.log'), ('A.log', 'a.log'), ('é.log', 'e\u0301.log')]:
                requests = [Request('dist/evidence/' + left, digest(first_data), first),
                            Request('dist/evidence/' + right, digest(second_data), second)]
                with self.assertRaises(ValueError):
                    _ = restore(root, requests, '', lambda _: None, download)
                self.assertFalse((root / 'dist').exists())
            requests = [Request('dist/evidence/a.log', digest(first_data), first),
                        Request('dist/evidence/b.log', digest(first_data), replace(first, archive_sha256='0' * 64))]
            with self.assertRaises(ValueError):
                _ = restore(root, requests, '', lambda _: None, download)
            self.assertFalse((root / 'dist').exists())

    def test_symlink_ancestor_and_leaf_reject_without_changes(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            (root / 'outside').mkdir()
            (root / 'dist').mkdir()
            try:
                (root / 'dist/evidence').symlink_to(root / 'outside', target_is_directory=True)
            except OSError:
                self.skipTest('symlink privilege unavailable')
            request = Request('dist/evidence/a.log', digest(b'raw'), None)
            with self.assertRaises(ValueError):
                _ = restore(root, [request], '', lambda _: None)
            self.assertEqual(list((root / 'outside').iterdir()), [])
            (root / 'dist/evidence').unlink()
            (root / 'dist/evidence').mkdir()
            _ = (root / 'original.log').write_bytes(b'raw')
            (root / 'dist/evidence/a.log').symlink_to(root / 'original.log')
            with self.assertRaises(ValueError):
                _ = restore(root, [request], '', lambda _: None)
            self.assertEqual((root / 'original.log').read_bytes(), b'raw')

    def test_hardlinked_log_is_not_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            (root / 'dist/evidence').mkdir(parents=True)
            original = root / 'original.log'
            _ = original.write_bytes(b'raw')
            try:
                os.link(original, root / 'dist/evidence/a.log')
            except OSError:
                self.skipTest('hardlink support unavailable')
            with self.assertRaises(ValueError):
                _ = restore(root, [Request('dist/evidence/a.log', digest(b'raw'), None)], '', lambda _: None)
            self.assertEqual(original.read_bytes(), b'raw')

    def test_invalid_attempt_creates_no_output(self) -> None:
        pin, result, data = package()
        with tempfile.TemporaryDirectory() as name, patch('tools.evidence.retrieval.restore.time.time',
                                                         return_value=timestamp('2026-10-10T12:03:00Z')):
            root = Path(name).resolve()
            requests = [Request('dist/evidence/a.log', digest(data), pin),
                        Request('dist/evidence/b.log', '0' * 64, pin)]
            with self.assertRaises(ValueError):
                _ = restore(root, requests, '', lambda _: None, lambda _pin, _token: result)
            self.assertFalse((root / 'dist').exists())
            def stale(_: str) -> None:
                raise ValueError('source mismatch')
            with self.assertRaisesRegex(ValueError, 'source mismatch'):
                _ = restore(root, requests[:1], '', stale, lambda _pin, _token: self.fail('downloaded stale source'))
            self.assertFalse((root / 'dist').exists())

    def test_paths_conflicts_missing_and_empty_logs(self) -> None:
        pin, result, data = package()
        with tempfile.TemporaryDirectory() as name, patch('tools.evidence.retrieval.restore.time.time',
                                                         return_value=timestamp('2026-10-10T12:03:00Z')):
            root = Path(name).resolve()
            for path in ('../outside.log', 'dist/evidence/../outside.log', 'dist/evidence/con.log',
                         'dist/evidence/a.json', 'dist/evidence/C:drive.log', 'dist/evidence/a\\b.log'):
                with self.assertRaises(ValueError):
                    _ = restore(root, [Request(path, digest(data), pin)], '', lambda _: None, lambda _p, _t: result)
            target = root / 'dist/evidence/a.log'
            target.parent.mkdir(parents=True)
            _ = target.write_bytes(b'preserve existing')
            with self.assertRaises(ValueError):
                _ = restore(root, [Request('dist/evidence/a.log', digest(data), pin)], '', lambda _: None, lambda _p, _t: result)
            self.assertEqual(target.read_bytes(), b'preserve existing')
            with self.assertRaises(FileNotFoundError):
                _ = restore(root, [Request('dist/evidence/missing.log', digest(data), None)], '', lambda _: None)
            empty_pin, empty_result, empty = package(b'')
            with self.assertRaises(ValueError):
                _ = restore(root, [Request('dist/evidence/empty.log', digest(empty), empty_pin)], '', lambda _: None,
                            lambda _p, _t: empty_result)
            self.assertFalse((target.parent / 'empty.log').exists())

    def test_expiry_during_final_identity_check_prevents_output(self) -> None:
        pin, result, data = package()
        now = [timestamp('2026-10-10T12:03:00Z')]
        calls = [0]
        def identity(_: str) -> None:
            calls[0] += 1
            if calls[0] == 2:
                now[0] = pin.expires_at_unix
        with tempfile.TemporaryDirectory() as name, patch('tools.evidence.retrieval.restore.time.time', side_effect=lambda: now[0]):
            root = Path(name).resolve()
            with self.assertRaises(ValueError):
                _ = restore(root, [Request('dist/evidence/a.log', digest(data), pin)], '', identity, lambda _p, _t: result)
            self.assertFalse((root / 'dist').exists())

    def test_expired_or_unavailable_artifact_cannot_use_local_cache(self) -> None:
        pin, result, data = package()
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve()
            target = root / 'dist/evidence/a.log'
            target.parent.mkdir(parents=True)
            _ = target.write_bytes(data)
            request = Request('dist/evidence/a.log', digest(data), pin)
            with patch('tools.evidence.retrieval.restore.time.time', return_value=pin.expires_at_unix), self.assertRaises(ValueError):
                _ = restore(root, [request], '', lambda _: None, lambda _p, _t: result)
            def unavailable(_pin: Pin, _token: str) -> Download:
                raise ValueError('artifact unavailable')
            with self.assertRaisesRegex(ValueError, 'artifact unavailable'):
                _ = restore(root, [request], '', lambda _: None, unavailable)
            self.assertEqual(target.read_bytes(), data)


if __name__ == '__main__':
    _ = unittest.main()
