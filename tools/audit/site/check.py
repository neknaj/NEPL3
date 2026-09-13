"""Verify a production docs-only bundle and its scriptless browser routes.

This test runner requires pinned Playwright engines. Assertions must be enabled;
missing runners, digest/link mismatches and browser failures are test failures.
It tests generated static pages, not Playground or live Pages deployment.
"""
if not __debug__:
    raise RuntimeError('site audit requires Python assertions')
import argparse, hashlib, json, threading, subprocess, re
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
        self.redirect = None
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'script' or any((k.startswith('on') for k in attrs)):
            self.scripts += 1
        if tag == 'meta' and attrs.get('http-equiv', '').lower() == 'refresh':
            assert attrs.get('content', '').startswith('0;URL='), 'unexpected redirect'
            self.redirect = attrs['content'][6:]
            self.links.append(self.redirect)
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
    assert docs.keys() == {page['route'] for page in registry} | markdown_routes | {'index.html', 'docs/index.html', 'examples/index.html', 'api/rust/index.html'}, 'page coverage'
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


def validate_links(docs, files, base):
    for name, doc in docs.items():
        if not name.startswith('api/rust/'):
            assert not doc.scripts
        for link in doc.links:
            # Rustdoc on Windows can emit backslashes. HTTP(S) URL parsing in
            # browsers treats these as path separators (unlike urljoin).
            url = urlsplit(urljoin('https://local.invalid' + base + name, link.replace('\\', '/')))
            if url.netloc != 'local.invalid':
                continue
            assert url.path.startswith(base), (name, link)
            target = unquote(url.path[len(base):])
            assert target in files, (name, link, target)
            if url.fragment:
                fragment = unquote(url.fragment)
                seen = set()
                while target in docs and docs[target].redirect is not None:
                    assert target.startswith('api/rust/') and target not in seen, 'redirect cycle or non-API redirect'
                    seen.add(target)
                    redirected = urlsplit(urljoin('https://local.invalid' + base + target, docs[target].redirect.replace('\\', '/')))
                    assert redirected.netloc == 'local.invalid' and redirected.path.startswith(base), 'external API redirect'
                    target = unquote(redirected.path[len(base):])
                assert target in docs, (name, link)
                line_range = re.fullmatch(r'(\d+)-(\d+)', fragment) if target.startswith('api/rust/src/') else None
                assert url.fragment in docs[target].ids or fragment in docs[target].ids or (line_range and all(part in docs[target].ids for part in line_range.groups())), (name, link)


