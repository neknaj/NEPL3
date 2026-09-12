"""Verify a production docs-only bundle and its scriptless browser routes.

This test runner requires pinned Playwright engines. Assertions must be enabled;
missing runners, digest/link mismatches and browser failures are test failures.
It tests generated static pages, not Playground or live Pages deployment.
"""
if not __debug__:
    raise RuntimeError('site audit requires Python assertions')
import argparse, hashlib, json, threading, subprocess
from pathlib import Path
from html.parser import HTMLParser
from urllib.parse import urlsplit, unquote, urljoin
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from playwright.sync_api import sync_playwright

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
    assert build['capability'] == 'docs-only' and build['runtime_identity'] is None, 'unexpected runtime capability'
    registry = json.loads(blob('doc/canonical.json'))['pages']
    assert registry, 'empty canonical registry'
    assert docs.keys() == {page['route'] for page in registry} | {'index.html', 'docs/index.html'}, 'page coverage'
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
    for records, expected, sized in [(build['files'], declared - {'build.json'}, True),
                              (json.loads((root / 'doc-manifest.json').read_text(encoding='utf-8'))['files'],
                               declared - {'build.json', 'index.html', 'docs/index.html', 'assets/site.css', '.nojekyll', 'doc-manifest.json'}, False)]:
        assert len(records) == len({record['path'] for record in records}), 'duplicate nested file'
        assert {record['path'] for record in records} == expected, 'nested manifest coverage'
        for record in records:
            assert hashlib.sha256(files[record['path']].read_bytes()).hexdigest() == record['sha256'], 'nested digest mismatch'
            if sized:
                assert record['bytes'] == files[record['path']].stat().st_size, 'nested size mismatch'

    class Handler(SimpleHTTPRequestHandler):

        def __init__(self, *args, **kw):
            super().__init__(*args, directory=str(root), **kw)

        def do_GET(self):
            path = urlsplit(self.path).path
            if not path.startswith(base):
                self.send_error(404)
                return
            self.path = '/' + path[len(base):]
            super().do_GET()

        def log_message(self, *args):
            pass
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    rows = []
    try:
        with sync_playwright() as p:
            for engine in ['chromium', 'firefox', 'webkit']:
                browser = getattr(p, engine).launch()
                for width in [375, 1280]:
                    context = browser.new_context(java_script_enabled=False, viewport={'width': width, 'height': 900})
                    page = context.new_page()
                    fail = []
                    page.on('requestfailed', lambda r: fail.append(r.url))
                    page.on('response', lambda r: fail.append(r.url) if r.status >= 400 else None)
                    for name in docs:
                        response = page.goto(f'http://127.0.0.1:{server.server_port}' + base + name, wait_until='networkidle')
                        state = page.evaluate('()=>({scripts:document.scripts.length,styles:document.styleSheets.length,overflow:document.documentElement.scrollWidth>innerWidth,title:document.title})')
                        assert response.status == 200 and (not fail) and (state['scripts'] == 0) and (state['styles'] > 0) and (not state['overflow']), (name, state, fail)
                        rows.append(dict(engine=engine, version=browser.version, width=width, page=name, **state))
                    context.close()
                browser.close()
    finally:
        server.shutdown()
        server.server_close()
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
