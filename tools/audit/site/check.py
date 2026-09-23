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
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import final, override
from urllib.parse import urlsplit, unquote, urljoin

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from browser import Case, observe
import records
from tools.serialization.json import JsonValue, integer, string

@final
class Document(HTMLParser):

    def __init__(self, text: str) -> None:
        super().__init__()
        self.ids: set[str | None] = set()
        self.links: list[str | None] = []
        self.scripts = 0
        self.feed(text)

    @override
    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attributes = dict(attrs)
        if tag == 'script' or any((k.startswith('on') for k in attributes)):
            self.scripts += 1
        if 'id' in attributes:
            assert attributes['id'] not in self.ids, 'duplicate id'
            self.ids.add(attributes['id'])
        for key in ('href', 'src'):
            if key in attributes:
                self.links.append(attributes[key])

def expected_inputs(build: records.Build, manifest_commit: str, entries: Sequence[records.PageReceipt],
                    routes: set[str], source_root: Path, config_path: str, renderer: Path | None) -> None:
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=source_root, text=True).strip()
    assert build.source_commit == manifest_commit == commit, 'stale source commit'
    def blob(path: str) -> bytes:
        return subprocess.check_output(['git', 'show', commit + ':' + path], cwd=source_root)
    config = records.document(blob(config_path))
    assert integer(config['version']) == 1 and build.base_path == string(config['base_path']), 'unexpected base'
    assert build.design == string(records.document(blob('design/tasks.json'))['design']), 'design revision mismatch'
    assert build.overview == records.Overview('README.md', hashlib.sha256(blob('README.md')).hexdigest(),
                                             'pulldown-cmark/0.13.4'), 'overview source mismatch'
    registry = records.pages(records.document(blob('doc/canonical.json'))['pages'])
    assert registry, 'empty canonical registry'
    tracked = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', commit, '--', 'doc/spec/'], cwd=source_root, text=True).splitlines()
    markdown_routes = {'docs/spec/' + path[len('doc/spec/'):-3] + '.html' for path in tracked
                       if re.fullmatch(r'doc/spec/[0-9]{2}-[^/]*\.md', path) and path not in {p.projection for p in registry}}
    assert routes == {page.route for page in registry} | markdown_routes | {'index.html', 'docs/index.html', 'examples/index.html'}, 'page coverage'
    assert len(entries) == len(registry), 'source coverage'
    actual = {entry.id: entry for entry in entries}
    assert len(actual) == len(entries), 'duplicate source id'
    for page in registry:
        entry = actual[page.id]
        assert entry.input == page.source and entry.route == page.route, 'source route mismatch'
        assert entry.source_sha256 == hashlib.sha256(blob(page.source)).hexdigest(), 'source digest mismatch'
    assert renderer is not None, 'renderer executable is required'
    assert hashlib.sha256(renderer.read_bytes()).hexdigest() == build.renderer_sha256, 'renderer mismatch'


def shared_markdown_paths(registry: Sequence[records.Reference], registered: Sequence[records.ReferenceReceipt],
                          markdown: Sequence[records.MarkdownPage], base: str, commit: str,
                          source_bytes: Callable[[str], bytes], output_bytes: Callable[[str], bytes]) -> set[str]:
    """Allow only explicitly selected, source-bound HTML in both manifests."""
    declared = {r.id: r for r in registry}
    assert len(declared) == len(registry), 'duplicate reference id'
    receipts = {r.id: r for r in registered}
    assert receipts.keys() == declared.keys() and len(receipts) == len(registered), 'reference coverage'
    markdown_by_source = {r.source: r for r in markdown}
    shared: set[str] = set()
    for ident, reference in declared.items():
        receipt = receipts[ident]
        assert receipt.source == receipt.input == reference.source, 'reference input'
        raw = source_bytes(reference.source)
        if reference.source not in markdown_by_source:
            assert receipt.projection is None and receipt.route == reference.route, 'raw reference route'
            assert output_bytes(receipt.route) == raw, 'raw reference bytes'
            continue
        entry = markdown_by_source[reference.source]
        projection = receipt.projection
        assert projection is not None, 'missing reference projection'
        assert receipt.route == projection.output_route == entry.route, 'projected reference route'
        assert projection.source_sha256 == entry.sha256 == hashlib.sha256(raw).hexdigest(), 'reference source digest'
        payload = output_bytes(entry.route)
        assert receipt.sha256 == projection.output_sha256 == hashlib.sha256(payload).hexdigest(), 'reference output digest'
        assert receipt.size == len(payload), 'reference output size'
        assert projection.renderer == 'nepl3-tools.site-markdown/1; pulldown-cmark/0.13.4', 'reference renderer'
        assert (projection.context_base, projection.context_commit) == (base, commit), 'reference context'
        shared.add(entry.route)
    return shared


