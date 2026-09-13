"""Compare original fixed KaTeX and Rust-serialized visual trees in real browsers.

Consumes the normal production TeX integration test's stdout, not golden HTML.
This tests visual-tree/style lowering, not independent MathML accessibility or
complete Doc artifact admission. Assets are served only on loopback.
"""
import argparse
import hashlib
import json
import sys
import threading
import time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from playwright.sync_api import sync_playwright


MEASURE = """() => {
  const root = document.querySelector('.katex-display') || document.querySelector('.katex');
  if (!root) throw new Error('missing math root');
  const origin = root.getBoundingClientRect();
  const properties = ['height','width','min-width','padding-left','border-bottom-width',
    'border-top-width','border-right-width','top','bottom','left','margin-left',
    'margin-right','margin-top','vertical-align','position','font-family','font-size'];
  return [root, ...root.querySelectorAll('*')].map(e => {
    const rect = e.getBoundingClientRect(), style = getComputedStyle(e);
    return {tag:e.tagName, text:e.textContent,
      rect:[rect.x-origin.x,rect.y-origin.y,rect.width,rect.height],
      style:properties.map(p => style.getPropertyValue(p))};
  });
}"""


def run(corpus, package, engines):
    rows = []
    for line in corpus.read_text(encoding='utf-8-sig').splitlines():
        marker = 'MATH_VISUAL_CASE '
        if marker in line:
            rows.append(json.loads(line.split(marker, 1)[1]))
    if len(rows) < 30 or not any('<svg' in row['original'] for row in rows):
        raise ValueError('missing actual constructor/SVG corpus')
    manifest = json.loads((Path(__file__).resolve().parents[2] / 'math/katex/assets.json').read_text(encoding='utf-8'))
    for entry in manifest['files']:
        data = (package / entry['source']).read_bytes()
        if len(data) != entry['bytes'] or hashlib.sha256(data).hexdigest() != entry['sha256']:
            raise ValueError('fixed resource mismatch: ' + entry['source'])

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(package / 'dist'), **kwargs)

        def log_message(self, *args):
            pass

        def do_GET(self):
            if self.path == '/case':
                data = self.server.document.encode('utf-8')
                self.send_response(200)
                self.send_header('Content-Type', 'text/html; charset=utf-8')
                self.send_header('Content-Length', str(len(data)))
                self.send_header('Cache-Control', 'no-store')
                self.end_headers()
                self.wfile.write(data)
            else:
                super().do_GET()

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    checked = 0
    try:
        with sync_playwright() as p:
            for engine in engines:
                browser = getattr(p, engine).launch()
                try:
                    for width in [375, 1280]:
                        context = browser.new_context(java_script_enabled=False, viewport={'width': width, 'height': 900})
                        page = context.new_page()
                        failures = []
                        page.on('requestfailed', lambda request: failures.append(request.url))
                        page.goto(f'http://127.0.0.1:{server.server_port}/')
                        # Inspect the fixed browser-parsed stylesheet, including
                        # shorthands/all. Only its unrelated legacy accessibility
                        # declaration is permitted to be important.
                        page.set_content('<link rel="stylesheet" href="katex.min.css">')
                        page.wait_for_function('document.styleSheets.length === 1')
                        important = page.evaluate("""() => {
                          const out=[];
                          function walk(rules) { for (const r of rules) {
                            if (r.style) for (const p of r.style) {
                              if (r.style.getPropertyPriority(p)) out.push(p);
                            }
                            if (r.cssRules) walk(r.cssRules);
                          }}
                          walk(document.styleSheets[0].cssRules); return out;
                        }""")
                        if any(prop != '-ms-high-contrast-adjust' for prop in important):
                            raise AssertionError(('important stylesheet conflict', important))
                        for index, row in enumerate(rows):
                            measured = []
                            for rewritten in [False, True]:
                                html = row['html'] if rewritten else row['original']
                                css = row['css'] if rewritten else ''
                                csp = "default-src 'none'; style-src 'self' 'unsafe-inline'; font-src 'self'; script-src 'none'"
                                if rewritten:
                                    csp += "; style-src-attr 'none'"
                                server.document = '<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="' + csp + \
                                    '"><link rel="stylesheet" href="katex.min.css"><style>' + css + '</style></head><body>' + html + \
                                    '<span id="csp-probe" style="display:none">probe</span></body></html>'
                                # Real navigation is essential: document.write
                                # replacement is not a fresh CSP document.
                                page.goto(f'http://127.0.0.1:{server.server_port}/case')
                                page.evaluate('document.body.getBoundingClientRect().height')
                                deadline = time.monotonic() + 10
                                # Do not await a page Promise with document JS
                                # disabled: Firefox can leave it unsettled. Poll
                                # the observed font state with a host deadline.
                                while page.evaluate('document.fonts.status') != 'loaded':
                                    if time.monotonic() > deadline:
                                        raise TimeoutError((engine, index, 'font loading'))
                                    time.sleep(0.05)
                                measured.append(page.evaluate(MEASURE))
                                if rewritten:
                                    assert page.locator('#csp-probe').evaluate('(e)=>getComputedStyle(e).display') != 'none'
                            a, b = measured
                            assert len(a) == len(b), (engine, index, 'node count')
                            for original, rewritten in zip(a, b):
                                assert original['tag'] == rewritten['tag'] and original['text'] == rewritten['text'], (engine, index, 'content')
                                assert original['style'] == rewritten['style'], (engine, index, original, rewritten)
                                assert all(abs(x-y) < 0.05 for x, y in zip(original['rect'], rewritten['rect'])), (engine, index, original, rewritten)
                            checked += 1
                        assert not failures, failures
                        context.close()
                        print(f'{engine} width={width}: {len(rows)} comparisons passed', file=sys.stderr, flush=True)
                finally:
                    browser.close()
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
    return {'engines': engines, 'cases': len(rows), 'comparisons': checked,
            'scope': 'fixed CSS, visual tree and computed style lowering; not Doc/Math accessibility acceptance'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, required=True)
    parser.add_argument('--package', type=Path, default=Path(__file__).parent / 'node_modules/katex')
    parser.add_argument('--engines', nargs='+', choices=['chromium', 'firefox', 'webkit'], default=['chromium', 'firefox', 'webkit'])
    args = parser.parse_args()
    print(json.dumps(run(args.corpus, args.package.resolve(), args.engines)))
