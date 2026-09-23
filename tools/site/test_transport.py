from contextlib import contextmanager
from collections.abc import Generator, Mapping, Sequence
from types import MappingProxyType
from typing import NamedTuple, override
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import unittest
from unittest.mock import patch

from deployment.receipt import Receipt, Phase
from deployment.transport import direct_status as status, status as bounded_status, TransportError
import subprocess
import sys
import time
import base64


class Request(NamedTuple):
    path: str
    headers: Mapping[str, str]
    body: bytes = b''


@contextmanager
def server(code: int = 200, body: bytes = b'{"status":"succeed"}',
           headers: Mapping[str, str] | None = None,
           extra_headers: Sequence[tuple[str, str]] = (),
           length: str | None = None) -> Generator[list[Request]]:
    seen: list[Request] = []
    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:
            data = self.rfile.read(int(self.headers['Content-Length']))
            self.do_GET()
            seen[-1] = seen[-1]._replace(body=data)

        def do_GET(self) -> None:
            seen.append(Request(self.path, MappingProxyType(dict(self.headers))))
            self.send_response(code)
            for key, value in (headers or {"Content-Type": "application/json"}).items():
                self.send_header(key, value)
            for key, value in extra_headers:
                self.send_header(key, value)
            self.send_header("Content-Length", str(len(body)) if length is None else length)
            self.end_headers()
            _ = self.wfile.write(body)
        @override
        def log_message(self, format: str, *args: object) -> None:
            pass
    instance = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=instance.serve_forever, daemon=True); thread.start()
    try:
        # Only the test replaces the socket factory. Production has no insecure
        # endpoint/configuration option. This does not test TLS itself.
        def connect(host: str, *, timeout: float) -> HTTPConnection:
            if host != "api.github.com": raise AssertionError(host)
            return HTTPConnection("127.0.0.1", instance.server_port, timeout=timeout)
        with patch("deployment.transport.HTTPSConnection", connect):
            yield seen
    finally:
        instance.shutdown(); instance.server_close(); thread.join()


class TransportTests(unittest.TestCase):
    def receipt(self) -> Receipt:
        return Receipt("123", "https://api.github.com/repos/neknaj/NEPL3/pages/deployments/123", "a" * 64)

    def test_real_http_parser_binds_response_and_retains_bytes(self) -> None:
        with server() as seen:
            result = status(self.receipt(), "test-token")
        self.assertEqual(result.observation.phase, Phase.SUCCEEDED)
        self.assertEqual(result.raw_response, b'{"status":"succeed"}')
        self.assertEqual(len(seen), 1)
        self.assertEqual(seen[0][0], "/repos/neknaj/NEPL3/pages/deployments/123")
        self.assertEqual(seen[0][1]["Authorization"], "Bearer test-token")
        self.assertEqual(seen[0][1]["X-GitHub-Api-Version"], "2026-03-10")

    def test_redirect_error_encoding_mime_and_limit_rejected(self) -> None:
        cases: tuple[tuple[int, bytes, Mapping[str, str] | None], ...] = (
            (302, b'', {"Location": "https://evil.invalid/"}),
            (404, b'secret response', None),
            (200, b'{}', {"Content-Type": "text/html"}),
            (200, b'{}', {"Content-Type": "application/json", "Content-Encoding": "gzip"}),
            (200, b'x' * 65537, None),
            (200, b'{"status":"succeed","status":"failed"}', None),
        )
        for code, body, headers in cases:
            with self.subTest(code=code, headers=headers), server(code, body, headers) as seen:
                with self.assertRaises(TransportError) as error:
                    _ = status(self.receipt(), "test-token")
                self.assertNotIn("test-token", str(error.exception))
                self.assertNotIn("secret response", str(error.exception))
                self.assertEqual(len(seen), 1)

    def test_invalid_token_timeout_and_connection_failure(self) -> None:
        with patch("deployment.transport.HTTPSConnection") as factory:
            for token in ["", "token\r\nX-Injected: yes", None]:
                with self.assertRaises(ValueError): _ = status(self.receipt(), token)
            for timeout in [True, 0, 11, float("nan")]:
                with self.assertRaises(ValueError): _ = status(self.receipt(), "token", timeout=timeout)
            factory.assert_not_called()
        closes: list[bool] = []
        test = self
        class FailedConnection:
            def __init__(self, host: str, *, timeout: float) -> None:
                self.host: str = host
                self.timeout: float = timeout

            def request(self, method: str, path: str, *, body: bytes | None,
                        headers: Mapping[str, str]) -> None:
                test.assertEqual(method, "GET")
                test.assertEqual(path, "/repos/neknaj/NEPL3/pages/deployments/123")
                test.assertIsNone(body)
                test.assertEqual(headers["Authorization"], "Bearer test-token")
                raise OSError("test-token")

            def close(self) -> None:
                closes.append(True)

        with patch("deployment.transport.HTTPSConnection", FailedConnection):
            with self.assertRaisesRegex(TransportError, "connection failed") as error:
                _ = status(self.receipt(), "test-token")
            self.assertIsNone(error.exception.__cause__)
            self.assertEqual(closes, [True])

    def test_public_boundary_passes_credentials_on_stdin_and_revalidates(self) -> None:
        calls: list[tuple[tuple[str, ...], bytes]] = []
        reply = base64.b64encode(b'{"status":"succeed"}')
        def run(command: Sequence[str], *, input: bytes, capture_output: bool,
                timeout: float, check: bool) -> subprocess.CompletedProcess[bytes]:
            self.assertTrue(capture_output)
            self.assertFalse(check)
            self.assertEqual(timeout, 10)
            calls.append((tuple(command), input))
            return subprocess.CompletedProcess(command, 0, reply, b'')
        with patch("deployment.transport.subprocess.run", run):
            result = bounded_status(self.receipt(), "private-token")
            self.assertEqual(result.observation.phase, Phase.SUCCEEDED)
            self.assertEqual(len(calls), 1)
            self.assertNotIn("private-token", str(calls[0][0]))
            self.assertIn(b'private-token', calls[0][1])
            reply = b'not base64!'
            with self.assertRaises(TransportError): _ = bounded_status(self.receipt(), "private-token")

    def test_actual_stuck_child_is_terminated_by_outer_deadline(self) -> None:
        original = subprocess.run
        def stuck(command: Sequence[str], *, input: bytes, capture_output: bool,
                  timeout: float, check: bool) -> subprocess.CompletedProcess[bytes]:
            self.assertIn("-I", command)
            return original([sys.executable, "-I", "-c", "import time; time.sleep(60)"],
                            input=input, capture_output=capture_output, timeout=timeout, check=check)
        start = time.monotonic()
        with patch("deployment.transport.subprocess.run", stuck):
            with self.assertRaisesRegex(TransportError, "deadline exceeded"):
                _ = bounded_status(self.receipt(), "private-token", timeout=0.2)
        self.assertLess(time.monotonic() - start, 5)

    def test_short_body_and_ambiguous_framing_fail(self) -> None:
        cases: tuple[tuple[str | None, tuple[tuple[str, str], ...]], ...] = (("120", ()), ("-1", ()), ("20,20", ()),
                               (None, (("Content-Length", "20"),)),
                               (None, (("Transfer-Encoding", "chunked"),)))
        for length, extras in cases:
            with self.subTest(length=length, extras=extras), server(length=length, extra_headers=extras):
                with self.assertRaises(TransportError): _ = status(self.receipt(), "token")
