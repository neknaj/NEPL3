"""Record/reload a creation receipt against its preceding intent.

This records an API observation; it does not authorize creating a deployment.
"""
import base64
from dataclasses import replace

from journal import Event, append, load
from journal.model import MAX_EVIDENCE, decode, encode
from payload import checked
from .receipt import created


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
    checked(len(snapshot.events) >= 2, "creation receipt missing")
    intent, event = snapshot.events[-2:]
    expected = {"DeployIntent": "DeployReceipt", "RecoveryIntent": "RecoveryReceipt"}.get(intent.kind)
    checked(expected is not None and event == replace(intent, kind=expected),
            "creation receipt intent identity mismatch")
    value = decode(snapshot.evidence[-1])
    checked(set(value) == {"version", "owner", "repository", "response"}
            and type(value["version"]) is int and value["version"] == 1, "receipt evidence shape")
    checked(value["owner"] == owner and value["repository"] == repository,
            "receipt evidence repository mismatch")
    checked(isinstance(value["response"], str), "receipt response must be base64")
    raw = base64.b64decode(value["response"], validate=True)
    receipt = created(raw, owner=owner, repository=repository)
    return snapshot.head, event, receipt, raw
