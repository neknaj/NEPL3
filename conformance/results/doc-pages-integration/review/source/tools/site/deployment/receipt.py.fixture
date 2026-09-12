"""Bind Pages responses to their request; never infer current publication."""
from dataclasses import dataclass
from enum import Enum
import re

from journal.model import decode, hex_id
from payload import checked, digest

MAX_RESPONSE = 64 * 1024


def component(value):
    checked(isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_-]{1,128}", value),
            "invalid deployment identifier")
    return value


def endpoint(owner, repository, deployment_id):
    for value in (owner, repository):
        checked(isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_.-]{1,100}", value)
                and value not in (".", ".."), "invalid repository identity")
    return f"https://api.github.com/repos/{owner}/{repository}/pages/deployments/{component(deployment_id)}"


def response(raw):
    checked(isinstance(raw, bytes) and 0 < len(raw) <= MAX_RESPONSE, "response size limit")
    value = decode(raw)
    checked(isinstance(value, dict), "response must be an object")
    return value


@dataclass(frozen=True)
class Receipt:
    deployment_id: str
    status_endpoint: str
    response_sha256: str

    def __post_init__(self):
        self.validate()

    def validate(self):
        component(self.deployment_id)
        hex_id(self.response_sha256, 64)
        checked(isinstance(self.status_endpoint, str), "invalid receipt endpoint")
        match = re.fullmatch(r"https://api[.]github[.]com/repos/([^/]+)/([^/]+)/pages/deployments/([^/]+)", self.status_endpoint)
        checked(match is not None, "invalid receipt endpoint")
        checked(self.status_endpoint == endpoint(match[1], match[2], self.deployment_id),
                "receipt deployment identity mismatch")


def created(raw, *, owner, repository):
    value = response(raw)
    deployment_id = component(value.get("id"))
    expected = endpoint(owner, repository, deployment_id)
    # The documented creation example adds /status, whereas the documented GET
    # endpoint does not. Validate either spelling, but always construct our GET.
    checked(value.get("status_url") in (expected, expected + "/status"),
            "receipt status URL does not match repository and deployment")
    # page_url is informational: use the pinned SiteConfig for public requests.
    return Receipt(deployment_id, expected, digest(raw))


class Phase(Enum):
    PENDING = "pending"
    SUCCEEDED = "succeeded"
    FAILED = "failed"
    UNKNOWN = "unknown"


@dataclass(frozen=True)
class Observation:
    deployment_id: str
    phase: Phase
    status: str
    response_sha256: str


def observed(raw, *, receipt, request_url):
    checked(isinstance(receipt, Receipt) and request_url == receipt.status_endpoint,
            "status response request identity mismatch")
    receipt.validate()
    value = response(raw)
    status = value.get("status")
    checked(isinstance(status, str) and re.fullmatch(r"[a-z_]{1,80}", status), "invalid status")
    if status == "succeed":
        phase = Phase.SUCCEEDED
    elif status in ("deployment_queued", "deployment_in_progress"):
        phase = Phase.PENDING
    elif status in ("deployment_failed", "deployment_content_failed", "deployment_cancelled"):
        phase = Phase.FAILED
    else:
        # New server states cannot silently become success or indefinite retry.
        phase = Phase.UNKNOWN
    return Observation(receipt.deployment_id, phase, status, digest(raw))
