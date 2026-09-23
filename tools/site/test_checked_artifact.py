import json
from pathlib import Path
import tempfile
import unittest

from checked_artifact import validate
from payload import digest, pack
from tools.serialization.json import decode, object_value


class CheckedArtifactTests(unittest.TestCase):
    def fixture(self, root: Path) -> str:
        site = root / 'site'
        site.mkdir()
        files = {'index.html': b'<h1>Document</h1>', '.nojekyll': b'',
                 'build.json': json.dumps({'source_commit': 'a' * 40,
                                           'base_path': '/NEPL3/'}).encode()}
        for name, data in files.items():
            _ = (site / name).write_bytes(data)
        manifest = json.dumps({'version': 1, 'files': [
            {'path': name, 'bytes': len(data), 'sha256': digest(data)}
            for name, data in files.items()]}).encode()
        _ = (site / 'manifest.json').write_bytes(manifest)
        identity = digest(manifest)
        receipt = pack(site, identity, root / 'pages.tar')
        _ = (root / 'pages-receipt.json').write_text(json.dumps(receipt.representation()), encoding='utf-8')
        _ = (root / 'site-results.json').write_text(json.dumps({
            'result': 'passed', 'source_commit': 'a' * 40,
            'manifest_sha256': identity}), encoding='utf-8')
        return identity

    def test_original_payload_and_rejection_boundaries(self) -> None:
        # Expectations derive from immutable CI input, not a rebuilt site.
        for mutation in ('none', 'commit', 'site', 'tar', 'report'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                identity = self.fixture(root)
                commit = 'a' * 40
                if mutation == 'commit':
                    commit = 'b' * 40
                elif mutation == 'site':
                    _ = (root / 'site/index.html').write_bytes(b'changed')
                elif mutation == 'tar':
                    with (root / 'pages.tar').open('ab') as stream:
                        _ = stream.write(b'changed')
                elif mutation == 'report':
                    report = dict(object_value(decode((root / 'site-results.json').read_bytes())))
                    report['result'] = 'failed'
                    _ = (root / 'site-results.json').write_text(json.dumps(report), encoding='utf-8')
                if mutation == 'none':
                    self.assertEqual(validate(root, commit), identity)
                else:
                    with self.assertRaises(ValueError):
                        _ = validate(root, commit)
