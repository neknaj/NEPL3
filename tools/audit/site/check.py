"""Verify a production docs-only bundle and its scriptless browser routes.

This test runner requires pinned Playwright engines. Assertions must be enabled;
missing runners, digest/link mismatches and browser failures are test failures.
It tests generated static pages, not Playground or live Pages deployment.
"""
if not __debug__:
    raise RuntimeError('site audit requires Python assertions')
import argparse, hashlib, json, subprocess, re, sys
from pathlib import Path
from html.parser import HTMLParser
from urllib.parse import urlsplit, unquote, urljoin

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from browser import observe

class Document(HTMLParser):

    def __init__(self, text):
        super().__init__()
        self.ids = set()
        self.links = []
        self.scripts = 0
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'script' or any((k.startswith('on') for k in attrs)):
            self.scripts += 1
        if 'id' in attrs:
            assert attrs['id'] not in self.ids, 'duplicate id'
            self.ids.add(attrs['id'])
        for key in ('href', 'src'):
            if key in attrs:
                self.links.append(attrs[key])

def expected_inputs(build, manifest, doc_manifest, docs, source_root, config_path, renderer):
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source_root, text=True).strip()
    assert build['source_commit'] == manifest['source_commit'] == commit, 'stale source commit'
    def blob(path):
        return subprocess.check_output(['git', 'show', commit + ':' + path], cwd=source_root)
    config = json.loads(blob(config_path))
    assert config['version'] == 1 and build['base_path'] == config['base_path'], 'unexpected base'
    assert build['design'] == json.loads(blob('design/tasks.json'))['design'], 'design revision mismatch'
    assert build['overview'] == {'source': 'README.md', 'sha256': hashlib.sha256(blob('README.md')).hexdigest(),
                                 'renderer': 'pulldown-cmark/0.13.4'}, 'overview source mismatch'
    assert build['capability'] == 'docs-only' and build['runtime_identity'] is None, 'unexpected runtime capability'
    registry = json.loads(blob('doc/canonical.json'))['pages']
    assert registry, 'empty canonical registry'
    tracked = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', commit, '--', 'doc/spec/'], cwd=source_root, text=True).splitlines()
    markdown_routes = {'docs/spec/' + path[len('doc/spec/'):-3] + '.html' for path in tracked
                       if re.fullmatch(r'doc/spec/[0-9]{2}-[^/]*\.md', path) and path not in {p.get('projection') for p in registry}}
    assert docs.keys() == {page['route'] for page in registry} | markdown_routes | {'index.html', 'docs/index.html', 'examples/index.html'}, 'page coverage'
    entries = doc_manifest['pages']
    assert len(entries) == len(registry), 'source coverage'
    actual = {entry['id']: entry for entry in entries}
    assert len(actual) == len(entries), 'duplicate source id'
    for page in registry:
        entry = actual[page['id']]
        assert entry['input'] == page['source'] and entry['route'] == page['route'], 'source route mismatch'
        assert entry['source_sha256'] == hashlib.sha256(blob(page['source'])).hexdigest(), 'source digest mismatch'
    assert renderer is not None, 'renderer executable is required'
    assert hashlib.sha256(renderer.read_bytes()).hexdigest() == build['renderer']['executable_sha256'], 'renderer mismatch'


def shared_markdown_paths(registry, registered, markdown, build, source_bytes, output_bytes):
    """Allow only explicitly selected, source-bound HTML in both manifests."""
    declared = {r['id']: r for r in registry.get('files', [])}
    assert len(declared) == len(registry.get('files', [])), 'duplicate reference id'
    receipts = {r['id']: r for r in registered}
    assert receipts.keys() == declared.keys() and len(receipts) == len(registered), 'reference coverage'
    markdown_by_source = {r['source']: r for r in markdown['pages']}
    shared = set()
    for ident, reference in declared.items():
        receipt = receipts[ident]
        assert receipt['source'] == receipt['input'] == reference['source'], 'reference input'
        raw = source_bytes(reference['source'])
        if reference['source'] not in markdown_by_source:
            assert 'projection' not in receipt and receipt['route'] == reference['route'], 'raw reference route'
            assert output_bytes(receipt['route']) == raw, 'raw reference bytes'
            continue
        entry = markdown_by_source[reference['source']]
        projection = receipt['projection']
        assert receipt['route'] == projection['output_route'] == entry['route'], 'projected reference route'
        assert receipt['source_sha256'] == entry['sha256'] == hashlib.sha256(raw).hexdigest(), 'reference source digest'
        payload = output_bytes(entry['route'])
        assert receipt['sha256'] == projection['output_sha256'] == hashlib.sha256(payload).hexdigest(), 'reference output digest'
        assert receipt['bytes'] == len(payload), 'reference output size'
        assert projection['renderer'] == 'nepl3-tools.site-markdown/1; pulldown-cmark/0.13.4', 'reference renderer'
        assert json.loads(projection['context']) == {'base': build['base_path'], 'source_commit': build['source_commit']}, 'reference context'
        shared.add(entry['route'])
    return shared


