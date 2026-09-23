"""Bind Pages responses to their request; never infer current publication."""
from dataclasses import dataclass
from enum import Enum
import re
from collections.abc import Mapping

from journal.model import decode, hex_id
from payload import checked, digest
from tools.serialization.json import JsonValue, object_value

MAX_RESPONSE = 64 * 1024


def component(value: JsonValue) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", value):
        raise ValueError("invalid deployment identifier")
    return value


def endpoint(owner: str, repository: str, deployment_id: str) -> str:
    for value in (owner, repository):
        checked(re.fullmatch(r"[A-Za-z0-9_.-]{1,100}", value)
                and value not in (".", ".."), "invalid repository identity")
    return f"https://api.github.com/repos/{owner}/{repository}/pages/deployments/{component(deployment_id)}"


def response(raw: bytes) -> Mapping[str, JsonValue]:
    checked(0 < len(raw) <= MAX_RESPONSE, "response size limit")
    return object_value(decode(raw))


@dataclass(frozen=True, slots=True)
class Receipt:
    deployment_id: str
    status_endpoint: str
    response_sha256: str

    def __post_init__(self) -> None:
        self.validate()

    def validate(self) -> None:
        _ = component(self.deployment_id)
        hex_id(self.response_sha256, 64)
        match = re.fullmatch(r"https://api[.]github[.]com/repos/([^/]+)/([^/]+)/pages/deployments/([^/]+)", self.status_endpoint)
        if match is None:
            raise ValueError("invalid receipt endpoint")
        checked(self.status_endpoint == endpoint(match[1], match[2], self.deployment_id),
                "receipt deployment identity mismatch")


def created(raw: bytes, *, owner: str, repository: str) -> Receipt:
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


@dataclass(frozen=True, slots=True)
class Observation:
    deployment_id: str
    phase: Phase
    status: str
    response_sha256: str


def observed(raw: bytes, *, receipt: Receipt, request_url: str) -> Observation:
    checked(request_url == receipt.status_endpoint,
            "status response request identity mismatch")
    receipt.validate()
    value = response(raw)
    status = value.get("status")
    if not isinstance(status, str) or not re.fullmatch(r"[a-z_]{1,80}", status):
        raise ValueError("invalid status")
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
