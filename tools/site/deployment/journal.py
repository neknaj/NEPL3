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


def wait_recorded(mirror, expected_head, token, *, owner, repository, remaining_seconds):
    import time
    from .transport import status
    return _wait_recorded(mirror, expected_head, token, owner=owner, repository=repository,
                          remaining_seconds=remaining_seconds, clock=time.monotonic,
                          sleep=time.sleep, fetch=status)


def wait_remote(mirror, expected_head, token, *, expected_url, owner, repository, remaining_seconds):
    """Persist each observation remotely before fetching again or returning success."""
    import time
    from .transport import status
    from journal import remote

    def confirm_start():
        checked(remote.head(mirror, expected_url) == expected_head,
                "remote receipt journal changed; reconcile before polling")

    def confirm_record(previous, current):
        remote.publish(mirror, expected_url, previous, current)

    return _wait_recorded(mirror, expected_head, token, owner=owner, repository=repository,
                          remaining_seconds=remaining_seconds, clock=time.monotonic,
                          sleep=time.sleep, fetch=status, before_wait=confirm_start,
                          after_record=confirm_record)


def _wait_recorded(mirror, expected_head, token, *, owner, repository,
                   remaining_seconds, clock, sleep, fetch, before_wait=None, after_record=None):
    from .poll import _wait, Report, Stop
    checked(type(remaining_seconds) in (int, float) and 0 < remaining_seconds <= 600,
            "invalid remaining status wait")
    deadline = clock() + remaining_seconds
    if before_wait is not None:
        before_wait()
    history = status_history(mirror, owner=owner, repository=repository)
    checked(history.head == expected_head, "stale wait journal head")
    checked(not history.observations or history.observations[-1].observation.phase == Phase.PENDING,
            "terminal or unknown history requires reconciliation")
    head = expected_head

    def persist(result):
        nonlocal head
        previous = head
        head, _ = record_status(mirror, head, result.raw_response, owner=owner,
                                repository=repository, request_url=history.receipt.status_endpoint)
        if after_record is not None:
            after_record(previous, head)

    if clock() >= deadline:
        return head, Report(history.receipt.deployment_id, Stop.DEADLINE, ())
    report = _wait(history.receipt, token, timeout=remaining_seconds, clock=clock,
                   sleep=sleep, fetch=fetch, on_response=persist, absolute_deadline=deadline)
    return head, report
