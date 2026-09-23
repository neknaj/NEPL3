"""Record/reload a creation receipt against its preceding intent.

This records an API observation; it does not authorize creating a deployment.
"""
import base64
from dataclasses import dataclass, replace
from pathlib import Path
from collections.abc import Callable

from journal import Event, append, load
from journal.model import MAX_EVIDENCE, Snapshot, Kind, decode, encode
from payload import checked
from .receipt import Phase, Receipt, Observation, created, observed
from .transport import Result
from .poll import Fetch, Report, Stop, wait_with
from tools.serialization.json import object_value, string, integer


def record_created(mirror: Path, expected_head: str | None, intent: Event, raw: bytes,
                   *, owner: str, repository: str) -> tuple[str, Receipt]:
    intent.validate()
    checked(intent.kind in ("DeployIntent", "RecoveryIntent"), "creation requires intent")
    receipt = created(raw, owner=owner, repository=repository)
    evidence = encode(dict(version=1, owner=owner, repository=repository,
                           response=base64.b64encode(raw).decode("ascii")))
    checked(len(evidence) <= MAX_EVIDENCE, "receipt journal evidence limit")
    snapshot = load(mirror)
    checked(snapshot.head == expected_head and snapshot.events and snapshot.events[-1] == intent,
            "creation receipt does not follow the expected intent")
    kind: Kind = "DeployReceipt" if intent.kind == "DeployIntent" else "RecoveryReceipt"
    head = append(mirror, expected_head, replace(intent, kind=kind), evidence)
    return head, receipt


def latest_created(mirror: Path, *, owner: str, repository: str) -> tuple[str | None, Event, Receipt, bytes]:
    snapshot = load(mirror)
    event, receipt, raw = _created(snapshot, len(snapshot.events) - 1, owner, repository)
    return snapshot.head, event, receipt, raw


def require_creation_receipts(snapshot: Snapshot, *, owner: str, repository: str) -> None:
    """Refuse new writes while any creation acceptance remains unresolved.

    This verifies receipt pairing and original API evidence, not completion,
    current publication identity, public health, or LKG promotion. Those remain
    additional publisher gates even when every intent has a receipt.
    """
    checked(len(snapshot.events) == len(snapshot.evidence), 'journal evidence count mismatch')
    for index, event in enumerate(snapshot.events):
        if event.kind in ('DeployIntent', 'RecoveryIntent'):
            checked(index + 1 < len(snapshot.events), 'unresolved creation intent; reconcile before writing')
            _ = _created(snapshot, index + 1, owner, repository)
        elif event.kind in ('DeployReceipt', 'RecoveryReceipt'):
            # Also reject an orphan receipt or forged non-adjacent pairing.
            _ = _created(snapshot, index, owner, repository)


def _created(snapshot: Snapshot, index: int, owner: str, repository: str) -> tuple[Event, Receipt, bytes]:
    checked(index >= 1, "creation receipt missing")
    intent, event = snapshot.events[index-1:index+1]
    pairs: dict[Kind, Kind] = {"DeployIntent": "DeployReceipt", "RecoveryIntent": "RecoveryReceipt"}
    expected = pairs.get(intent.kind)
    checked(expected is not None and event == replace(intent, kind=expected),
            "creation receipt intent identity mismatch")
    value = object_value(decode(snapshot.evidence[index]))
    checked(set(value) == {"version", "owner", "repository", "response"}
            and integer(value["version"]) == 1, "receipt evidence shape")
    checked(value["owner"] == owner and value["repository"] == repository,
            "receipt evidence repository mismatch")
    raw = base64.b64decode(string(value["response"]), validate=True)
    receipt = created(raw, owner=owner, repository=repository)
    return event, receipt, raw


@dataclass(frozen=True, slots=True)
class StatusHistory:
    head: str
    event: Event
    receipt: Receipt
    creation_response: bytes
    observations: tuple[Result, ...]


def status_history(mirror: Path, *, owner: str, repository: str) -> StatusHistory:
    return _history(load(mirror), owner, repository)


def _history(snapshot: Snapshot, owner: str, repository: str) -> StatusHistory:
    index = len(snapshot.events) - 1
    while index >= 0 and snapshot.events[index].kind == "Observation":
        index -= 1
    event, receipt, original = _created(snapshot, index, owner, repository)
    observations: list[Result] = []
    for child, evidence in zip(snapshot.events[index+1:], snapshot.evidence[index+1:]):
        checked(child == replace(event, kind="Observation"), "status event identity mismatch")
        checked(not observations or observations[-1].observation.phase == Phase.PENDING,
                "status after terminal or unknown response requires reconciliation")
        value = object_value(decode(evidence))
        checked(set(value) == {"version", "request_url", "response"}
                and integer(value["version"]) == 1,
                "status evidence shape")
        raw = base64.b64decode(string(value["response"]), validate=True)
        result = observed(raw, receipt=receipt, request_url=string(value["request_url"]))
        observations.append(Result(result, raw))
    if snapshot.head is None:
        raise ValueError('creation receipt history has no head')
    return StatusHistory(snapshot.head, event, receipt, original, tuple(observations))


def record_status(mirror: Path, expected_head: str, raw: bytes, *, owner: str,
                  repository: str, request_url: str) -> tuple[str, Observation]:
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


def wait_recorded(mirror: Path, expected_head: str, token: str, *, owner: str,
                  repository: str, remaining_seconds: float) -> tuple[str, Report]:
    import time
    from .transport import status
    return wait_recorded_with(mirror, expected_head, token, owner=owner, repository=repository,
                          remaining_seconds=remaining_seconds, clock=time.monotonic,
                          sleep=time.sleep, fetch=status)


def wait_remote(mirror: Path, expected_head: str, token: str, *, expected_url: str,
                owner: str, repository: str, remaining_seconds: float) -> tuple[str, Report]:
    """Persist each observation remotely before fetching again or returning success."""
    import time
    from .transport import status
    from journal import remote

    def confirm_start() -> None:
        checked(remote.head(mirror, expected_url) == expected_head,
                "remote receipt journal changed; reconcile before polling")

    def confirm_record(previous: str, current: str) -> None:
        _ = remote.publish(mirror, expected_url, previous, current)

    return wait_recorded_with(mirror, expected_head, token, owner=owner, repository=repository,
                          remaining_seconds=remaining_seconds, clock=time.monotonic,
                          sleep=time.sleep, fetch=status, before_wait=confirm_start,
                          after_record=confirm_record)


def wait_recorded_with(mirror: Path, expected_head: str, token: str, *, owner: str, repository: str,
                       remaining_seconds: float, clock: Callable[[], float], sleep: Callable[[float], None],
                       fetch: Fetch, before_wait: Callable[[], None] | None = None,
                       after_record: Callable[[str, str], None] | None = None) -> tuple[str, Report]:
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

    def persist(result: Result) -> None:
        nonlocal head
        previous = head
        head, _ = record_status(mirror, head, result.raw_response, owner=owner,
                                repository=repository, request_url=history.receipt.status_endpoint)
        if after_record is not None:
            after_record(previous, head)

    if clock() >= deadline:
        return head, Report(history.receipt.deployment_id, Stop.DEADLINE, ())
    report = wait_with(history.receipt, token, timeout=remaining_seconds, clock=clock,
                   sleep=sleep, fetch=fetch, on_response=persist, absolute_deadline=deadline)
    return head, report
