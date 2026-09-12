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
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import build_opener, HTTPRedirectHandler, ProxyHandler, Request

from payload import checked, digest, path_name, snapshot, unique_object


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def endpoint(url, base, local):
    checked(isinstance(base, str) and base.startswith('/') and base.endswith('/'), 'invalid base')
    if base != '/':
        path_name(base[1:-1])
    checked(isinstance(url, str) and url.isascii() and not any(c.isspace() for c in url), 'invalid URL')
    parsed = urlsplit(url)
    checked(not parsed.username and not parsed.password and not parsed.query and not parsed.fragment,
            'URL contains credentials, query or fragment')
    checked(parsed.path == base and parsed.hostname, 'URL/base mismatch')
    if local:
        checked(parsed.scheme == 'http' and parsed.hostname == '127.0.0.1' and parsed.port,
                'local test requires explicit IPv4 loopback HTTP port')
    else:
        checked(parsed.scheme == 'https' and parsed.port in (None, 443), 'HTTPS required')
        checked(re.fullmatch(r'[a-z0-9.-]+', parsed.hostname), 'invalid hostname')
    return url


def observe(root, identity, url, local, deadline):
    files = snapshot(root, identity)
    build = json.loads(files['build.json'].decode('utf-8'), object_pairs_hook=unique_object)
    checked(build.get('capability') == 'docs-only', 'interactive smoke requires runtime/browser checks')
    commit = build.get('source_commit')
    checked(isinstance(commit, str) and re.fullmatch(r'[0-9a-f]{40}', commit), 'invalid source commit')
    endpoint(url, build.get('base_path'), local)
    # Never inherit proxy credentials, redirect to a different origin, or send
    # cookies. HTTPS uses urllib's default certificate/hostname verification.
    opener = build_opener(ProxyHandler({}), NoRedirect())
    rows = []

    def get(route, expected=None, *, cache_bust=False):
        remaining = deadline - time.monotonic()
        checked(remaining > 0, 'smoke deadline exceeded')
        request_url = url + route + ('?nepl3-smoke=' + identity if cache_bust else '')
        request = Request(request_url, headers={'Cache-Control': 'no-cache',
                          'Pragma': 'no-cache', 'Accept-Encoding': 'identity',
                          'User-Agent': 'NEPL3-doc-smoke/1'})
        try:
            response = opener.open(request, timeout=min(10, remaining))
        except HTTPError as error:
            response = error
        with response:
            status = response.status
            if expected is None:
                checked(status == 404, 'unknown route did not return 404')
                rows.append(dict(route=route, url=request_url, status=status))
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
            rows.append(dict(route=route, url=request_url, status=status, bytes=len(data), sha256=digest(data), mime=mime))

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
    return dict(version=1, result='passed', kind='docs-only-http-byte-check',
                transport='loopback-http' if local else 'https', url=url,
                source_commit=commit, manifest_sha256=identity, observations=rows,
                omitted_control_files=['.nojekyll'], publication_verified=False)


def run(root, identity, url, *, local=False, timeout=300):
    checked(type(timeout) in (int, float) and 0 < timeout <= 300, 'invalid smoke timeout')
    checked(isinstance(url, str) and len(url) <= 8192, 'URL limit')
    context = dict(url=url, manifest_sha256=identity,
                   transport='loopback-http' if local else 'https', publication_verified=False)
    command = [sys.executable, str(Path(__file__).resolve()), str(root), url,
               '--manifest-sha256', identity, '--timeout', str(timeout), '--worker']
    if local:
        command.append('--local-http')
    try:
        result = subprocess.run(command, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        return dict(version=1, result='failed', reason='deadline', **context)
    if result.returncode != 0:
        return dict(version=1, result='failed', reason='worker-failed', exit_code=result.returncode, **context)
    report = json.loads(result.stdout)
    report.update(context)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('site', type=Path)
    parser.add_argument('url')
    parser.add_argument('--manifest-sha256', required=True)
    parser.add_argument('--timeout', type=float, default=300)
    parser.add_argument('--local-http', action='store_true')
    parser.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.worker:
        try:
            result = observe(args.site, args.manifest_sha256, args.url, args.local_http,
                             time.monotonic() + args.timeout)
        except Exception as error:
            result = dict(version=1, result='failed', reason=type(error).__name__,
                          detail=str(error)[:4096], publication_verified=False)
    else:
        result = run(args.site, args.manifest_sha256, args.url, local=args.local_http, timeout=args.timeout)
    print(json.dumps(result, sort_keys=True))
    # Worker reports are data; outer command's failed observation must fail CI.
    return 0 if args.worker or result['result'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
