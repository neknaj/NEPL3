import json
from pathlib import Path
import tempfile
import unittest

from checked_artifact import validate
from payload import digest, pack


class CheckedArtifactTests(unittest.TestCase):
    def fixture(self, root):
        site = root / 'site'
        site.mkdir()
        files = {'index.html': b'<h1>Document</h1>', '.nojekyll': b'',
                 'build.json': json.dumps({'source_commit': 'a' * 40,
                                           'base_path': '/NEPL3/'}).encode()}
        for name, data in files.items():
            (site / name).write_bytes(data)
        manifest = json.dumps({'version': 1, 'files': [
            {'path': name, 'bytes': len(data), 'sha256': digest(data)}
            for name, data in files.items()]}).encode()
        (site / 'manifest.json').write_bytes(manifest)
        identity = digest(manifest)
        receipt = pack(site, identity, root / 'pages.tar')
        (root / 'pages-receipt.json').write_text(json.dumps(receipt), encoding='utf-8')
        (root / 'site-results.json').write_text(json.dumps({
            'result': 'passed', 'source_commit': 'a' * 40,
            'manifest_sha256': identity}), encoding='utf-8')
        return identity

    def test_original_payload_and_rejection_boundaries(self):
        # Expectations derive from immutable CI input, not a rebuilt site.
        for mutation in ('none', 'commit', 'site', 'tar', 'report'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                identity = self.fixture(root)
                commit = 'a' * 40
                if mutation == 'commit':
                    commit = 'b' * 40
                elif mutation == 'site':
                    (root / 'site/index.html').write_bytes(b'changed')
                elif mutation == 'tar':
                    with (root / 'pages.tar').open('ab') as stream:
                        stream.write(b'changed')
                elif mutation == 'report':
                    report = json.loads((root / 'site-results.json').read_bytes())
                    report['result'] = 'failed'
                    (root / 'site-results.json').write_text(json.dumps(report), encoding='utf-8')
                if mutation == 'none':
                    self.assertEqual(validate(root, commit), identity)
                else:
                    with self.assertRaises(ValueError):
                        validate(root, commit)
