"""Reject corrupted static artifacts before starting a browser."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import subprocess
import copy
from check import verify, expected_inputs


class ArtifactRejection(unittest.TestCase):
    def test_expected_checkout_rejects_stale_missing_or_mixed_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'doc').mkdir()
            (root / 'design').mkdir()
            (root / 'design/tasks.json').write_text('{"design":"test-design"}', encoding='utf-8')
            (root / 'config.json').write_text('{"version":1,"base_path":"/NEPL3/"}', encoding='utf-8')
            (root / 'intro.nepld').write_bytes(b'original input')
            registry = [{'id': 'intro', 'source': 'intro.nepld', 'route': 'docs/intro.html'}]
            (root / 'doc/canonical.json').write_text(json.dumps({'pages': registry}), encoding='utf-8')
            subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
            subprocess.run(['git', 'add', '.'], cwd=root, check=True)
            subprocess.run(['git', '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                            'commit', '-qm', 'fixture'], cwd=root, check=True)
            commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
            renderer = root / 'renderer'; renderer.write_bytes(b'fixture renderer identity')
            build = {'source_commit': commit, 'base_path': '/NEPL3/',
                     'design': 'test-design', 'capability': 'docs-only', 'runtime_identity': None,
                     'renderer': {'executable_sha256': hashlib.sha256(renderer.read_bytes()).hexdigest()}}
            manifest = {'source_commit': commit}
            doc = {'pages': [{'id': 'intro', 'input': 'intro.nepld', 'route': 'docs/intro.html',
                              'source_sha256': hashlib.sha256(b'original input').hexdigest()}]}
            docs = dict.fromkeys(['index.html', 'docs/index.html', 'docs/intro.html'])
            expected_inputs(build, manifest, doc, docs, root, 'config.json', renderer)
            for failure in ['commit', 'manifest', 'base', 'route', 'source', 'duplicate', 'renderer', 'design', 'capability']:
                b, m, d, pages = copy.deepcopy((build, manifest, doc, docs))
                if failure == 'commit': b['source_commit'] = '0' * 40
                if failure == 'manifest': m['source_commit'] = 'f' * 40
                if failure == 'base': b['base_path'] = '/wrong/'
                if failure == 'route': pages.pop('docs/intro.html')
                if failure == 'source': d['pages'][0]['source_sha256'] = '0' * 64
                if failure == 'duplicate': d['pages'].append(d['pages'][0])
                if failure == 'renderer': b['renderer']['executable_sha256'] = '0' * 64
                if failure == 'design': b['design'] = 'old'
                if failure == 'capability': b['capability'] = 'interactive'
                with self.subTest(failure=failure), self.assertRaises(AssertionError):
                    expected_inputs(b, m, d, pages, root, 'config.json', renderer)

    def test_empty_site_and_duplicate_manifest_records_are_rejected(self):
        for duplicate in [False, True]:
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                content = b'{"base_path":"/NEPL3/"}'
                (root / 'build.json').write_bytes(content)
                record = {'path': 'build.json', 'bytes': len(content),
                          'sha256': hashlib.sha256(content).hexdigest()}
                records = [record, record] if duplicate else [record]
                (root / 'manifest.json').write_text(json.dumps({'files': records}), encoding='utf-8')
                message = 'duplicate manifest file' if duplicate else 'empty HTML site'
                with self.assertRaisesRegex(AssertionError, message):
                    verify(root)

    def test_modified_bytes_missing_resource_and_missing_fragment(self):
        for html, damage in [
            ('<h1>Original</h1>', True),
            ('<link rel="stylesheet" href="missing.css">', False),
            ('<a href="index.html#missing">Link</a>', False),
            ('<script>0</script>', False),
        ]:
            with self.subTest(html=html), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                data = {
                    'index.html': html.encode('utf-8'),
                    'build.json': b'{"base_path":"/NEPL3/"}',
                }
                records = []
                for name, content in data.items():
                    (root / name).write_bytes(content)
                    records.append({'path': name, 'bytes': len(content),
                                    'sha256': hashlib.sha256(content).hexdigest()})
                (root / 'manifest.json').write_text(json.dumps({'files': records}), encoding='utf-8')
                if damage:
                    (root / 'index.html').write_bytes(b'<h1>Changed</h1>')
                with self.assertRaises(AssertionError):
                    verify(root)


if __name__ == '__main__':
    unittest.main()
