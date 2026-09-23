"""Persist an authorized attempt before its single API call and bind its receipt.

The publisher owns eligibility, artifact provenance, lock and remote protection.
This function enforces ordering and per-intent replay rejection, not those gates.
"""
import time
from pathlib import Path
from collections.abc import Callable
from typing import Protocol

from journal import Event, append, load, remote
from journal.model import encode
from payload import checked
from .create import create, validate, CreationUnknown
from .journal import record_created, require_creation_receipts
from .receipt import Receipt
from .poll import Report


class Submit(Protocol):
    def __call__(self, mirror: Path, expected_head: str | None, intent: Event, /, *,
                 expected_url: str, owner: str, repository: str, artifact_id: int,
                 token: str, oidc_token: str, remaining_seconds: float) -> tuple[str, Receipt]: ...


class Wait(Protocol):
    def __call__(self, mirror: Path, expected_head: str, token: str, /, *, expected_url: str,
                 owner: str, repository: str, remaining_seconds: float) -> tuple[str, Report]: ...


class Send(Protocol):
    def __call__(self, owner: str, repository: str, artifact_id: int, build_version: str,
                 token: str, oidc_token: str, /, *, timeout: float) -> tuple[Receipt, bytes]: ...


def execute(mirror: Path, expected_head: str | None, intent: Event, *, expected_url: str,
            owner: str, repository: str, artifact_id: int, token: str, oidc_token: str,
            remaining_seconds: float) -> tuple[str, Report]:
    """Submit once, then await remotely recorded status within the same budget.

    A succeeded status is not public smoke, LKG promotion or proof of the current
    publication. The caller retains all authorization and reconciliation duties.
    """
    from .journal import wait_remote
    return execute_with(mirror, expected_head, intent, expected_url=expected_url,
                    owner=owner, repository=repository, artifact_id=artifact_id,
                    token=token, oidc_token=oidc_token, remaining_seconds=remaining_seconds,
                    clock=time.monotonic, submit_attempt=submit, wait_attempt=wait_remote)


def execute_with(mirror: Path, expected_head: str | None, intent: Event, *, expected_url: str,
                 owner: str, repository: str, artifact_id: int, token: str, oidc_token: str,
                 remaining_seconds: float, clock: Callable[[], float], submit_attempt: Submit,
                 wait_attempt: Wait) -> tuple[str, Report]:
    checked(type(remaining_seconds) in (int, float) and 0 < remaining_seconds <= 3600,
            "invalid remaining publication budget")
    deadline = clock() + remaining_seconds
    head, _ = submit_attempt(mirror, expected_head, intent, expected_url=expected_url,
                             owner=owner, repository=repository, artifact_id=artifact_id,
                             token=token, oidc_token=oidc_token, remaining_seconds=remaining_seconds)
    remaining = deadline - clock()
    if remaining <= 0:
        raise CreationUnknown("publication deadline before polling; reconcile recorded receipt")
    return wait_attempt(mirror, head, token, expected_url=expected_url, owner=owner,
                        repository=repository, remaining_seconds=min(600, remaining))


def submit(mirror: Path, expected_head: str | None, intent: Event, *, expected_url: str,
           owner: str, repository: str, artifact_id: int, token: str, oidc_token: str,
           remaining_seconds: float) -> tuple[str, Receipt]:
    return submit_with(mirror, expected_head, intent, expected_url=expected_url,
                   owner=owner, repository=repository, artifact_id=artifact_id,
                   token=token, oidc_token=oidc_token, remaining_seconds=remaining_seconds,
                   clock=time.monotonic, send=create)


def submit_with(mirror: Path, expected_head: str | None, intent: Event, *, expected_url: str,
                owner: str, repository: str, artifact_id: int, token: str, oidc_token: str,
                remaining_seconds: float, clock: Callable[[], float], send: Send) -> tuple[str, Receipt]:
    checked(type(remaining_seconds) in (int, float) and 0 < remaining_seconds <= 3600,
            "invalid remaining publication budget")
    deadline = clock() + remaining_seconds
    intent.validate()
    checked(intent.kind in ("DeployIntent", "RecoveryIntent"), "creation intent required")
    _ = validate(owner, repository, artifact_id, intent.source_commit, token, oidc_token,
             min(10, remaining_seconds))
    snapshot = load(mirror)
    checked(snapshot.head == expected_head, "stale submission journal head")
    checked(not any(e.kind == intent.kind and e.transaction == intent.transaction
                    for e in snapshot.events), "intent already attempted; reconcile without resubmitting")
    require_creation_receipts(snapshot, owner=owner, repository=repository)
    checked(remote.head(mirror, expected_url) == expected_head, "remote submission head changed")
    evidence = encode(dict(version=1, owner=owner, repository=repository,
                           artifact_id=artifact_id, pages_build_version=intent.source_commit,
                           environment="github-pages"))
    intent_head = append(mirror, expected_head, intent, evidence)
    _ = remote.publish(mirror, expected_url, expected_head, intent_head)
    remaining = deadline - clock()
    if remaining <= 0:
        raise CreationUnknown("publication deadline after intent; reconcile without resubmitting")
    # Never catch and retry: a timeout can mean the server accepted this POST.
    _, raw = send(owner, repository, artifact_id, intent.source_commit, token,
                  oidc_token, timeout=min(10, remaining))
    receipt_head, receipt = record_created(mirror, intent_head, intent, raw,
                                           owner=owner, repository=repository)
    _ = remote.publish(mirror, expected_url, intent_head, receipt_head)
    if clock() >= deadline:
        raise CreationUnknown("publication deadline after receipt; reconcile recorded state")
    return receipt_head, receipt
