"""Bounded HTTP byte checks for a checked docs-only site.

HTTPS observations are not Pages control-plane identity or LKG proof. The
publisher must combine them with its journal, deployment receipt and browser
checks. Explicit loopback HTTP mode is exclusively local test evidence.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import time
from http.client import HTTPMessage, HTTPResponse
from typing import IO, override
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import build_opener, HTTPRedirectHandler, ProxyHandler, Request

from payload import checked, digest, path_name, snapshot
from observation import Content, Context, Deadline, Failed, Missing, Observation, ObservationError, Passed, Report, WorkerExit, worker_report
from tools.serialization.json import decode, object_value, string


class NoRedirect(HTTPRedirectHandler):
    @override
    def redirect_request(self, req: Request, fp: IO[bytes], code: int, msg: str,
                         headers: HTTPMessage, newurl: str) -> None:
        return None


def endpoint(url: str, base: object, local: bool) -> str:
    if not isinstance(base, str) or not base.startswith('/') or not base.endswith('/'):
        raise ValueError('invalid base')
    if base != '/':
        _ = path_name(base[1:-1])
    checked(url.isascii() and not any(c.isspace() for c in url), 'invalid URL')
    parsed = urlsplit(url)
    checked(not parsed.username and not parsed.password and not parsed.query and not parsed.fragment,
            'URL contains credentials, query or fragment')
    if parsed.path != base or not parsed.hostname:
        raise ValueError('URL/base mismatch')
    if local:
        checked(parsed.scheme == 'http' and parsed.hostname == '127.0.0.1' and parsed.port,
                'local test requires explicit IPv4 loopback HTTP port')
    else:
        checked(parsed.scheme == 'https' and parsed.port in (None, 443), 'HTTPS required')
        checked(re.fullmatch(r'[a-z0-9.-]+', parsed.hostname), 'invalid hostname')
    return url


def observe(root: Path, identity: str, url: str, local: bool, deadline: float) -> Passed:
    files = snapshot(root, identity)
    build = object_value(decode(files['build.json'], reject_duplicates=True))
    checked(build.get('capability') == 'docs-only', 'interactive smoke requires runtime/browser checks')
    commit = string(build.get('source_commit'))
    checked(re.fullmatch(r'[0-9a-f]{40}', commit), 'invalid source commit')
    _ = endpoint(url, build.get('base_path'), local)
    # Never inherit proxy credentials, redirect to a different origin, or send
    # cookies. HTTPS uses urllib's default certificate/hostname verification.
    opener = build_opener(ProxyHandler({}), NoRedirect())
    rows: list[Observation] = []

    def get(route: str, expected: bytes | None = None, *, cache_bust: bool = False) -> None:
        remaining = deadline - time.monotonic()
        checked(remaining > 0, 'smoke deadline exceeded')
        request_url = url + route + ('?nepl3-smoke=' + identity if cache_bust else '')
        request = Request(request_url, headers={'Cache-Control': 'no-cache',
                          'Pragma': 'no-cache', 'Accept-Encoding': 'identity',
                          'User-Agent': 'NEPL3-doc-smoke/1'})
        try:
            # urllib's stub returns Any for all protocols. This endpoint permits
            # only HTTP(S), whose documented response is HTTPResponse.
            response: object = opener.open(request, timeout=min(10, remaining))  # pyright: ignore[reportAny]
        except HTTPError as error:
            response = error
        if not isinstance(response, (HTTPResponse, HTTPError)):
            raise ValueError('unexpected HTTP response type')
        with response:
            status = response.status
            if expected is None:
                checked(status == 404, 'unknown route did not return 404')
                rows.append(Missing(route, request_url))
                return
            checked(status == 200, 'non-200 response for ' + route)
            checked(response.headers.get('Content-Encoding', 'identity') == 'identity', 'unexpected content encoding')
            data = response.read(len(expected) + 1)
            checked(data == expected, 'published byte mismatch for ' + route)
            mime = response.headers.get_content_type()
            if route.endswith('.html') or route.endswith('/') or route == '':
                checked(mime == 'text/html', 'wrong HTML MIME')
            elif route.endswith('.css'):
                checked(mime == 'text/css', 'wrong CSS MIME')
            rows.append(Content(route, request_url, len(data), digest(data), mime))

    # Check the build identity before and after the other files. A matching
    # observation cannot prove that all CDN caches switched atomically.
    get('build.json', files['build.json'], cache_bust=True)
    for name, data in sorted(files.items()):
        if name != '.nojekyll':
            get(name, data)
        if name == 'index.html':
            get('', data)
        elif name.endswith('/index.html'):
            get(name[:-10], data)
    missing = '__nepl3_missing_' + identity
    checked(missing not in files, 'missing-route probe collision')
    get(missing)
    get('build.json', files['build.json'], cache_bust=True)
    checked(time.monotonic() <= deadline, 'smoke deadline exceeded')
    return Passed(Context(url, identity, 'loopback-http' if local else 'https'), commit, tuple(rows))


def run(root: Path, identity: str, url: str, *, local: bool = False,
        timeout: float = 300) -> Report:
    checked(type(timeout) in (int, float) and 0 < timeout <= 300, 'invalid smoke timeout')
    checked(len(url) <= 8192, 'URL limit')
    context = Context(url, identity, 'loopback-http' if local else 'https')
    command = [sys.executable, str(Path(__file__).resolve()), str(root), url,
               '--manifest-sha256', identity, '--timeout', str(timeout), '--worker']
    if local:
        command.append('--local-http')
    try:
        result = subprocess.run(command, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        return Failed(Deadline(), context)
    if result.returncode != 0:
        return Failed(WorkerExit(result.returncode), context)
    return worker_report(result.stdout, context)


class Arguments(argparse.Namespace):
    site: Path = Path()
    url: str = ''
    manifest_sha256: str = ''
    timeout: float = 300
    local_http: bool = False
    worker: bool = False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument('site', type=Path)
    _ = parser.add_argument('url')
    _ = parser.add_argument('--manifest-sha256', required=True)
    _ = parser.add_argument('--timeout', type=float, default=300)
    _ = parser.add_argument('--local-http', action='store_true')
    _ = parser.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args(namespace=Arguments())
    result: Report
    if args.worker:
        try:
            result = observe(args.site, args.manifest_sha256, args.url, args.local_http,
                             time.monotonic() + args.timeout)
        except Exception as error:
            result = Failed(ObservationError(type(error).__name__, str(error)[:4096]))
    else:
        result = run(args.site, args.manifest_sha256, args.url, local=args.local_http, timeout=args.timeout)
    print(json.dumps(result.representation(), sort_keys=True))
    # Worker reports are data; outer command's failed observation must fail CI.
    return 0 if args.worker or isinstance(result, Passed) else 1


if __name__ == '__main__':
    sys.exit(main())
