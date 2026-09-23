"""Isolated creation attempt; credentials only cross stdin, never argv or logs."""
import base64
from pathlib import Path
import sys
from dataclasses import dataclass, field

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from deployment.create import direct_create
from journal.model import decode
from tools.serialization.json import object_value, string, integer


@dataclass(frozen=True, slots=True)
class Request:
    owner: str
    repository: str
    artifact_id: int
    build_version: str
    token: str = field(repr=False)
    oidc_token: str = field(repr=False)
    timeout: float


def request_value(raw: bytes) -> Request:
    value = object_value(decode(raw))
    if set(value) != {'owner', 'repository', 'artifact_id', 'build_version', 'token', 'oidc_token', 'timeout'}:
        raise ValueError('invalid creation request fields')
    timeout = value['timeout']
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)):
        raise ValueError('invalid creation timeout type')
    return Request(string(value['owner']), string(value['repository']), integer(value['artifact_id']),
                   string(value['build_version']), string(value['token']), string(value['oidc_token']), timeout)


def main() -> int:
    try:
        raw = sys.stdin.buffer.read(24001)
        if len(raw) > 24000:
            return 1
        request = request_value(raw)
        response = direct_create(request.owner, request.repository, request.artifact_id, request.build_version,
                                 request.token, request.oidc_token, request.timeout)
        _ = sys.stdout.buffer.write(base64.b64encode(response))
        return 0
    except Exception:
        return 1


if __name__ == "__main__":
    sys.exit(main())
