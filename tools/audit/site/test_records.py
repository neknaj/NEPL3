"""Manifest boundaries and a complete, minimal static-audit input."""
from dataclasses import dataclass
from collections.abc import Sequence
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from check import verify
from browser import Case
import records
from tools.serialization.json import JsonValue


def encoded(value: JsonValue) -> bytes:
    return json.dumps(value).encode('utf-8')


def identity(path: str, raw: bytes) -> dict[str, JsonValue]:
    return dict(path=path, sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw))


@dataclass(frozen=True, slots=True)
class Fixture:
    root: Path
    source: Path
    renderer: Path
    commit: str


def fixture(directory: Path, *, runtime: bool = False) -> Fixture:
    source, root = directory / 'source', directory / 'site'
    source.mkdir()
    root.mkdir()
    inputs: dict[str, bytes] = {
        'README.md': b'# Overview', 'intro.nepld': b'fixture source',
        'doc/canonical.json': encoded(dict(pages=[dict(id='intro', source='intro.nepld',
            route='docs/intro.html', projection='doc/intro.md')], files=[dict(
                id='raw', source='raw.txt', route='sources/raw.txt')])),
        'raw.txt': b'raw reference',
        'site/config.json': encoded(dict(version=1, base_path='/NEPL3/')),
        'design/tasks.json': encoded(dict(design='fixture')),
        'site/examples.json': encoded(dict(version=1, profile='design/profile.json', examples=[dict(
            id='hello', source='examples/hello.txt', language='Hello', category='Greeting')])),
        'design/profile.json': encoded(dict(id='fixture-profile')),
        'examples/hello.txt': 'hello 世界\r\n'.encode('utf-8'),
    }
    for name, raw in inputs.items():
        target = source / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = target.write_bytes(raw)
    _ = subprocess.run(['git', 'init', '-q'], cwd=source, check=True)
    _ = subprocess.run(['git', '-c', 'core.autocrlf=false', 'add', '.'], cwd=source, check=True)
    _ = subprocess.run(['git', '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
                        '-c', 'core.autocrlf=false', 'commit', '-qm', 'audit fixture'], cwd=source, check=True)
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source, text=True).strip()
    renderer = directory / 'renderer'
    _ = renderer.write_bytes(b'fixture renderer')
    files: dict[str, bytes] = {name: b'<!doctype html><title>Fixture</title>' for name in (
        'index.html', 'docs/index.html', 'docs/intro.html', 'examples/index.html')}
    files['.nojekyll'] = b''
    files['assets/site.css'] = b'body{}'
    files['sources/raw.txt'] = inputs['raw.txt']
    files['examples/sources/hello.txt'] = inputs['examples/hello.txt']
    files['examples/manifest.json'] = encoded(dict(
        source_commit=commit, capability='source-view',
        catalog_sha256=hashlib.sha256(inputs['site/examples.json']).hexdigest(),
        source_profile=dict(path='design/profile.json', id='fixture-profile',
            sha256=hashlib.sha256(inputs['design/profile.json']).hexdigest(), resolved_runtime=runtime),
        examples=[dict(id='hello', source='examples/hello.txt', language='Hello', category='Greeting',
            path='examples/sources/hello.txt', bytes=len(inputs['examples/hello.txt']),
            sha256=hashlib.sha256(inputs['examples/hello.txt']).hexdigest(), revision=commit,
            required_source_profile='fixture-profile', execution_available=False)]))
    files['markdown-manifest.json'] = encoded(dict(version=1, source_commit=commit,
                                                  renderer='pulldown-cmark/0.13.4', pages=[]))
    files['doc-manifest.json'] = encoded(dict(
        pages=[dict(id='intro', input='intro.nepld', route='docs/intro.html',
                    source_sha256=hashlib.sha256(inputs['intro.nepld']).hexdigest())],
        registered_files=[dict(id='raw', source='raw.txt', input='raw.txt', route='sources/raw.txt',
            sha256=hashlib.sha256(inputs['raw.txt']).hexdigest(), bytes=len(inputs['raw.txt']))],
        files=[identity(name, files[name]) for name in ('docs/intro.html', 'sources/raw.txt')]))
    files['build.json'] = encoded(dict(source_commit=commit, base_path='/NEPL3/', design='fixture',
        capability='docs-only', runtime_identity=None,
        overview=dict(source='README.md', sha256=hashlib.sha256(inputs['README.md']).hexdigest(),
                      renderer='pulldown-cmark/0.13.4'),
        renderer=dict(executable_sha256=hashlib.sha256(renderer.read_bytes()).hexdigest()),
        files=[identity(name, raw) for name, raw in files.items()]))
    files['manifest.json'] = encoded(dict(source_commit=commit,
                                          files=[identity(name, raw) for name, raw in files.items()]))
    for name, raw in files.items():
        target = root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = target.write_bytes(raw)
    return Fixture(root, source, renderer, commit)


