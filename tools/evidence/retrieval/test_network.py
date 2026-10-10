"""Offline HTTP fixtures; never real authentication or execution evidence."""
from dataclasses import replace
import json
import io
import socket
from http.client import HTTPResponse
import subprocess
import sys
from pathlib import Path
from typing import override, cast
from types import TracebackType
import unittest
from unittest.mock import patch

from tools.evidence.retrieval import network
from tools.evidence.retrieval.metadata import timestamp
from tools.evidence.retrieval.test_metadata import fixture, raw
from tools.evidence.runner import digest


class Reply:
    def __init__(self, status: int, headers: list[tuple[str, str]], data: bytes = b'') -> None:
        self.status: int = status
        self.headers: list[tuple[str, str]] = headers
        self.data: bytes = data

    def __enter__(self) -> 'Reply':
        return self

    def __exit__(self, kind: type[BaseException] | None, value: BaseException | None,
                 traceback: TracebackType | None) -> None:
        return None

    def getheaders(self) -> list[tuple[str, str]]:
        return self.headers

    def getheader(self, key: str, default: str = '') -> str:
        return next((v for k, v in self.headers if k.lower() == key.lower()), default)

    def read(self, limit: int) -> bytes:
        return self.data[:limit]


class Connection:
    replies: list[Reply] = []
    requests: list[tuple[str, str, dict[str, str]]] = []

    def __init__(self, host: str, *, timeout: int) -> None:
        self.host: str = host
        self.timeout: int = timeout

    def request(self, method: str, path: str, *, headers: dict[str, str]) -> None:
        if method != 'GET':
            raise AssertionError('unexpected mutation')
        self.requests.append((self.host, path, headers))

    def getresponse(self) -> Reply:
        return self.replies.pop(0)

    def close(self) -> None:
        return None


class NetworkTests(unittest.TestCase):
    @override
    def setUp(self) -> None:
        Connection.requests = []
        Connection.replies = []

    def test_redirect_does_not_receive_authentication(self) -> None:
        pin, artifact, run = fixture()
        data = b'x' * 100
        pin = replace(pin, archive_sha256=digest(data))
        artifact['digest'] = 'sha256:' + pin.archive_sha256
        Connection.replies = [
            Reply(200, [('Content-Type', 'application/json')], raw(artifact)),
            Reply(200, [('Content-Type', 'application/json')], raw(run)),
            Reply(302, [('Location', 'https://productionresults1.blob.core.windows.net/artifact.zip?sig=fixture')]),
            Reply(200, [('Content-Length', '100')], data)]
        with patch.object(network, 'HTTPSConnection', Connection), patch(
                'tools.evidence.retrieval.network.time.time', return_value=timestamp('2026-10-10T12:03:00Z')):
            result = network.direct(pin, 'synthetic-token')
        self.assertEqual(result.archive, data)
        self.assertEqual(len(Connection.requests), 4)
        for host, _, headers in Connection.requests[:3]:
            self.assertEqual(host, 'api.github.com')
            self.assertEqual(headers['Authorization'], 'Bearer synthetic-token')
        self.assertNotIn('Authorization', Connection.requests[3][2])

    def test_rejects_unapproved_redirect_destinations(self) -> None:
        for url in ('http://productionresults1.blob.core.windows.net/file',
                    'https://example.invalid/file', '\0https://productionresults1.blob.core.windows.net/file',
                    'https://@productionresults1.blob.core.windows.net/file', 'https://127.0.0.1/file',
                    'https://productionresults1.blob.core.windows.net.evil.invalid/file',
                    'https://user:password@productionresults1.blob.core.windows.net/file',
                    'https://productionresults1.blob.core.windows.net:444/file',
                    'https://productionresults1.blob.core.windows.net/file#fragment'):
            with self.assertRaises(ValueError):
                _ = network.blob_endpoint(url)

    def test_bad_http_metadata_fails_without_blob_request(self) -> None:
        pin, _, _ = fixture()
        for response in (Reply(410, []), Reply(200, [('Content-Type', 'text/html')]),
                         Reply(200, [('Content-Type', 'application/json'), ('Content-Length', '65537')]),
                         Reply(200, [('Content-Type', 'application/json'), ('Content-Encoding', 'gzip')]),
                         Reply(200, [('Content-Type', 'application/json'), ('Content-Length', '2'), ('Transfer-Encoding', 'chunked')]),
                         Reply(200, [('Content-Type', 'application/json'), ('Content-Length', '3')], b'{}')):
            Connection.replies = [response]
            with patch.object(network, 'HTTPSConnection', Connection), self.assertRaises(ValueError):
                _ = network.direct(pin, 'synthetic-token')

    def test_real_http_chunk_framing(self) -> None:
        class SocketFixture:
            def __init__(self, wire: bytes) -> None:
                self.wire: bytes = wire
            def makefile(self, mode: str) -> io.BytesIO:
                if mode != 'rb':
                    raise AssertionError('unexpected mode')
                return io.BytesIO(self.wire)
        def response(wire: bytes) -> HTTPResponse:
            reply = HTTPResponse(cast(socket.socket, cast(object, SocketFixture(wire))))
            reply.begin()
            return reply
        prefix = b'HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n'
        with response(prefix + b'2\r\n{}\r\n0\r\n\r\n') as reply:
            self.assertEqual(network.body(reply, 2), b'{}')
        for malformed in (b'2\r\n{}XX0\r\n\r\n', b'2\n{}\r\n0\r\n\r\n',
                          b'2\r\n{}\r\n0\r\nSecret: fixture\r\n\r\n', b'3\r\nabc\r\n0\r\n\r\n'):
            with response(prefix + malformed) as reply, self.assertRaises(ValueError):
                _ = network.body(reply, 2)

    def test_worker_deadline_and_failures_are_sanitized(self) -> None:
        pin, _, _ = fixture()
        with patch('tools.evidence.retrieval.network.subprocess.run', side_effect=subprocess.TimeoutExpired('secret-token', 45)):
            with self.assertRaisesRegex(network.RetrievalError, '^artifact retrieval failed or exceeded deadline$'):
                _ = network.fetch(pin, 'synthetic-token')
        failed = subprocess.CompletedProcess(['worker'], 1, stdout=b'secret-token')
        with patch('tools.evidence.retrieval.network.subprocess.run', return_value=failed):
            with self.assertRaisesRegex(network.RetrievalError, '^artifact retrieval failed or exceeded deadline$'):
                _ = network.fetch(pin, 'synthetic-token')
        # Invalid real worker input exits without emitting request details.
        done = subprocess.run([sys.executable, '-I', str(Path(network.__file__).with_name('worker.py'))],
                              input=json.dumps({'invalid': 'synthetic-secret'}).encode(), capture_output=True,
                              timeout=10, check=False)
        self.assertNotEqual(done.returncode, 0)
        self.assertEqual(done.stdout, b'')
        self.assertEqual(done.stderr, b'')


if __name__ == '__main__':
    _ = unittest.main()
