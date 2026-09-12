"""One Pages POST attempt; uncertain outcomes must be reconciled, never retried."""
import base64
import json
from pathlib import Path
import re
import subprocess
import sys

from journal.model import hex_id
from payload import checked
from .receipt import created, endpoint
from .transport import request


class CreationUnknown(ValueError):
    """The API may have accepted the attempt; preserve its durable intent."""


def validate(owner, repository, artifact_id, build_version, token, oidc_token, timeout):
    endpoint(owner, repository, "validate")
    checked(type(artifact_id) is int and 0 < artifact_id < 2**53, "invalid artifact ID")
    hex_id(build_version, 40)
    for secret, maximum in ((token, 4096), (oidc_token, 16384)):
        checked(isinstance(secret, str) and 0 < len(secret) <= maximum
                and re.fullmatch(r"[A-Za-z0-9_.-]+", secret), "invalid credential")
    checked(type(timeout) in (int, float) and 0 < timeout <= 10, "invalid creation timeout")


def _create(owner, repository, artifact_id, build_version, token, oidc_token, timeout):
    validate(owner, repository, artifact_id, build_version, token, oidc_token, timeout)
    body = json.dumps(dict(artifact_id=artifact_id, pages_build_version=build_version,
                           environment="github-pages", oidc_token=oidc_token)).encode()
    raw = request(f"/repos/{owner}/{repository}/pages/deployments", token, timeout=timeout, body=body)
    created(raw, owner=owner, repository=repository)
    return raw


def create(owner, repository, artifact_id, build_version, token, oidc_token, *, timeout=10):
    """Caller must durably confirm intent and all publication gates beforehand.

    This transport does not authorize deployment, obtain OIDC credentials, check
    artifact provenance, or automatically repeat any POST. Raw response bytes
    must be persisted by the publisher before continuing.
    """
    validate(owner, repository, artifact_id, build_version, token, oidc_token, timeout)
    payload = json.dumps(dict(owner=owner, repository=repository, artifact_id=artifact_id,
                              build_version=build_version, token=token,
                              oidc_token=oidc_token, timeout=timeout)).encode()
    try:
        result = subprocess.run([sys.executable, "-I", str(Path(__file__).with_name("create_worker.py"))],
                                input=payload, capture_output=True, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired):
        raise CreationUnknown("Pages creation outcome unknown; reconcile intent") from None
    if result.returncode != 0 or len(result.stdout) > 90000:
        raise CreationUnknown("Pages creation outcome unknown; reconcile intent")
    try:
        raw = base64.b64decode(result.stdout, validate=True)
        receipt = created(raw, owner=owner, repository=repository)
    except (ValueError, RecursionError):
        raise CreationUnknown("Pages creation response invalid; reconcile intent") from None
    return receipt, raw