class RecordTests(unittest.TestCase):
    def test_complete_manifest_chain_reaches_browser_with_normalized_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            value = fixture(Path(directory))
            calls: list[tuple[Path, str, tuple[str, ...], tuple[str, ...]]] = []
            def observed(root: Path, base: str, pages: Sequence[str], sources: Sequence[str]) -> tuple[Case, ...]:
                calls.append((root, base, tuple(pages), tuple(sources)))
                return ()
            with patch('check.observe', side_effect=observed) as browser:
                report = verify(value.root, value.source, renderer=value.renderer)
            browser.assert_called_once()
            self.assertEqual(calls[0][0:2], (value.root.resolve(), '/NEPL3/'))
            self.assertEqual(set(calls[0][2]),
                             {'index.html', 'docs/index.html', 'docs/intro.html', 'examples/index.html'})
            self.assertEqual(calls[0][3], ('hello 世界\n',))
            self.assertEqual(report.source_commit, value.commit)
            # Four pages, CSS, .nojekyll, two source files and five manifests.
            self.assertEqual(report.files, 13)
            self.assertEqual(report.representation()['cases'], [])

    def test_runtime_claim_fails_after_consistent_outer_digests(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            value = fixture(Path(directory), runtime=True)
            with patch('check.observe') as browser, self.assertRaisesRegex(AssertionError, 'runtime resolution'):
                _ = verify(value.root, value.source, renderer=value.renderer)
            browser.assert_not_called()

    def test_raw_reference_has_no_projection_source_digest(self) -> None:
        wire: JsonValue = [dict(id='raw', source='raw.txt', input='raw.txt', route='sources/raw.txt',
                                sha256='digest', bytes=4)]
        self.assertEqual(records.reference_receipts(wire),
                         (records.ReferenceReceipt('raw', 'raw.txt', 'raw.txt', 'sources/raw.txt', 'digest', 4, None),))

    def test_projection_context_and_field_types(self) -> None:
        projection: dict[str, JsonValue] = dict(renderer='renderer', context='{"base":"/","source_commit":"commit"}',
                                               output_route='guide.html', output_sha256='output')
        reference: dict[str, JsonValue] = dict(id='guide', source='guide.md', input='guide.md', route='guide.html',
            sha256='output', bytes=5, source_sha256='source', projection=projection)
        self.assertEqual(records.reference_receipts([reference])[0].projection,
                         records.Projection('source', 'renderer', '/', 'commit', 'guide.html', 'output'))
        for context in ('{"base":"/","source_commit":"commit","unexpected":1}',
                        '{"base":"/","source_commit":1}'):
            malformed: dict[str, JsonValue] = dict(reference, projection=dict(projection, context=context))
            with self.subTest(context=context), self.assertRaises((AssertionError, ValueError)):
                _ = records.reference_receipts([malformed])
        with self.assertRaises(ValueError):
            _ = records.files([dict(path='x', sha256='digest', bytes=True)], sized=True)
        with self.assertRaises(ValueError):
            _ = records.document(b'{"pages":[],"pages":[]}')