def verify(root, source_root=None, config_path='site/config.json', renderer=None):
    root = root.resolve()
    paths = list(root.rglob('*'))
    assert not any((p.is_symlink() for p in paths)), 'symlink in artifact'
    assert sum((p.stat().st_size for p in paths if p.is_file())) <= 32 * 1024 * 1024, 'artifact size limit'
    manifest = json.loads((root / 'manifest.json').read_text(encoding='utf-8'))
    build = json.loads((root / 'build.json').read_text(encoding='utf-8'))
    base = build['base_path']
    files = {str(p.relative_to(root)).replace(chr(92), '/'): p for p in root.rglob('*') if p.is_file()}
    declared = {r['path'] for r in manifest['files']}
    assert len(declared) == len(manifest['files']), 'duplicate manifest file'
    assert declared | {'manifest.json'} == files.keys()
    for record in manifest['files']:
        data = files[record['path']].read_bytes()
        assert len(data) == record['bytes'] and hashlib.sha256(data).hexdigest() == record['sha256']
    docs = {name: Document(path.read_text(encoding='utf-8')) for name, path in files.items() if name.endswith('.html')}
    assert docs, 'empty HTML site'
    for name, doc in docs.items():
        assert not doc.scripts
        for link in doc.links:
            url = urlsplit(urljoin('https://local.invalid' + base + name, link))
            if url.netloc != 'local.invalid':
                continue
            assert url.path.startswith(base), (name, link)
            target = unquote(url.path[len(base):])
            assert target in files, (name, link, target)
            if url.fragment:
                assert unquote(url.fragment) in docs[target].ids, (name, link)
    expected_inputs(build, manifest, json.loads((root / 'doc-manifest.json').read_text(encoding='utf-8')),
                    docs, source_root or Path(__file__).resolve().parents[3], config_path, renderer)
    examples = json.loads((root / 'examples/manifest.json').read_text(encoding='utf-8'))
    checkout = source_root or Path(__file__).resolve().parents[3]
    def blob(path):
        return subprocess.check_output(['git', 'show', build['source_commit'] + ':' + path], cwd=checkout)
    markdown = json.loads((root / 'markdown-manifest.json').read_text(encoding='utf-8'))
    assert markdown['version'] == 1 and markdown['source_commit'] == build['source_commit']
    assert markdown['renderer'] == 'pulldown-cmark/0.13.4'
    projections = {p['projection'] for p in json.loads(blob('doc/canonical.json'))['pages']}
    tracked = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', build['source_commit'], '--', 'doc/spec/'], cwd=checkout, text=True).splitlines()
    expected_markdown = {p for p in tracked if re.fullmatch(r'doc/spec/[0-9]{2}-[^/]*\.md', p)} - projections
    assert {p['source'] for p in markdown['pages']} == expected_markdown
    assert len(markdown['pages']) == len(expected_markdown)
    markdown_paths = {'markdown-manifest.json'}
    for entry in markdown['pages']:
        assert re.fullmatch(r'doc/spec/[0-9]{2}-[A-Za-z0-9-]+\.md', entry['source'])
        assert entry['sha256'] == hashlib.sha256(blob(entry['source'])).hexdigest()
        assert entry['route'] == 'docs/spec/' + entry['source'][len('doc/spec/'):-3] + '.html'
        assert entry['route'] in docs
        markdown_paths.add(entry['route'])
    doc_manifest = json.loads((root / 'doc-manifest.json').read_text(encoding='utf-8'))
    shared = shared_markdown_paths(json.loads(blob('doc/canonical.json')),
        doc_manifest.get('registered_files', []), markdown, build, blob, lambda path: files[path].read_bytes())
    catalog_bytes = blob('site/examples.json')
    catalog = json.loads(catalog_bytes)
    assert examples['source_commit'] == build['source_commit'] and examples['capability'] == 'source-view'
    assert examples['catalog_sha256'] == hashlib.sha256(catalog_bytes).hexdigest()
    assert len(examples['examples']) == len(catalog['examples'])
    assert examples['source_profile'] == {'path': catalog['profile'], 'id': json.loads(blob(catalog['profile']))['id'],
        'sha256': hashlib.sha256(blob(catalog['profile'])).hexdigest(), 'resolved_runtime': False}
    example_paths = {'examples/index.html', 'examples/manifest.json'}
    for entry, declared_example in zip(examples['examples'], catalog['examples']):
        assert all(entry[k] == v for k, v in declared_example.items()), 'example catalog mismatch'
        data = blob(entry['source'])
        assert entry['path'] == 'examples/sources/' + entry['id'] + '.txt', 'example artifact path'
        assert files[entry['path']].read_bytes() == data, 'example source changed'
        assert entry['bytes'] == len(data) and entry['sha256'] == hashlib.sha256(data).hexdigest()
        assert entry['revision'] == build['source_commit'] and entry['execution_available'] is False
        assert entry['required_source_profile'] == examples['source_profile']['id']
        example_paths.add(entry['path'])
    for records, expected, sized in [(build['files'], declared - {'build.json'}, True),
                              (json.loads((root / 'doc-manifest.json').read_text(encoding='utf-8'))['files'],
                               (declared - {'build.json', 'index.html', 'docs/index.html', 'assets/site.css', '.nojekyll', 'doc-manifest.json'} - example_paths - markdown_paths) | shared, False)]:
        assert len(records) == len({record['path'] for record in records}), 'duplicate nested file'
        assert {record['path'] for record in records} == expected, 'nested manifest coverage'
        for record in records:
            assert hashlib.sha256(files[record['path']].read_bytes()).hexdigest() == record['sha256'], 'nested digest mismatch'
            if sized:
                assert record['bytes'] == files[record['path']].stat().st_size, 'nested size mismatch'

    expected_sources = [files[e['path']].read_bytes().decode('utf-8').replace('\r\n', '\n').replace('\r', '\n') for e in examples['examples']]
    rows = [case.representation() for case in observe(root, base, tuple(docs), expected_sources)]
    return dict(result='passed', base=base, source_commit=build['source_commit'], manifest_sha256=hashlib.sha256((root / 'manifest.json').read_bytes()).hexdigest(), files=len(files), cases=rows)
if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--config', default='site/config.json')
    parser.add_argument('--renderer', type=Path, required=True)
    a = parser.parse_args()
    result = verify(a.root, config_path=a.config, renderer=a.renderer)
    a.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(len(result['cases']), 'browser cases passed')
