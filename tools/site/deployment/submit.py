"""Persist an authorized attempt before its single API call and bind its receipt.

The publisher owns eligibility, artifact provenance, lock and remote protection.
This function enforces ordering and per-intent replay rejection, not those gates.
"""
import time

from journal import Event, append, load, remote
from journal.model import encode
from payload import checked
from .create import create, validate, CreationUnknown
from .journal import record_created


def execute(mirror, expected_head, intent, *, expected_url, owner, repository,
            artifact_id, token, oidc_token, remaining_seconds):
    """Submit once, then await remotely recorded status within the same budget.

    A succeeded status is not public smoke, LKG promotion or proof of the current
    publication. The caller retains all authorization and reconciliation duties.
    """
    from .journal import wait_remote
    return _execute(mirror, expected_head, intent, expected_url=expected_url,
                    owner=owner, repository=repository, artifact_id=artifact_id,
                    token=token, oidc_token=oidc_token, remaining_seconds=remaining_seconds,
                    clock=time.monotonic, submit_attempt=submit, wait_attempt=wait_remote)


def _execute(mirror, expected_head, intent, *, expected_url, owner, repository,
             artifact_id, token, oidc_token, remaining_seconds, clock,
             submit_attempt, wait_attempt):
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


def submit(mirror, expected_head, intent, *, expected_url, owner, repository,
           artifact_id, token, oidc_token, remaining_seconds):
    return _submit(mirror, expected_head, intent, expected_url=expected_url,
                   owner=owner, repository=repository, artifact_id=artifact_id,
                   token=token, oidc_token=oidc_token, remaining_seconds=remaining_seconds,
                   clock=time.monotonic, send=create)


def _submit(mirror, expected_head, intent, *, expected_url, owner, repository,
            artifact_id, token, oidc_token, remaining_seconds, clock, send):
    checked(type(remaining_seconds) in (int, float) and 0 < remaining_seconds <= 3600,
            "invalid remaining publication budget")
    deadline = clock() + remaining_seconds
    checked(isinstance(intent, Event), "typed intent required")
    intent.validate()
    checked(intent.kind in ("DeployIntent", "RecoveryIntent"), "creation intent required")
    validate(owner, repository, artifact_id, intent.source_commit, token, oidc_token,
             min(10, remaining_seconds))
    snapshot = load(mirror)
    checked(snapshot.head == expected_head, "stale submission journal head")
    checked(not any(e.kind == intent.kind and e.transaction == intent.transaction
                    for e in snapshot.events), "intent already attempted; reconcile without resubmitting")
    checked(remote.head(mirror, expected_url) == expected_head, "remote submission head changed")
    evidence = encode(dict(version=1, owner=owner, repository=repository,
                           artifact_id=artifact_id, pages_build_version=intent.source_commit,
                           environment="github-pages"))
    intent_head = append(mirror, expected_head, intent, evidence)
    remote.publish(mirror, expected_url, expected_head, intent_head)
    remaining = deadline - clock()
    if remaining <= 0:
        raise CreationUnknown("publication deadline after intent; reconcile without resubmitting")
    # Never catch and retry: a timeout can mean the server accepted this POST.
    _, raw = send(owner, repository, artifact_id, intent.source_commit, token,
                  oidc_token, timeout=min(10, remaining))
    receipt_head, receipt = record_created(mirror, intent_head, intent, raw,
                                           owner=owner, repository=repository)
    remote.publish(mirror, expected_url, intent_head, receipt_head)
    if clock() >= deadline:
        raise CreationUnknown("publication deadline after receipt; reconcile recorded state")
    return receipt_head, receipt