@dataclass(frozen=True, slots=True)
class Report:
    base: str
    source_commit: str
    manifest_sha256: str
    files: int
    cases: tuple[Case, ...]

    def representation(self) -> dict[str, JsonValue]:
        return dict(result='passed', base=self.base, source_commit=self.source_commit,
                    manifest_sha256=self.manifest_sha256, files=self.files,
                    cases=[case.representation() for case in self.cases])


def verify(root: Path, source_root: Path | None = None, config_path: str = 'site/config.json',
           renderer: Path | None = None) -> Report:
    root = root.resolve()
    paths = list(root.rglob('*'))
    assert not any((p.is_symlink() for p in paths)), 'symlink in artifact'
    assert sum((p.stat().st_size for p in paths if p.is_file())) <= 32 * 1024 * 1024, 'artifact size limit'
    manifest = records.document((root / 'manifest.json').read_bytes())
    build_input = records.document((root / 'build.json').read_bytes())
    base = string(build_input['base_path'])
    outer_files = records.files(manifest['files'], sized=True)
    files = {str(p.relative_to(root)).replace(chr(92), '/'): p for p in root.rglob('*') if p.is_file()}
    declared = {r.path for r in outer_files}
    assert len(declared) == len(outer_files), 'duplicate manifest file'
    assert declared | {'manifest.json'} == files.keys()
    for record in outer_files:
        data = files[record.path].read_bytes()
        assert len(data) == record.size and hashlib.sha256(data).hexdigest() == record.sha256
    docs = {name: Document(path.read_text(encoding='utf-8')) for name, path in files.items() if name.endswith('.html')}
    assert docs, 'empty HTML site'
    for name, doc in docs.items():
        assert not doc.scripts
        for link in doc.links:
            url = urlsplit(urljoin('https://local.invalid' + base + name, link or ''))
            if url.netloc != 'local.invalid':
                continue
            assert url.path.startswith(base), (name, link)
            target = unquote(url.path[len(base):])
            assert target in files, (name, link, target)
            if url.fragment:
                assert unquote(url.fragment) in docs[target].ids, (name, link)
    build = records.build(build_input)
    doc_manifest = records.document((root / 'doc-manifest.json').read_bytes())
    expected_inputs(build, string(manifest['source_commit']), records.page_receipts(doc_manifest['pages']),
                    set(docs), source_root or Path(__file__).resolve().parents[3], config_path, renderer)
    examples = records.examples(records.document((root / 'examples/manifest.json').read_bytes()))
    checkout = source_root or Path(__file__).resolve().parents[3]
    def blob(path: str) -> bytes:
        return subprocess.check_output(['git', 'show', build.source_commit + ':' + path], cwd=checkout)
    markdown_input = records.document((root / 'markdown-manifest.json').read_bytes())
    assert integer(markdown_input['version']) == 1 and string(markdown_input['source_commit']) == build.source_commit
    assert string(markdown_input['renderer']) == 'pulldown-cmark/0.13.4'
    markdown = records.markdown_pages(markdown_input['pages'])
    registry = records.document(blob('doc/canonical.json'))
    canonical_pages = records.pages(registry['pages'])
    assert all(page.projection is not None for page in canonical_pages), 'missing canonical projection'
    projections = {page.projection for page in canonical_pages}
    tracked = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', build.source_commit, '--', 'doc/spec/'], cwd=checkout, text=True).splitlines()
    expected_markdown = {p for p in tracked if re.fullmatch(r'doc/spec/[0-9]{2}-[^/]*\.md', p)} - projections
    assert {page.source for page in markdown} == expected_markdown
    assert len(markdown) == len(expected_markdown)
    markdown_paths = {'markdown-manifest.json'}
    for entry in markdown:
        assert re.fullmatch(r'doc/spec/[0-9]{2}-[A-Za-z0-9-]+\.md', entry.source)
        assert entry.sha256 == hashlib.sha256(blob(entry.source)).hexdigest()
        assert entry.route == 'docs/spec/' + entry.source[len('doc/spec/'):-3] + '.html'
        assert entry.route in docs
        markdown_paths.add(entry.route)
    shared = shared_markdown_paths(records.references(registry.get('files', [])),
        records.reference_receipts(doc_manifest.get('registered_files', [])), markdown,
        build.base_path, build.source_commit, blob, lambda path: files[path].read_bytes())
    catalog_bytes = blob('site/examples.json')
    catalog = records.catalog(records.document(catalog_bytes))
    assert examples.source_commit == build.source_commit
    assert examples.catalog_sha256 == hashlib.sha256(catalog_bytes).hexdigest()
    assert len(examples.examples) == len(catalog.examples)
    profile_bytes = blob(catalog.profile)
    assert examples.source_profile == records.SourceProfile(catalog.profile,
        string(records.document(profile_bytes)['id']), hashlib.sha256(profile_bytes).hexdigest())
    example_paths = {'examples/index.html', 'examples/manifest.json'}
    for entry, declared_example in zip(examples.examples, catalog.examples, strict=True):
        assert entry.example == declared_example, 'example catalog mismatch'
        data = blob(entry.example.source)
        assert entry.path == 'examples/sources/' + entry.example.id + '.txt', 'example artifact path'
        assert files[entry.path].read_bytes() == data, 'example source changed'
        assert entry.size == len(data) and entry.sha256 == hashlib.sha256(data).hexdigest()
        assert entry.revision == build.source_commit
        assert entry.required_source_profile == examples.source_profile.id
        example_paths.add(entry.path)
    for roster, expected, sized in [(records.files(build_input['files'], sized=True), declared - {'build.json'}, True),
                              (records.files(doc_manifest['files'], sized=False),
                               (declared - {'build.json', 'index.html', 'docs/index.html', 'assets/site.css', '.nojekyll', 'doc-manifest.json'} - example_paths - markdown_paths) | shared, False)]:
        assert len(roster) == len({record.path for record in roster}), 'duplicate nested file'
        assert {record.path for record in roster} == expected, 'nested manifest coverage'
        for record in roster:
            assert hashlib.sha256(files[record.path].read_bytes()).hexdigest() == record.sha256, 'nested digest mismatch'
            if sized:
                assert record.size == files[record.path].stat().st_size, 'nested size mismatch'

    expected_sources = [files[entry.path].read_bytes().decode('utf-8').replace('\r\n', '\n').replace('\r', '\n') for entry in examples.examples]
    rows = observe(root, base, tuple(docs), expected_sources)
    return Report(base, build.source_commit, hashlib.sha256((root / 'manifest.json').read_bytes()).hexdigest(), len(files), rows)


class Arguments(argparse.Namespace):
    root: Path = Path()
    output: Path = Path()
    config: str = 'site/config.json'
    renderer: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser()
    _ = parser.add_argument('root', type=Path)
    _ = parser.add_argument('output', type=Path)
    _ = parser.add_argument('--config', default='site/config.json')
    _ = parser.add_argument('--renderer', type=Path, required=True)
    a = parser.parse_args(namespace=Arguments())
    result = verify(a.root, config_path=a.config, renderer=a.renderer)
    _ = a.output.write_text(json.dumps(result.representation(), indent=2) + '\n', encoding='utf-8')
    print(len(result.cases), 'browser cases passed')


if __name__ == '__main__':
    main()
