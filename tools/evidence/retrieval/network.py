"""Fixed-origin authenticated GETs with a killable total worker deadline."""
from dataclasses import asdict, dataclass
import base64
from http.client import HTTPSConnection, HTTPException, HTTPResponse
import json
from pathlib import Path
import re
import subprocess
import sys
import time
from urllib.parse import urlsplit

from tools.evidence.archive import checked
from tools.evidence.retrieval.metadata import verify
from tools.evidence.retrieval.pin import Pin
from tools.evidence.runner import digest
from tools.serialization.json import decode, object_value, string


class RetrievalError(ValueError):
    """Sanitized transport failure; never includes token or signed URL."""


@dataclass(frozen=True, slots=True)
class Download:
    artifact: bytes
    run: bytes
    archive: bytes


def chunks(reply: HTTPResponse, limit: int) -> bytes:
    stream = reply.fp
    output = bytearray()
    for _ in range(4096):
        line = stream.readline(1025)
        checked(re.fullmatch(rb'[0-9A-Fa-f]{1,8}\r\n', line), 'invalid artifact chunk size line')
        size = int(line[:-2], 16)
        if size == 0:
            checked(stream.readline(1025) == b'\r\n', 'artifact chunk trailers unsupported')
            return bytes(output)
        checked(size <= limit - len(output), 'artifact chunk body limit')
        block = stream.read(size + 2)
        checked(len(block) == size + 2 and block[-2:] == b'\r\n', 'invalid artifact chunk terminator')
        output.extend(block[:-2])
    raise RetrievalError('artifact chunk count limit')


def body(reply: HTTPResponse, limit: int) -> bytes:
    checked(reply.getheader('Content-Encoding', 'identity') == 'identity', 'artifact HTTP encoding')
    lengths = [value for key, value in reply.getheaders() if key.lower() == 'content-length']
    transfers = [value for key, value in reply.getheaders() if key.lower() == 'transfer-encoding']
    checked(len(lengths) <= 1 and len(transfers) <= 1 and not (lengths and transfers), 'ambiguous artifact HTTP framing')
    expected = None
    if lengths:
        checked(re.fullmatch(r'[0-9]{1,9}', lengths[0]), 'invalid artifact HTTP length')
        expected = int(lengths[0])
        checked(expected <= limit, 'artifact HTTP size limit')
    elif transfers:
        checked(transfers == ['chunked'], 'unsupported artifact transfer encoding')
    raw = chunks(reply, limit) if transfers else reply.read(limit + 1)
    checked(len(raw) <= limit and (expected is None or len(raw) == expected), 'artifact HTTP body size mismatch')
    return raw


def blob_endpoint(location: str) -> tuple[str, str]:
    checked(location.isascii() and len(location) <= 8192 and not any(ord(c) <= 32 or ord(c) == 127 for c in location),
            'invalid artifact redirect')
    parsed = urlsplit(location)
    host = parsed.hostname
    # Current GitHub Actions artifact storage. Other backends fail explicitly;
    # never broaden the redirect audience or forward the GitHub token implicitly.
    checked(parsed.scheme == 'https' and host is not None and
            re.fullmatch(r'[a-z0-9]+\.blob\.core\.windows\.net', host) and
            parsed.port in (None, 443) and parsed.username is None and parsed.password is None and
            not parsed.fragment and parsed.path.startswith('/'), 'unsupported artifact redirect host')
    if host is None:
        raise RetrievalError('artifact redirect has no host')
    return host, parsed.path + ('?' + parsed.query if parsed.query else '')


