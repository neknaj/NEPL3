"""Record/reload a creation receipt against its preceding intent.

This records an API observation; it does not authorize creating a deployment.
"""
import base64
from dataclasses import dataclass, replace

from journal import Event, append, load
from journal.model import MAX_EVIDENCE, decode, encode
from payload import checked
from .receipt import Phase, Receipt, created, observed
from .transport import Result


def record_created(mirror, expected_head, intent, raw, *, owner, repository):
    checked(isinstance(intent, Event), "typed intent required")
    intent.validate()
    checked(intent.kind in ("DeployIntent", "RecoveryIntent"), "creation requires intent")
    receipt = created(raw, owner=owner, repository=repository)
    evidence = encode(dict(version=1, owner=owner, repository=repository,
                           response=base64.b64encode(raw).decode("ascii")))
    checked(len(evidence) <= MAX_EVIDENCE, "receipt journal evidence limit")
    snapshot = load(mirror)
    checked(snapshot.head == expected_head and snapshot.events and snapshot.events[-1] == intent,
            "creation receipt does not follow the expected intent")
    kind = "DeployReceipt" if intent.kind == "DeployIntent" else "RecoveryReceipt"
    head = append(mirror, expected_head, replace(intent, kind=kind), evidence)
    return head, receipt


def latest_created(mirror, *, owner, repository):
    snapshot = load(mirror)
    event, receipt, raw = _created(snapshot, len(snapshot.events) - 1, owner, repository)
    return snapshot.head, event, receipt, raw


def _created(snapshot, index, owner, repository):
    checked(index >= 1, "creation receipt missing")
    intent, event = snapshot.events[index-1:index+1]
    expected = {"DeployIntent": "DeployReceipt", "RecoveryIntent": "RecoveryReceipt"}.get(intent.kind)
    checked(expected is not None and event == replace(intent, kind=expected),
            "creation receipt intent identity mismatch")
    value = decode(snapshot.evidence[index])
    checked(set(value) == {"version", "owner", "repository", "response"}
            and type(value["version"]) is int and value["version"] == 1, "receipt evidence shape")
    checked(value["owner"] == owner and value["repository"] == repository,
            "receipt evidence repository mismatch")
    checked(isinstance(value["response"], str), "receipt response must be base64")
    raw = base64.b64decode(value["response"], validate=True)
    receipt = created(raw, owner=owner, repository=repository)
    return event, receipt, raw


@dataclass(frozen=True)
class StatusHistory:
    head: str
    event: Event
    receipt: Receipt
    creation_response: bytes
    observations: tuple[Result, ...]


def status_history(mirror, *, owner, repository):
    return _history(load(mirror), owner, repository)


def _history(snapshot, owner, repository):
    index = len(snapshot.events) - 1
    while index >= 0 and snapshot.events[index].kind == "Observation":
        index -= 1
    event, receipt, original = _created(snapshot, index, owner, repository)
    observations = []
    for child, evidence in zip(snapshot.events[index+1:], snapshot.evidence[index+1:]):
        checked(child == replace(event, kind="Observation"), "status event identity mismatch")
        checked(not observations or observations[-1].observation.phase == Phase.PENDING,
                "status after terminal or unknown response requires reconciliation")
        value = decode(evidence)
        checked(set(value) == {"version", "request_url", "response"}
                and type(value["version"]) is int and value["version"] == 1,
                "status evidence shape")
        checked(isinstance(value["response"], str), "status response must be base64")
        raw = base64.b64decode(value["response"], validate=True)
        result = observed(raw, receipt=receipt, request_url=value["request_url"])
        observations.append(Result(result, raw))
    return StatusHistory(snapshot.head, event, receipt, original, tuple(observations))


def record_status(mirror, expected_head, raw, *, owner, repository, request_url):
    history = status_history(mirror, owner=owner, repository=repository)
    checked(history.head == expected_head, "stale status journal head")
    checked(not history.observations or history.observations[-1].observation.phase == Phase.PENDING,
            "status after terminal or unknown response requires reconciliation")
    result = observed(raw, receipt=history.receipt, request_url=request_url)
    evidence = encode(dict(version=1, request_url=request_url,
                           response=base64.b64encode(raw).decode("ascii")))
    checked(len(evidence) <= MAX_EVIDENCE, "status journal evidence limit")
    head = append(mirror, expected_head, replace(history.event, kind="Observation"), evidence)
    return head, result