def verify(root, source_root=None, config_path='site/config.json', renderer=None):
    root = root.resolve()
    paths = list(root.rglob('*'))
    assert not any((p.is_symlink() for p in paths)), 'symlink in artifact'
    assert sum((p.stat().st_size for p in paths if p.is_file())) <= 64 * 1024 * 1024, 'artifact size limit'
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
    validate_links(docs, files, base)
    api_docs = {name: doc for name, doc in docs.items() if name.startswith('api/rust/') and name != 'api/rust/index.html'}
    docs = {name: doc for name, doc in docs.items() if name not in api_docs}
    expected_inputs(build, manifest, json.loads((root / 'doc-manifest.json').read_text(encoding='utf-8')),
                    docs, source_root or Path(__file__).resolve().parents[3], config_path, renderer)
    examples = json.loads((root / 'examples/manifest.json').read_text(encoding='utf-8'))
    checkout = source_root or Path(__file__).resolve().parents[3]
    def blob(path):
        return subprocess.check_output(['git', 'show', build['source_commit'] + ':' + path], cwd=checkout)
    api = json.loads((root / 'api/rust/manifest.json').read_text(encoding='utf-8'))
    assert api['version'] == 1 and api['source_commit'] == build['source_commit']
    metadata = json.loads(subprocess.check_output(['cargo','metadata','--no-deps','--locked','--format-version','1'],cwd=checkout))
    names = sorted(t['name'] for p in metadata['packages'] if p['id'] in metadata['workspace_members'] for t in p['targets'] if 'lib' in t['kind'])
    assert api['roots'] == [{'name':name,'route':f'api/rust/{name}/index.html'} for name in names]
    assert api['command'] == ['cargo','doc','--workspace','--no-deps','--locked']
    assert api['rustdoc'] == subprocess.check_output(['rustdoc','--version'],cwd=checkout,text=True).strip()
    rustdoc_executable = Path(subprocess.check_output(['rustup','which','rustdoc'],cwd=checkout,text=True).strip())
    assert api['rustdoc_sha256'] == hashlib.sha256(rustdoc_executable.read_bytes()).hexdigest()
    assert api['compiler'] == subprocess.check_output(['rustc','-vV'],cwd=checkout,text=True).strip()
    corrected = set()
    allowed_corrections = {
        f'api/rust/trait.impl/{owner}/{module}/host/trait.{name}.js': f'api/rust/{owner}/{module}/trait.{name}.html'
        for owner, module, name in [('nepl3_engine','binding','BindingHost'), ('nepl3_reader','tokenizer','TokenizationHost')]
    }
    for correction in api['corrections']:
        resource = correction['resource']
        assert resource in allowed_corrections and resource not in corrected
        corrected.add(resource)
        assert correction['page'] == allowed_corrections[resource]
        assert correction['reason'] == 'rustdoc-1.97-empty-public-implementors/1'
        assert api['rustdoc'] == 'rustdoc 1.97.0 (2d8144b78 2026-07-07)'
        for key in ('page', 'resource'):
            assert correction[key + '_sha256'] == hashlib.sha256(files[correction[key]].read_bytes()).hexdigest()
    assert all(p['route'] in api_docs for p in api['roots'])
    api_paths = {path for path in files if path.startswith('api/rust/')}
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
                               declared - {'build.json', 'index.html', 'docs/index.html', 'assets/site.css', '.nojekyll', 'doc-manifest.json'} - example_paths - markdown_paths - api_paths, False)]:
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
                        if name == 'examples/index.html':
                            expected_sources = [files[e['path']].read_bytes().decode('utf-8').replace('\r\n', '\n').replace('\r', '\n') for e in examples['examples']]
                            assert page.locator('pre > code').all_text_contents() == expected_sources, 'example display differs from source'
                        rows.append(dict(engine=engine, version=browser.version, width=width, page=name, **state))
                    context.close()
                    for enabled in [False, True]:
                        context = browser.new_context(java_script_enabled=enabled, viewport={'width': width, 'height': 900})
                        for entry in api['roots']:
                            # Each observation owns its page and request log.
                            # Teardown cancellations from a previous page must
                            # not become failures of the next crate's requests.
                            page = context.new_page()
                            failures = []
                            finished = {}
                            page.on('requestfailed', lambda r, log=failures: log.append(('request', r.url, r.failure)))
                            page.on('response', lambda r, log=failures: log.append(('response', r.url, r.status)) if r.status >= 400 else None)
                            page.on('requestfinished', lambda r, log=finished: log.update({r.url: r.response()}))
                            response = page.goto(f'http://127.0.0.1:{server.server_port}' + base + entry['route'], wait_until='networkidle')
                            assert response.status == 200 and not failures, (entry, failures)
                            assert page.locator('#main-content').is_visible()
                            if enabled:
                                # Rustdoc collapses the search field on narrow
                                # screens; its documented shortcut opens it.
                                page.keyboard.press('/')
                                search = page.locator('input.search-input')
                                search.fill('SourceSnapshot')
                                search.press('Enter')
                                page.locator('#search').wait_for(state='visible')
                                page.locator('#search a[href*="struct.SourceSnapshot.html"]').first.wait_for(state='visible')
                                # Rustdoc fills its three result tabs separately.
                                # Names appearing does not finish the type search.
                                page.wait_for_function("()=>document.querySelectorAll('#search-tabs .count.loading').length === 0")
                                page.wait_for_load_state('networkidle')
                            redundant_cancellations = []
                            for kind, url, cause in failures:
                                # WebKit can cancel a duplicate search-shard load
                                # while successfully loading the same URL. Admit
                                # only that observed case, with finished response
                                # bytes equal to this artifact; never a missing
                                # asset or an unexplained failed request.
                                assert enabled and engine == 'webkit' and kind == 'request' and cause == 'Load request cancelled', (engine, entry, failures)
                                completed = finished.get(url)
                                parsed = urlsplit(url)
                                assert parsed.scheme == 'http' and parsed.netloc == f'127.0.0.1:{server.server_port}' and not parsed.query and not parsed.fragment, url
                                assert parsed.path.startswith(base + 'api/rust/search.index/') and parsed.path.endswith('.js'), url
                                path = unquote(parsed.path[len(base):])
                                assert path in files and completed is not None and completed.status == 200, (url, failures)
                                assert completed.body() == files[path].read_bytes(), url
                                redundant_cancellations.append(url)
                            rows.append(dict(engine=engine, version=browser.version, width=width, page=entry['route'], javascript=enabled, api=True, redundant_cancellations=redundant_cancellations))
                            page.close()
                        context.close()
                browser.close()
    finally:
        server.shutdown()
        server.server_close()
    return dict(result='passed', base=base, source_commit=build['source_commit'], manifest_sha256=hashlib.sha256((root / 'manifest.json').read_bytes()).hexdigest(), files=len(files), static_api_pages=len(api_docs), cases=rows)
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