def api(path: str, token: str, *, limit: int, redirect: bool = False) -> bytes | str:
    checked(re.fullmatch(r'/repos/neknaj/NEPL3/actions/(artifacts/[1-9][0-9]*(/zip)?|runs/[1-9][0-9]*/attempts/[1-9][0-9]*)', path),
            'unsupported artifact API path')
    connection = HTTPSConnection('api.github.com', timeout=10)
    try:
        connection.request('GET', path, headers={
            'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
            'X-GitHub-Api-Version': '2026-03-10', 'User-Agent': 'NEPL3-evidence/1',
            'Accept-Encoding': 'identity', 'Cache-Control': 'no-cache'})
        with connection.getresponse() as reply:
            if redirect:
                checked(reply.status == 302, 'artifact download unavailable')
                locations = [value for key, value in reply.getheaders() if key.lower() == 'location']
                checked(len(locations) == 1, 'artifact redirect missing or ambiguous')
                _ = blob_endpoint(locations[0])
                return locations[0]
            checked(reply.status == 200, 'artifact metadata unavailable')
            checked(reply.getheader('Content-Type', '').split(';', 1)[0].strip().lower() in
                    ('application/json', 'application/vnd.github+json'), 'artifact metadata MIME')
            return body(reply, limit)
    finally:
        connection.close()


def direct(pin: Pin, token: str) -> Download:
    pin.validate()
    checked(re.fullmatch(r'[A-Za-z0-9_.-]{1,4096}', token), 'invalid artifact token')
    artifact = api(urlsplit(pin.artifact_url).path, token, limit=65536)
    run = api(urlsplit(pin.run_url).path, token, limit=65536)
    checked(isinstance(artifact, bytes) and isinstance(run, bytes), 'artifact metadata response kind')
    if not isinstance(artifact, bytes) or not isinstance(run, bytes):
        raise RetrievalError('artifact metadata response kind')
    verify(pin, artifact, run, int(time.time()))
    location = api(urlsplit(pin.artifact_url).path + '/zip', token, limit=0, redirect=True)
    if not isinstance(location, str):
        raise RetrievalError('artifact redirect response kind')
    host, path = blob_endpoint(location)
    connection = HTTPSConnection(host, timeout=10)
    try:
        # Separate connection, no Authorization, cookies, proxies or redirects.
        connection.request('GET', path, headers={'User-Agent': 'NEPL3-evidence/1', 'Accept-Encoding': 'identity'})
        with connection.getresponse() as reply:
            checked(reply.status == 200, 'artifact blob unavailable')
            archive = body(reply, pin.archive_bytes)
    finally:
        connection.close()
    checked(len(archive) == pin.archive_bytes and digest(archive) == pin.archive_sha256, 'downloaded artifact pin mismatch')
    verify(pin, artifact, run, int(time.time()))
    return Download(artifact, run, archive)


def fetch(pin: Pin, token: str) -> Download:
    pin.validate()
    checked(re.fullmatch(r'[A-Za-z0-9_.-]{1,4096}', token), 'invalid artifact token')
    fields = dict(asdict(pin), schema='nepl3.github-artifact/1', repository='neknaj/NEPL3')
    request = json.dumps({'pin': fields, 'token': token}).encode()
    try:
        completed = subprocess.run([sys.executable, '-I', str(Path(__file__).with_name('worker.py'))],
                                   input=request, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                   timeout=45, check=False)
        checked(completed.returncode == 0 and len(completed.stdout) <= 45 * 1024 * 1024,
                'artifact retrieval worker failed')
        response = object_value(decode(completed.stdout, reject_duplicates=True, reject_nonfinite=True))
        checked(set(response) == {'artifact', 'run', 'archive'}, 'artifact worker response fields')
        result = Download(*(base64.b64decode(string(response[key]), validate=True)
                            for key in ('artifact', 'run', 'archive')))
        verify(pin, result.artifact, result.run, int(time.time()))
        checked(len(result.archive) == pin.archive_bytes and digest(result.archive) == pin.archive_sha256,
                'artifact worker archive mismatch')
        return result
    except (ValueError, OSError, HTTPException, subprocess.TimeoutExpired, RecursionError):
        raise RetrievalError('artifact retrieval failed or exceeded deadline') from None
