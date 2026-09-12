"""Bounded authenticated GitHub observations; no deployment or redirects."""
import base64
from dataclasses import dataclass, asdict
from enum import Enum
import json
from pathlib import Path
import re
import subprocess
import sys

from deployment.candidate import document, integer
from deployment.receipt import endpoint, MAX_RESPONSE
from deployment.transport import request, TransportError
from payload import checked


class Kind(str, Enum):
    RUN = 'run'
    JOBS = 'jobs'
    ARTIFACT = 'artifact'
    MAIN = 'main'


@dataclass(frozen=True)
class Query:
    owner: str
    repository: str
    kind: Kind
    identity: int = 0
    attempt: int = 0

    def path(self):
        endpoint(self.owner, self.repository, '1')
        checked(isinstance(self.kind, Kind), 'invalid observation kind')
        checked(type(self.identity) is int and type(self.attempt) is int,
                'invalid observation identity')
        prefix = f'/repos/{self.owner}/{self.repository}'
        if self.kind == Kind.MAIN:
            checked(self.identity == self.attempt == 0, 'unexpected main identity')
            return prefix + '/git/ref/heads/main'
        integer(self.identity)
        if self.kind == Kind.JOBS:
            integer(self.attempt)
            return prefix + f'/actions/runs/{self.identity}/attempts/{self.attempt}/jobs?per_page=100&page=1'
        checked(self.attempt == 0, 'unexpected attempt')
        suffix = 'runs' if self.kind == Kind.RUN else 'artifacts'
        return prefix + f'/actions/{suffix}/{self.identity}'


def validate(query, token, timeout):
    checked(isinstance(query, Query), 'invalid observation query')
    query.path()
    checked(isinstance(token, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,4096}', token),
            'invalid authentication token')
    checked(type(timeout) in (int, float) and 0 < timeout <= 10, 'invalid I/O timeout')


def _read(query, token, *, timeout=10):
    validate(query, token, timeout)
    raw = request(query.path(), token, timeout=timeout)
    document(raw)
    return raw


def read(query, token, *, timeout=10):
    """Return original JSON bytes, not an eligible Candidate or permission proof.

    Each response is capped at 64 KiB. Jobs exceeding one complete page are
    rejected by candidate.verify, never silently treated as a full listing.
    The caller supplies the remaining transaction budget and retains the bytes.
    """
    validate(query, token, timeout)
    body = json.dumps(dict(query=asdict(query), token=token, timeout=timeout)).encode()
    try:
        child = subprocess.run([sys.executable, '-I', str(Path(__file__).with_name('worker.py'))],
                               input=body, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        raise TransportError('CI observation deadline exceeded') from None
    except OSError:
        raise TransportError('CI observation worker unavailable') from None
    if child.returncode != 0 or len(child.stdout) > 90000:
        raise TransportError('CI observation worker failed')
    try:
        raw = base64.b64decode(child.stdout, validate=True)
        checked(len(raw) <= MAX_RESPONSE, 'CI observation response limit')
        document(raw)
    except (ValueError, RecursionError):
        raise TransportError('CI observation worker response invalid') from None
    return raw
