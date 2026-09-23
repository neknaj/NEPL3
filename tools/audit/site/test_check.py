"""Reject corrupted static artifacts before starting a browser."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import subprocess
from dataclasses import replace
from collections.abc import Sequence
from check import verify, expected_inputs, shared_markdown_paths
import records
from tools.serialization.json import JsonValue


class ArtifactRejection(unittest.TestCase):
    def test_shared_markdown_provenance_is_not_inferred_from_a_route(self) -> None:
        raw, html = b'# Guide', b'<h1>Guide</h1>'
        source_hash, output_hash = [hashlib.sha256(b).hexdigest() for b in (raw, html)]
        registry = (records.Reference('guide', 'doc/spec/23-guide.md', 'sources/23-guide.md'),)
        markdown = (records.MarkdownPage('doc/spec/23-guide.md', 'docs/spec/23-guide.html', source_hash),)
        receipt = records.ReferenceReceipt('guide', 'doc/spec/23-guide.md', 'doc/spec/23-guide.md',
            'docs/spec/23-guide.html', output_hash, len(html),
            records.Projection(source_hash, 'nepl3-tools.site-markdown/1; pulldown-cmark/0.13.4',
                               '/NEPL3/', 'fixture', 'docs/spec/23-guide.html', output_hash))
        def check(receipts: Sequence[records.ReferenceReceipt]) -> set[str]:
            return shared_markdown_paths(registry, receipts, markdown, '/NEPL3/', 'fixture', lambda _: raw, lambda _: html)
        self.assertEqual(check([receipt]), {'docs/spec/23-guide.html'})
        assert receipt.projection is not None
        for corrupt in (replace(receipt, projection=replace(receipt.projection, source_sha256='incorrect')), replace(receipt, sha256='incorrect'),
                        replace(receipt, route='incorrect'), replace(receipt, input='incorrect')):
            with self.subTest(receipt=corrupt), self.assertRaises(AssertionError):
                _ = check([corrupt])
        with self.assertRaises(AssertionError):
            _ = check([receipt, receipt])
        with self.assertRaises(AssertionError):
            _ = check([])

    def test_expected_checkout_rejects_stale_missing_or_mixed_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'doc').mkdir()
            (root / 'design').mkdir()
            _ = (root / 'design/tasks.json').write_text('{"design":"test-design"}', encoding='utf-8')
            _ = (root / 'config.json').write_text('{"version":1,"base_path":"/NEPL3/"}', encoding='utf-8')
            _ = (root / 'intro.nepld').write_bytes(b'original input')
            _ = (root / 'README.md').write_bytes(b'# Overview')
            registry = [{'id': 'intro', 'source': 'intro.nepld', 'route': 'docs/intro.html'}]
            _ = (root / 'doc/canonical.json').write_text(json.dumps({'pages': registry}), encoding='utf-8')
            _ = subprocess.run(['git', 'init', '-q'], cwd=root, check=True)
            _ = subprocess.run(['git', 'add', '.'], cwd=root, check=True)
            _ = subprocess.run(['git', '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                            'commit', '-qm', 'fixture'], cwd=root, check=True)
            commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
            renderer = root / 'renderer'
            _ = renderer.write_bytes(b'fixture renderer identity')
            build = records.Build(commit, '/NEPL3/', 'test-design',
                records.Overview('README.md', hashlib.sha256(b'# Overview').hexdigest(), 'pulldown-cmark/0.13.4'),
                hashlib.sha256(renderer.read_bytes()).hexdigest())
            doc = (records.PageReceipt('intro', 'intro.nepld', 'docs/intro.html',
                                       hashlib.sha256(b'original input').hexdigest()),)
            docs = {'index.html', 'docs/index.html', 'docs/intro.html', 'examples/index.html'}
            expected_inputs(build, commit, doc, docs, root, 'config.json', renderer)
            failures = (
                ('commit', replace(build, source_commit='0'*40), commit, doc, docs),
                ('manifest', build, 'f'*40, doc, docs),
                ('base', replace(build, base_path='/wrong/'), commit, doc, docs),
                ('route', build, commit, doc, docs - {'docs/intro.html'}),
                ('source', build, commit, (replace(doc[0], source_sha256='0'*64),), docs),
                ('duplicate', build, commit, doc + doc, docs),
                ('renderer', replace(build, renderer_sha256='0'*64), commit, doc, docs),
                ('design', replace(build, design='old'), commit, doc, docs))
            for failure, b, m, d, routes in failures:
                with self.subTest(failure=failure), self.assertRaises(AssertionError):
                    expected_inputs(b, m, d, routes, root, 'config.json', renderer)
            # The capability restriction is now checked by the JSON boundary.
            invalid: dict[str, JsonValue] = dict(capability='interactive', runtime_identity=None)
            with self.assertRaisesRegex(AssertionError, 'runtime capability'):
                _ = records.build(invalid)

    def test_empty_site_and_duplicate_manifest_records_are_rejected(self) -> None:
        for duplicate in [False, True]:
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                content = b'{"base_path":"/NEPL3/"}'
                _ = (root / 'build.json').write_bytes(content)
                record = {'path': 'build.json', 'bytes': len(content),
                          'sha256': hashlib.sha256(content).hexdigest()}
                records = [record, record] if duplicate else [record]
                _ = (root / 'manifest.json').write_text(json.dumps({'files': records}), encoding='utf-8')
                message = 'duplicate manifest file' if duplicate else 'empty HTML site'
                with self.assertRaisesRegex(AssertionError, message):
                    _ = verify(root)

    def test_modified_bytes_missing_resource_and_missing_fragment(self) -> None:
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
                records: list[dict[str, JsonValue]] = []
                for name, content in data.items():
                    _ = (root / name).write_bytes(content)
                    records.append({'path': name, 'bytes': len(content),
                                    'sha256': hashlib.sha256(content).hexdigest()})
                _ = (root / 'manifest.json').write_text(json.dumps({'files': records}), encoding='utf-8')
                if damage:
                    _ = (root / 'index.html').write_bytes(b'<h1>Changed</h1>')
                with self.assertRaises(AssertionError):
                    _ = verify(root)


if __name__ == '__main__':
    _ = unittest.main()
