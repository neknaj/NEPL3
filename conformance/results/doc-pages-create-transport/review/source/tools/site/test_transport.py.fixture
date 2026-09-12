from contextlib import contextmanager
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import unittest
from unittest.mock import patch

from deployment.receipt import Receipt, Phase
from deployment.transport import _status as status, status as bounded_status, TransportError
import subprocess
import sys
import time
import base64


@contextmanager
def server(code=200, body=b'{"status":"succeed"}', headers=None, extra_headers=(), length=None):
    seen = []
    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            data = self.rfile.read(int(self.headers['Content-Length']))
            self.do_GET()
            seen[-1] = (*seen[-1], data)

        def do_GET(self):
            seen.append((self.path, dict(self.headers)))
            self.send_response(code)
            for key, value in (headers or {"Content-Type": "application/json"}).items():
                self.send_header(key, value)
            for key, value in extra_headers:
                self.send_header(key, value)
            self.send_header("Content-Length", str(len(body)) if length is None else length)
            self.end_headers()
            self.wfile.write(body)
        def log_message(self, *args):
            pass
    instance = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=instance.serve_forever, daemon=True); thread.start()
    try:
        # Only the test replaces the socket factory. Production has no insecure
        # endpoint/configuration option. This does not test TLS itself.
        def connect(host, *, timeout):
            if host != "api.github.com": raise AssertionError(host)
            return HTTPConnection("127.0.0.1", instance.server_port, timeout=timeout)
        with patch("deployment.transport.HTTPSConnection", connect):
            yield seen
    finally:
        instance.shutdown(); instance.server_close(); thread.join()


class TransportTests(unittest.TestCase):
    def receipt(self):
        return Receipt("123", "https://api.github.com/repos/neknaj/NEPL3/pages/deployments/123", "a" * 64)

    def test_real_http_parser_binds_response_and_retains_bytes(self):
        with server() as seen:
            result = status(self.receipt(), "test-token")
        self.assertEqual(result.observation.phase, Phase.SUCCEEDED)
        self.assertEqual(result.raw_response, b'{"status":"succeed"}')
        self.assertEqual(len(seen), 1)
        self.assertEqual(seen[0][0], "/repos/neknaj/NEPL3/pages/deployments/123")
        self.assertEqual(seen[0][1]["Authorization"], "Bearer test-token")
        self.assertEqual(seen[0][1]["X-GitHub-Api-Version"], "2026-03-10")

    def test_redirect_error_encoding_mime_and_limit_rejected(self):
        for code, body, headers in [
            (302, b'', {"Location": "https://evil.invalid/"}),
            (404, b'secret response', None),
            (200, b'{}', {"Content-Type": "text/html"}),
            (200, b'{}', {"Content-Type": "application/json", "Content-Encoding": "gzip"}),
            (200, b'x' * 65537, None),
            (200, b'{"status":"succeed","status":"failed"}', None),
        ]:
            with self.subTest(code=code, headers=headers), server(code, body, headers) as seen:
                with self.assertRaises(TransportError) as error:
                    status(self.receipt(), "test-token")
                self.assertNotIn("test-token", str(error.exception))
                self.assertNotIn("secret response", str(error.exception))
                self.assertEqual(len(seen), 1)

    def test_invalid_token_timeout_and_connection_failure(self):
        with patch("deployment.transport.HTTPSConnection") as factory:
            for token in ["", "token\r\nX-Injected: yes", None]:
                with self.assertRaises(ValueError): status(self.receipt(), token)
            for timeout in [True, 0, 11, float("nan")]:
                with self.assertRaises(ValueError): status(self.receipt(), "token", timeout=timeout)
            factory.assert_not_called()
        with patch("deployment.transport.HTTPSConnection") as factory:
            factory.return_value.request.side_effect = OSError("test-token")
            with self.assertRaisesRegex(TransportError, "connection failed") as error:
                status(self.receipt(), "test-token")
            self.assertIsNone(error.exception.__cause__)
            factory.return_value.close.assert_called_once()

    def test_public_boundary_passes_credentials_on_stdin_and_revalidates(self):
        with patch("deployment.transport.subprocess.run") as run:
            run.return_value = subprocess.CompletedProcess([], 0, base64.b64encode(b'{"status":"succeed"}'), b'')
            result = bounded_status(self.receipt(), "private-token")
            self.assertEqual(result.observation.phase, Phase.SUCCEEDED)
            self.assertNotIn("private-token", str(run.call_args.args))
            self.assertIn(b'private-token', run.call_args.kwargs["input"])
            run.return_value = subprocess.CompletedProcess([], 0, b'not base64!', b'')
            with self.assertRaises(TransportError): bounded_status(self.receipt(), "private-token")

    def test_actual_stuck_child_is_terminated_by_outer_deadline(self):
        original = subprocess.run
        def stuck(command, **kwargs):
            return original([sys.executable, "-I", "-c", "import time; time.sleep(60)"], **kwargs)
        start = time.monotonic()
        with patch("deployment.transport.subprocess.run", stuck):
            with self.assertRaisesRegex(TransportError, "deadline exceeded"):
                bounded_status(self.receipt(), "private-token", timeout=0.2)
        self.assertLess(time.monotonic() - start, 5)

    def test_short_body_and_ambiguous_framing_fail(self):
        for length, extras in [("120", ()), ("-1", ()), ("20,20", ()),
                               (None, (("Content-Length", "20"),)),
                               (None, (("Transfer-Encoding", "chunked"),))]:
            with self.subTest(length=length, extras=extras), server(length=length, extra_headers=extras):
                with self.assertRaises(TransportError): status(self.receipt(), "token")
