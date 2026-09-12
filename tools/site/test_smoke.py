from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import tempfile
import threading
import time
import unittest
from urllib.parse import urlsplit

from payload import digest
from smoke import endpoint, run


@contextmanager
def fixture(mode='normal'):
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve()
        files = {'index.html': b'<h1>Home</h1>', 'docs/index.html': b'<h1>Docs</h1>',
                 'docs/chapter.html': '<p>文</p>'.encode(), 'assets/doc.css': b'p{color:black}',
                 '.nojekyll': b'', 'build.json': json.dumps(dict(capability='docs-only',
                 source_commit='a' * 40, base_path='/NEPL3/')).encode()}
        manifest = json.dumps(dict(version=1, files=[dict(path=n, bytes=len(b), sha256=digest(b))
                                                     for n, b in files.items()])).encode()
        files['manifest.json'] = manifest
        for name, data in files.items():
            p = root / name; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(data)
        seen = []
        started = threading.Event()

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                started.set()
                parsed = urlsplit(self.path)
                seen.append(dict(path=parsed.path, query=parsed.query,
                                 cache=self.headers.get('Cache-Control')))
                name = parsed.path.removeprefix('/NEPL3/')
                if not name or name.endswith('/'):
                    name += 'index.html'
                data = files.get(name)
                code = 200 if data is not None else 404
                mime = 'text/html' if name.endswith('.html') else 'text/css' if name.endswith('.css') else 'application/json'
                if mode == 'redirect':
                    self.send_response(302); self.send_header('Location', '/redirected'); self.end_headers(); return
                if mode == 'slow':
                    time.sleep(2)
                if mode == 'changed' and name == 'docs/chapter.html': data = b'old'
                if mode == 'extra-byte' and name == 'docs/chapter.html': data += b'x'
                if mode == 'mime' and name.endswith('.css'): mime = 'text/plain'
                if mode == 'missing' and name == 'docs/chapter.html': code = 404
                if mode == 'query-only' and not parsed.query: code = 503
                if mode == 'spa' and data is None: code = 200; data = files['index.html']
                self.send_response(code); self.send_header('Content-Type', mime)
                self.send_header('Content-Length', str(len(data or b''))); self.end_headers()
                try:
                    self.wfile.write(data or b'')
                except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                    pass

            def log_message(self, *args): pass

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        try:
            yield root, digest(manifest), f'http://127.0.0.1:{server.server_port}/NEPL3/', seen, started
        finally:
            server.shutdown(); server.server_close(); thread.join()


class SmokeTests(unittest.TestCase):
    def test_actual_http_checks_documents_assets_directory_routes_and_404(self):
        with fixture() as (root, identity, url, seen, _):
            result = run(root, identity, url, local=True)
            self.assertEqual(result['result'], 'passed', result)
            self.assertEqual(result['transport'], 'loopback-http')
            self.assertFalse(result['publication_verified'])
            paths = [r['path'] for r in seen]
            self.assertIn('/NEPL3/docs/', paths)
            self.assertIn('/NEPL3/', paths)
            self.assertEqual(paths.count('/NEPL3/build.json'), 3)
            self.assertNotIn('/NEPL3/.nojekyll', paths)
            self.assertTrue(all(r['cache'] == 'no-cache' for r in seen))
            self.assertEqual(sum(r['query'] == 'nepl3-smoke=' + identity for r in seen), 2)
            self.assertTrue(any(r['path'] == '/NEPL3/docs/' and r['query'] == '' for r in seen))

    def test_failures_are_not_passed_and_redirect_is_not_followed(self):
        for mode in ['changed', 'extra-byte', 'mime', 'missing', 'spa', 'redirect', 'query-only']:
            with self.subTest(mode=mode), fixture(mode) as (root, identity, url, seen, _):
                result = run(root, identity, url, local=True)
                self.assertEqual(result['result'], 'failed', result)
                self.assertFalse(result['publication_verified'])
                self.assertNotIn('/redirected', [r['path'] for r in seen])

    def test_outer_deadline_terminates_a_stalled_request(self):
        with fixture('slow') as (root, identity, url, _, started):
            before = time.monotonic()
            result = run(root, identity, url, local=True, timeout=1)
            self.assertTrue(started.is_set())
            self.assertEqual(result['reason'], 'deadline')
            self.assertLess(time.monotonic() - before, 2)

    def test_https_and_exact_base_are_required(self):
        endpoint('https://neknaj.github.io/NEPL3/', '/NEPL3/', False)
        for url in ['http://example.com/NEPL3/', 'https://x/other/', 'https://u:p@x/NEPL3/',
                    'https://x/NEPL3/?q=1', 'https://x/NEPL3/#a', 'https://x:444/NEPL3/']:
            with self.subTest(url=url), self.assertRaises(ValueError): endpoint(url, '/NEPL3/', False)
        with self.assertRaises(ValueError): endpoint('http://example.com:80/NEPL3/', '/NEPL3/', True)


if __name__ == '__main__':
    unittest.main()
