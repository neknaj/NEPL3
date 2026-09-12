import gzip
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

from payload import digest, pack, tar_bytes
from recovery import verify
import test_payload


class RecoveryTests(unittest.TestCase):
    def test_preserved_real_doc_tar_keeps_its_original_identity(self):
        root = Path(__file__).resolve().parents[2]
        data = gzip.decompress((root / 'conformance/results/doc-pages-payload/final-review/identical-tars.tar.gz.fixture').read_bytes())
        # Identities were independently recorded for the actual 14-chapter
        # site before this recovery implementation existed (PR105/PR106).
        result = verify(data, 'cc053272f49f8d4d79fc756560df5401bfdc22b8c1e1a1177ae0250cdc265cd9',
                        '39000dd5b7aad48deae97741c5b077b44a242a0badb7b30e81b3c1c26f301446')
        self.assertIs(result.data, data)
        self.assertEqual((result.files, len(result.data)), (22, 1177600))

    def test_file_directory_and_reserved_manifest_collisions_are_rejected(self):
        for extra in [{'assets': b'x', 'assets/a.css': b'y'},
                      {'MANIFEST.JSON': b'x'}, {'manifest.json/a': b'x'}]:
            with self.subTest(paths=list(extra)), tempfile.TemporaryDirectory() as directory:
                _, _, files, _ = self.source(directory)
                files.pop('manifest.json')
                files.update(extra)
                manifest = json.dumps(dict(version=1, files=[dict(path=n, bytes=len(b), sha256=digest(b))
                                                              for n, b in files.items()])).encode()
                files['manifest.json'] = manifest
                data = tar_bytes(files)
                with self.assertRaisesRegex(ValueError, 'colli'):
                    verify(data, digest(data), digest(manifest))

    def source(self, directory):
        parent = Path(directory).resolve()
        root = parent / 'site'
        identity, files = test_payload.PayloadTests().fixture(root)
        receipt = pack(root, identity, parent / 'original.tar')
        return (parent / 'original.tar').read_bytes(), identity, files, receipt

    def altered(self, data, change):
        out = io.BytesIO()
        with tarfile.open(fileobj=io.BytesIO(data), mode='r:') as original, tarfile.open(fileobj=out, mode='w', format=tarfile.PAX_FORMAT) as target:
            for member in original:
                value = original.extractfile(member).read()
                if member.name == 'index.html':
                    if change == 'link': member.type = tarfile.SYMTYPE; member.linkname = '../outside'; member.size = 0
                    if change == 'traversal': member.name = '../outside'
                    if change == 'metadata': member.mtime = 10
                    if change == 'content': value = b'x' * len(value)
                target.addfile(member, io.BytesIO(value) if member.isfile() else None)
                if change == 'duplicate' and member.name == 'index.html': target.addfile(member, io.BytesIO(value))
        return out.getvalue()

    def test_original_bytes_survive_validation_without_rebuilding(self):
        with tempfile.TemporaryDirectory() as directory:
            data, identity, files, receipt = self.source(directory)
            value = verify(data, receipt['tar_sha256'], identity)
            self.assertIs(value.data, data)
            self.assertEqual(value.files, len(files))

    def test_identity_mismatch_and_compression_wrapper_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            data, identity, _, _ = self.source(directory)
            with self.assertRaisesRegex(ValueError, 'saved tar digest'): verify(data, '0' * 64, identity)
            with self.assertRaisesRegex(ValueError, 'manifest digest'): verify(data, digest(data), '0' * 64)
            compressed = gzip.compress(data, mtime=0)
            with self.assertRaises(tarfile.ReadError): verify(compressed, digest(compressed), identity)

    def test_changed_archive_is_rejected_even_with_matching_outer_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            original, identity, _, _ = self.source(directory)
            for change in ['link', 'traversal', 'metadata', 'content', 'duplicate']:
                data = self.altered(original, change)
                with self.subTest(change=change), self.assertRaises(ValueError): verify(data, digest(data), identity)
            for suffix in [b'\0' * 512, original]:
                data = original + suffix
                with self.subTest(trailing=len(suffix)), self.assertRaisesRegex(ValueError, 'noncanonical'):
                    verify(data, digest(data), identity)

    def test_member_count_is_bounded_before_reading_content(self):
        with tempfile.TemporaryDirectory() as directory:
            data, identity, _, _ = self.source(directory)
            with patch('recovery.MAX_FILES', 1), self.assertRaisesRegex(ValueError, 'excessive tar'):
                verify(data, digest(data), identity)


if __name__ == '__main__': unittest.main()
