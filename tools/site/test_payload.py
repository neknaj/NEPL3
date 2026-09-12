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


class PayloadTests(unittest.TestCase):
    @unittest.skipUnless(os.name == 'nt', 'Windows junction regression')
    def test_windows_junction_is_rejected_without_following_it(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, _ = self.fixture(root)
            target = parent / 'external'; target.mkdir()
            sentinel = target / 'keep'; sentinel.write_bytes(b'keep')
            link = root / 'junction'
            subprocess.run(['powershell', '-NoProfile', '-Command',
                            'New-Item -ItemType Junction -Path $env:NEPL_LINK -Target $env:NEPL_TARGET | Out-Null'],
                           env={**os.environ, 'NEPL_LINK': str(link), 'NEPL_TARGET': str(target)}, check=True)
            try:
                with self.assertRaisesRegex(ValueError, 'linked directory'):
                    pack(root, identity, parent / 'out.tar')
                self.assertFalse((parent / 'out.tar').exists())
            finally:
                # Remove only this junction entry; never recursively traverse it.
                os.rmdir(link)
            self.assertEqual(sentinel.read_bytes(), b'keep')

    def test_input_limits_and_output_location(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, files = self.fixture(root)
            total = sum(map(len, files.values()))
            with patch('payload.MAX_BYTES', total):
                pack(root, identity, parent / 'exact.tar')
            with patch('payload.MAX_BYTES', total - 1):
                with self.assertRaisesRegex(ValueError, 'byte limit'):
                    pack(root, identity, parent / 'over.tar')
            with patch('payload.MAX_FILES', 3):
                with self.assertRaisesRegex(ValueError, 'file limit'):
                    pack(root, identity, parent / 'files.tar')
            with patch('payload.MAX_FILES', 4):
                self.assertEqual(snapshot(root, identity), files)
                for index in range(4):
                    (root / str(index)).mkdir()
                self.assertEqual(snapshot(root, identity), files)
                (root / 'excess').mkdir()
                with self.assertRaisesRegex(ValueError, 'entry limit'):
                    snapshot(root, identity)
            with self.assertRaisesRegex(ValueError, 'outside input'):
                pack(root, identity, root / 'output.tar')
            self.assertFalse((root / 'output.tar').exists())

    def test_directory_read_errors_propagate(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, _ = self.fixture(root)
            with patch('os.scandir', side_effect=PermissionError('denied')):
                with self.assertRaises(PermissionError):
                    pack(root, identity, parent / 'out.tar')
            self.assertFalse((parent / 'out.tar').exists())

    def fixture(self, root):
        root.mkdir()
        files = {'index.html': b'<h1>Checked document</h1>', 'build.json': b'{}', '.nojekyll': b''}
        for name, data in files.items():
            (root / name).write_bytes(data)
        manifest = json.dumps({'version': 1, 'files': [
            {'path': name, 'bytes': len(data), 'sha256': digest(data)} for name, data in files.items()
        ]}).encode('utf-8')
        (root / 'manifest.json').write_bytes(manifest)
        files['manifest.json'] = manifest
        return digest(manifest), files

    def test_identical_bytes_and_regular_members_without_clock_or_owner(self):
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
                    self.assertEqual(archive.extractfile(member).read(), files[member.name])
            self.assertFalse(a['publication_verified'])

    def test_corruption_and_unlisted_files_do_not_create_output(self):
        for corruption in ['digest', 'file', 'extra', 'missing', 'duplicate', 'traversal']:
            with self.subTest(corruption=corruption), tempfile.TemporaryDirectory() as directory:
                parent = Path(directory).resolve(); root = parent / 'site'
                identity, _ = self.fixture(root)
                if corruption == 'digest': identity = '0' * 64
                if corruption == 'file': (root / 'index.html').write_bytes(b'changed')
                if corruption == 'extra': (root / 'extra').write_bytes(b'extra')
                if corruption == 'missing': (root / 'index.html').unlink()
                if corruption in ['duplicate', 'traversal']:
                    manifest = json.loads((root / 'manifest.json').read_text(encoding='utf-8'))
                    if corruption == 'duplicate': manifest['files'].append(manifest['files'][0])
                    else: manifest['files'][0]['path'] = '../escape'
                    data = json.dumps(manifest).encode('utf-8')
                    (root / 'manifest.json').write_bytes(data); identity = digest(data)
                with self.assertRaises(ValueError): pack(root, identity, parent / 'out.tar')
                self.assertFalse((parent / 'out.tar').exists())

    def test_existing_output_and_hard_links_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory).resolve(); root = parent / 'site'
            identity, _ = self.fixture(root)
            output = parent / 'out.tar'; output.write_bytes(b'keep')
            with self.assertRaises(ValueError): pack(root, identity, output)
            self.assertEqual(output.read_bytes(), b'keep')
            os.link(root / 'index.html', parent / 'hard-link')
            with self.assertRaises(ValueError): pack(root, identity, parent / 'other.tar')
            self.assertFalse((parent / 'other.tar').exists())


if __name__ == '__main__':
    unittest.main()
