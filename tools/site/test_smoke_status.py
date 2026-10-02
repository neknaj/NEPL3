"""HTTP diagnostic regression tests with in-memory responses, without a server."""
from http.client import HTTPMessage
from io import BytesIO
import json
from pathlib import Path
import time
import unittest
from urllib.error import HTTPError
from urllib.request import Request
from unittest.mock import patch

from observation import Passed
from smoke import observe


class Responses:
    def __init__(self, files: dict[str, bytes], bad_route: str | None,
                 bad_status: int) -> None:
        self.files: dict[str, bytes] = files
        self.bad_route: str | None = bad_route
        self.bad_status: int = bad_status
        self.seen: list[str] = []

    def open(self, request: Request, timeout: float) -> HTTPError:
        if timeout <= 0:
            raise ValueError('nonpositive timeout')
        route = request.full_url.removeprefix('https://example.test/NEPL3/').split('?')[0]
        self.seen.append(route)
        body = self.files.get(route)
        status = 200 if body is not None else 404
        if route == self.bad_route:
            status = self.bad_status
        headers = HTTPMessage()
        headers['Content-Type'] = 'text/html' if route.endswith('.html') else 'application/json'
        # HTTPError is also an HTTP response. Raising it exercises the same
        # urllib branch as a real non-2xx response without any network access.
        raise HTTPError(request.full_url, status, 'fixture', headers, BytesIO(body or b''))


class SmokeStatusTests(unittest.TestCase):
    def files(self) -> dict[str, bytes]:
        return {
            'build.json': json.dumps(dict(capability='docs-only',
                source_commit='a' * 40, base_path='/NEPL3/')).encode(),
            'docs/spec/08-completion.html': b'<p>WholeInput</p>',
        }

    def test_failed_page_retains_exact_http_status_and_route(self) -> None:
        route = 'docs/spec/08-completion.html'
        for status in [301, 403, 404, 429, 503]:
            with self.subTest(status=status):
                files = self.files()
                response = Responses(files, route, status)
                with patch('smoke.snapshot', return_value=files), \
                     patch('smoke.build_opener', return_value=response):
                    with self.assertRaises(ValueError) as caught:
                        _ = observe(Path('unused'), 'b' * 64,
                            'https://example.test/NEPL3/', False, time.monotonic() + 10)
                self.assertEqual(str(caught.exception), f'HTTP {status} for {route}; expected 200')
                self.assertIn(route, response.seen)

    def test_unknown_route_retains_received_status(self) -> None:
        files = self.files()
        route = '__nepl3_missing_' + 'b' * 64
        response = Responses(files, route, 200)
        with patch('smoke.snapshot', return_value=files), \
             patch('smoke.build_opener', return_value=response):
            with self.assertRaises(ValueError) as caught:
                _ = observe(Path('unused'), 'b' * 64,
                    'https://example.test/NEPL3/', False, time.monotonic() + 10)
        self.assertEqual(str(caught.exception), f'HTTP 200 for {route}; expected 404 for unknown route')

    def test_matching_bytes_and_expected_missing_route_still_pass(self) -> None:
        files = self.files()
        response = Responses(files, None, 0)
        with patch('smoke.snapshot', return_value=files), \
             patch('smoke.build_opener', return_value=response):
            result = observe(Path('unused'), 'b' * 64,
                'https://example.test/NEPL3/', False, time.monotonic() + 10)
        self.assertIsInstance(result, Passed)
        self.assertEqual(response.seen.count('build.json'), 3)
        self.assertEqual(result.source_commit, 'a' * 40)


if __name__ == '__main__':
    _ = unittest.main()
