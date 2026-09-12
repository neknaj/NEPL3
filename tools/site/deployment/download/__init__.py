"""Fetch one Actions ZIP with bounded size, digest and process lifetime."""
import hashlib
from http.client import HTTPSConnection
import json
from pathlib import Path
import re
import subprocess
import sys
import time
from urllib.parse import urlsplit

from deployment.artifact import MAX_ARCHIVE
from deployment.observations import Query, Kind, validate as validate_query
from deployment.transport import TransportError
from journal.model import hex_id
from payload import checked


def validate(query, token, size, sha256, timeout):
    validate_query(query, token, min(timeout, 10) if type(timeout) in (int, float) else timeout)
    checked(query.kind == Kind.ARTIFACT, 'download requires artifact identity')
    checked(type(size) is int and 0 < size <= MAX_ARCHIVE, 'archive size limit')
    hex_id(sha256, 64)
    checked(type(timeout) in (int, float) and 0 < timeout <= 60, 'invalid download timeout')


def destination(location):
    checked(isinstance(location, str) and 0 < len(location) <= 8192
            and all(33 <= ord(c) <= 126 for c in location) and '\\' not in location,
            'invalid archive redirect')
    target = urlsplit(location)
    # GitHub's authenticated redirect selects a signed storage URL. Only the
    # supported Azure storage destination is accepted; changes fail visibly.
    checked(target.scheme == 'https' and target.username is None and target.password is None
            and target.port is None and not target.fragment
            and re.fullmatch(r'[a-z0-9]{3,24}[.]blob[.]core[.]windows[.]net', target.netloc)
            and target.path.startswith('/'), 'unsupported archive destination')
    return target


def remaining(deadline):
    value = deadline - time.monotonic()
    checked(value > 0, 'download deadline exceeded')
    return min(value, 10)


def _download(query, token, *, size, sha256, timeout=60):
    validate(query, token, size, sha256, timeout)
    deadline = time.monotonic() + timeout
    connection = HTTPSConnection('api.github.com', timeout=remaining(deadline))
    try:
        connection.request('GET', query.path() + '/zip', headers={
            'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
            'X-GitHub-Api-Version': '2026-03-10', 'User-Agent': 'NEPL3-Pages-publisher/1',
            'Accept-Encoding': 'identity', 'Cache-Control': 'no-cache'})
        with connection.getresponse() as reply:
            locations = reply.headers.get_all('Location', [])
            checked(reply.status == 302 and len(locations) == 1, 'archive redirect unavailable')
            target = destination(locations[0])
    finally:
        connection.close()
    connection = HTTPSConnection(target.hostname, timeout=remaining(deadline))
    try:
        # A new connection with a new header set: no GitHub credential, cookies,
        # referrer or follow-up redirect is sent to the signed storage URL.
        path = target.path + ('?' + target.query if target.query else '')
        connection.request('GET', path, headers={'Accept-Encoding': 'identity',
                                                'User-Agent': 'NEPL3-Pages-publisher/1'})
        with connection.getresponse() as reply:
            checked(reply.status == 200, 'archive download HTTP failure')
            checked(reply.getheader('Content-Encoding', 'identity') == 'identity', 'archive encoding')
            mime = reply.getheader('Content-Type', '').split(';', 1)[0].strip().lower()
            checked(mime in ('application/zip', 'application/octet-stream', 'application/x-zip-compressed'),
                    'archive MIME rejected')
            lengths = reply.headers.get_all('Content-Length', [])
            transfers = reply.headers.get_all('Transfer-Encoding', [])
            checked(len(lengths) <= 1 and len(transfers) <= 1 and not (lengths and transfers),
                    'archive ambiguous framing')
            if lengths:
                checked(re.fullmatch(r'[0-9]{1,9}', lengths[0]) and int(lengths[0]) == size,
                        'archive length mismatch')
            elif transfers:
                checked(transfers == ['chunked'], 'archive transfer encoding')
            raw = reply.read(size + 1)
            checked(len(raw) == size and hashlib.sha256(raw).hexdigest() == sha256,
                    'archive bytes mismatch')
            remaining(deadline)
            return raw
    finally:
        connection.close()


def download(query, token, *, size, sha256, timeout=60):
    """Return digest-checked ZIP bytes; candidate.prepare_upload checks contents.

    Size/digest must come from authenticated artifact metadata. This function
    does not establish CI eligibility or authorize any publication.
    """
    from dataclasses import asdict
    validate(query, token, size, sha256, timeout)
    request = json.dumps(dict(query=asdict(query), token=token, size=size,
                              sha256=sha256, timeout=timeout)).encode()
    try:
        child = subprocess.run([sys.executable, '-I', str(Path(__file__).with_name('worker.py'))],
                               input=request, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        raise TransportError('archive download deadline exceeded') from None
    except OSError:
        raise TransportError('archive download worker unavailable') from None
    if child.returncode != 0 or len(child.stdout) != size or hashlib.sha256(child.stdout).hexdigest() != sha256:
        raise TransportError('archive download failed')
    return child.stdout
