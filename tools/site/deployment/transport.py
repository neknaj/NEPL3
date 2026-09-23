"""Pages status GET with a killable process deadline and bounded response."""
from dataclasses import dataclass
import base64
import json
from pathlib import Path
import subprocess
import sys
from http.client import HTTPSConnection, HTTPException
import re
from urllib.parse import urlsplit

from payload import checked
from .receipt import MAX_RESPONSE, Observation, Receipt, observed


@dataclass(frozen=True, slots=True)
class Result:
    observation: Observation
    raw_response: bytes


class TransportError(ValueError):
    """Sanitized failure; never contains credentials or the server body."""


def validate_request(receipt: Receipt, token: object, timeout: float) -> str:
    receipt.validate()
    if not isinstance(token, str) or not re.fullmatch(r"[A-Za-z0-9_.-]{1,4096}", token):
        raise ValueError("invalid authentication token")
    checked(type(timeout) in (int, float) and 0 < timeout <= 10, "invalid I/O timeout")
    return token


def direct_status(receipt: Receipt, token: object, *, timeout: float = 10) -> Result:
    token = validate_request(receipt, token, timeout)
    raw = request(urlsplit(receipt.status_endpoint).path, token, timeout=timeout)
    try:
        observation = observed(raw, receipt=receipt, request_url=receipt.status_endpoint)
    except (ValueError, RecursionError):
        raise TransportError("Pages status response invalid") from None
    return Result(observation, raw)


def request(path: str, token: str, *, timeout: float, body: bytes | None = None) -> bytes:
    # Private fixed-host transport shared by status and creation adapters.
    # Callers validate the path, token, timeout and bounded request body.
    # A fixed host and direct HTTPSConnection do not inherit proxy settings,
    # cookies or redirect handlers. Default TLS certificate checks stay enabled.
    connection = HTTPSConnection("api.github.com", timeout=timeout)
    try:
        connection.request("GET" if body is None else "POST", path, body=body, headers={
            "Content-Type": "application/json",
            "Authorization": "Bearer " + token,
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2026-03-10",
            "User-Agent": "NEPL3-Pages-publisher/1",
            "Cache-Control": "no-cache",
            "Accept-Encoding": "identity",
        })
        with connection.getresponse() as reply:
            if reply.status != 200:
                raise TransportError("Pages status HTTP failure")
            if reply.getheader("Content-Encoding", "identity") != "identity":
                raise TransportError("Pages status content encoding rejected")
            mime = reply.getheader("Content-Type", "").split(";", 1)[0].strip().lower()
            if mime not in ("application/json", "application/vnd.github+json"):
                raise TransportError("Pages status MIME rejected")
            headers = reply.getheaders()
            lengths = [value for name, value in headers if name.lower() == "content-length"]
            transfers = [value for name, value in headers if name.lower() == "transfer-encoding"]
            if len(lengths) > 1 or len(transfers) > 1 or (lengths and transfers):
                raise TransportError("Pages status ambiguous framing")
            expected = None
            if lengths:
                if not re.fullmatch(r"[0-9]{1,8}", lengths[0]):
                    raise TransportError("Pages status invalid length")
                expected = int(lengths[0])
                if expected > MAX_RESPONSE:
                    raise TransportError("Pages status response limit")
            elif transfers and transfers != ["chunked"]:
                raise TransportError("Pages status transfer encoding rejected")
            raw = reply.read(MAX_RESPONSE + 1)
            if expected is not None and len(raw) != expected:
                raise TransportError("Pages status truncated response")
            if len(raw) > MAX_RESPONSE:
                raise TransportError("Pages status response limit")
        return raw
    except (OSError, HTTPException):
        raise TransportError("Pages status connection failed") from None
    finally:
        connection.close()


def status(receipt: Receipt, token: object, *, timeout: float = 10) -> Result:
    token = validate_request(receipt, token, timeout)
    request = json.dumps(dict(receipt=dict(deployment_id=receipt.deployment_id,
                        status_endpoint=receipt.status_endpoint, response_sha256=receipt.response_sha256),
                        token=token, timeout=timeout)).encode()
    try:
        completed = subprocess.run(
            [sys.executable, "-I", str(Path(__file__).with_name("worker.py"))],
            input=request, capture_output=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        raise TransportError("Pages status deadline exceeded") from None
    except OSError:
        raise TransportError("Pages status worker unavailable") from None
    if completed.returncode != 0 or len(completed.stdout) > 90000:
        raise TransportError("Pages status worker failed")
    try:
        raw = base64.b64decode(completed.stdout, validate=True)
        observation = observed(raw, receipt=receipt, request_url=receipt.status_endpoint)
    except (ValueError, RecursionError):
        raise TransportError("Pages status worker response invalid") from None
    return Result(observation, raw)
