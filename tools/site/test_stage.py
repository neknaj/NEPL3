import gzip
from pathlib import Path
import tempfile
import unittest

from stage import stage


class StageTests(unittest.TestCase):
    def test_original_tar_copy_and_failure_preservation(self):
        original = gzip.decompress((Path(__file__).resolve().parents[2] /
            'conformance/fixtures/site/pages.tar.gz.fixture').read_bytes())
        tar = 'cc053272f49f8d4d79fc756560df5401bfdc22b8c1e1a1177ae0250cdc265cd9'
        manifest = '39000dd5b7aad48deae97741c5b077b44a242a0badb7b30e81b3c1c26f301446'
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            source = root/'pages.tar'; source.write_bytes(original)
            output = root/'artifact.tar'
            with self.assertRaises(ValueError): stage(source, output, '0'*64, manifest)
            self.assertFalse(output.exists())
            result = stage(source, output, tar, manifest)
            self.assertEqual(output.read_bytes(), original)
            self.assertFalse(result['publication_verified'])
            with self.assertRaises(FileExistsError): stage(source, output, tar, manifest)
            self.assertEqual(output.read_bytes(), original)
            source.write_bytes(original+b'x')
            with self.assertRaises(ValueError): stage(source, root/'invalid.tar', tar, manifest)
            self.assertFalse((root/'invalid.tar').exists())
