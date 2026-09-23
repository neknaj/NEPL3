"""Record a bounded public byte observation against the submitted payload."""
from dataclasses import replace
from collections.abc import Callable
from pathlib import Path
from typing import Protocol
import time

from journal import append, remote
from journal.model import decode, encode, MAX_EVIDENCE
from payload import snapshot, tar_bytes, digest, checked
import smoke as http_smoke
from .journal import status_history
from .receipt import Phase
from observation import Passed, Report
from tools.serialization.json import object_value


class Observe(Protocol):
    def __call__(self, root: Path, manifest: str, url: str, /, *, timeout: float) -> Report: ...


def record(mirror: Path, expected_head: str, root: Path, manifest: str, url: str, *,
           expected_url: str, owner: str, repository: str,
           remaining_seconds: float) -> tuple[str, Report]:
    return record_with(mirror, expected_head, root, manifest, url, expected_url=expected_url,
                   owner=owner, repository=repository, remaining_seconds=remaining_seconds,
                   clock=time.monotonic, observe=http_smoke.run)


def record_with(mirror: Path, expected_head: str, root: Path, manifest: str, url: str, *,
                expected_url: str, owner: str, repository: str, remaining_seconds: float,
                clock: Callable[[], float], observe: Observe) -> tuple[str, Report]:
    checked(type(remaining_seconds) in (int, float) and 0 < remaining_seconds <= 3600,
            "invalid remaining publication budget")
    deadline = clock() + remaining_seconds
    history = status_history(mirror, owner=owner, repository=repository)
    checked(history.head == expected_head and history.observations
            and history.observations[-1].observation.phase == Phase.SUCCEEDED,
            "smoke requires the expected successful API observation")
    checked(remote.head(mirror, expected_url) == expected_head, "remote smoke head changed")
    files = snapshot(root, manifest)
    checked(digest(tar_bytes(files)) == history.event.payload_sha256,
            "smoke payload differs from submitted tar")
    build = object_value(decode(files['build.json']))
    checked(build.get('source_commit') == history.event.source_commit,
            "smoke source differs from submitted source")
    _ = http_smoke.endpoint(url, build.get('base_path'), False)
    started = clock()
    remaining = deadline - started
    checked(remaining > 0, "smoke budget exhausted before HTTP")
    allowance = min(300, remaining)
    observation_deadline = started + allowance
    report = observe(root, manifest, url, timeout=allowance)
    context = report.context
    checked(context is not None and context.manifest_sha256 == manifest and context.url == url
            and context.transport == 'https', "smoke report identity mismatch")
    if isinstance(report, Passed):
        checked(report.source_commit == history.event.source_commit,
                "smoke report source mismatch")
    # A late HTTP pass is retained as evidence, but is not a passed smoke event.
    late = clock() >= observation_deadline
    kind = 'SmokePassed' if isinstance(report, Passed) and not late else 'SmokeFailed'
    evidence = encode(dict(version=1, report=report.representation(), deadline_exceeded=late))
    checked(len(evidence) <= MAX_EVIDENCE, "smoke journal evidence limit")
    head = append(mirror, expected_head, replace(history.event, kind=kind), evidence)
    _ = remote.publish(mirror, expected_url, expected_head, head)
    checked(not late and clock() < deadline, "smoke deadline exceeded; reconcile recorded evidence")
    return head, report
