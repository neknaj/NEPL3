import io
import json
import os
import subprocess
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from payload import digest, pack, snapshot
from tools.serialization.json import decode, object_value, array


class PayloadTests(unittest.TestCase):
    def test_manifest_version_requires_integer_one(self) -> None:
        for version in [True, 1.0, '1', None, 2]:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as directory:
                parent = Path(directory).resolve(); root = parent / 'site'
                _, _ = self.fixture(root)
                manifest = dict(object_value(decode((root / 'manifest.json').read_bytes())))
                manifest['version'] = version
                data = json.dumps(manifest).encode('utf-8')
                _ = (root / 'manifest.json').write_bytes(data)
                with self.assertRaisesRegex(ValueError, 'unsupported manifest'):
                    _ = pack(root, digest(data), parent / 'out.tar')
                self.assertFalse((parent / 'out.tar').exists())

    @unittest.skipUnless(os.name == 'nt', 'Windows junction regression')
    def test_windows_junction_is_rejected_without_following_it(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, _ = self.fixture(root)
            target = parent / 'external'; target.mkdir()
            sentinel = target / 'keep'; _ = sentinel.write_bytes(b'keep')
            link = root / 'junction'
            _ = subprocess.run(['powershell', '-NoProfile', '-Command',
                            'New-Item -ItemType Junction -Path $env:NEPL_LINK -Target $env:NEPL_TARGET | Out-Null'],
                           env={**os.environ, 'NEPL_LINK': str(link), 'NEPL_TARGET': str(target)}, check=True)
            try:
                with self.assertRaisesRegex(ValueError, 'linked directory'):
                    _ = pack(root, identity, parent / 'out.tar')
                self.assertFalse((parent / 'out.tar').exists())
            finally:
                # Remove only this junction entry; never recursively traverse it.
                os.rmdir(link)
            self.assertEqual(sentinel.read_bytes(), b'keep')

    def test_input_limits_and_output_location(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, files = self.fixture(root)
            total = sum(map(len, files.values()))
            with patch('payload.MAX_BYTES', total):
                _ = pack(root, identity, parent / 'exact.tar')
            with patch('payload.MAX_BYTES', total - 1):
                with self.assertRaisesRegex(ValueError, 'byte limit'):
                    _ = pack(root, identity, parent / 'over.tar')
            with patch('payload.MAX_FILES', 3):
                with self.assertRaisesRegex(ValueError, 'file limit'):
                    _ = pack(root, identity, parent / 'files.tar')
            with patch('payload.MAX_FILES', 4):
                self.assertEqual(snapshot(root, identity), files)
                for index in range(4):
                    (root / str(index)).mkdir()
                self.assertEqual(snapshot(root, identity), files)
                (root / 'excess').mkdir()
                with self.assertRaisesRegex(ValueError, 'entry limit'):
                    _ = snapshot(root, identity)
            with self.assertRaisesRegex(ValueError, 'outside input'):
                _ = pack(root, identity, root / 'output.tar')
            self.assertFalse((root / 'output.tar').exists())

    def test_directory_read_errors_propagate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, _ = self.fixture(root)
            with patch('os.scandir', side_effect=PermissionError('denied')):
                with self.assertRaises(PermissionError):
                    _ = pack(root, identity, parent / 'out.tar')
            self.assertFalse((parent / 'out.tar').exists())

    def fixture(self, root: Path) -> tuple[str, dict[str, bytes]]:
        root.mkdir()
        files = {'index.html': b'<h1>Checked document</h1>', 'build.json': b'{}', '.nojekyll': b''}
        for name, data in files.items():
            _ = (root / name).write_bytes(data)
        manifest = json.dumps({'version': 1, 'files': [
            {'path': name, 'bytes': len(data), 'sha256': digest(data)} for name, data in files.items()
        ]}).encode('utf-8')
        _ = (root / 'manifest.json').write_bytes(manifest)
        files['manifest.json'] = manifest
        return digest(manifest), files

    def test_identical_bytes_and_regular_members_without_clock_or_owner(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, files = self.fixture(root)
            a = pack(root, identity, parent / 'a.tar')
            os.utime(root / 'index.html', (100000, 100000))
            b = pack(root, identity, parent / 'b.tar')
            self.assertEqual(a, b)
            self.assertEqual((parent / 'a.tar').read_bytes(), (parent / 'b.tar').read_bytes())
            with tarfile.open(fileobj=io.BytesIO((parent / 'a.tar').read_bytes()), mode='r:') as archive:
                self.assertEqual(archive.getnames(), sorted(files))
                for member in archive:
                    self.assertTrue(member.isfile())
                    self.assertEqual((member.mtime, member.uid, member.gid, member.mode), (0, 0, 0, 0o644))
                    stream = archive.extractfile(member)
                    self.assertIsNotNone(stream)
                    if stream is None:
                        self.fail('regular tar member has no data stream')
                    self.assertEqual(stream.read(), files[member.name])
            self.assertFalse(a.representation()['publication_verified'])

    def test_corruption_and_unlisted_files_do_not_create_output(self) -> None:
        for corruption in ['digest', 'file', 'extra', 'missing', 'duplicate', 'traversal']:
            with self.subTest(corruption=corruption), tempfile.TemporaryDirectory() as directory:
                parent = Path(directory).resolve(); root = parent / 'site'
                identity, _ = self.fixture(root)
                if corruption == 'digest': identity = '0' * 64
                if corruption == 'file': _ = (root / 'index.html').write_bytes(b'changed')
                if corruption == 'extra': _ = (root / 'extra').write_bytes(b'extra')
                if corruption == 'missing': (root / 'index.html').unlink()
                if corruption in ['duplicate', 'traversal']:
                    manifest = dict(object_value(decode((root / 'manifest.json').read_bytes())))
                    records = list(array(manifest['files']))
                    if corruption == 'duplicate': records.append(records[0])
                    else:
                        record = dict(object_value(records[0]))
                        record['path'] = '../escape'
                        records[0] = record
                    manifest['files'] = records
                    data = json.dumps(manifest).encode('utf-8')
                    _ = (root / 'manifest.json').write_bytes(data); identity = digest(data)
                with self.assertRaises(ValueError): _ = pack(root, identity, parent / 'out.tar')
                self.assertFalse((parent / 'out.tar').exists())

    def test_existing_output_and_hard_links_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, _ = self.fixture(root)
            output = parent / 'out.tar'; _ = output.write_bytes(b'keep')
            with self.assertRaises(ValueError): _ = pack(root, identity, output)
            self.assertEqual(output.read_bytes(), b'keep')
            os.link(root / 'index.html', parent / 'hard-link')
            with self.assertRaises(ValueError): _ = pack(root, identity, parent / 'other.tar')
            self.assertFalse((parent / 'other.tar').exists())


if __name__ == '__main__':
    _ = unittest.main()
