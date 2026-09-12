"""Record a bounded public byte observation against the submitted payload."""
from dataclasses import replace
import time

from journal import append, remote
from journal.model import decode, encode, MAX_EVIDENCE
from payload import snapshot, tar_bytes, digest, checked
import smoke as http_smoke
from .journal import status_history
from .receipt import Phase


def record(mirror, expected_head, root, manifest, url, *, expected_url, owner,
           repository, remaining_seconds):
    return _record(mirror, expected_head, root, manifest, url, expected_url=expected_url,
                   owner=owner, repository=repository, remaining_seconds=remaining_seconds,
                   clock=time.monotonic, observe=http_smoke.run)


def _record(mirror, expected_head, root, manifest, url, *, expected_url, owner,
            repository, remaining_seconds, clock, observe):
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
    build = decode(files['build.json'])
    checked(isinstance(build, dict) and build.get('source_commit') == history.event.source_commit,
            "smoke source differs from submitted source")
    http_smoke.endpoint(url, build.get('base_path'), False)
    started = clock()
    remaining = deadline - started
    checked(remaining > 0, "smoke budget exhausted before HTTP")
    allowance = min(300, remaining)
    observation_deadline = started + allowance
    report = observe(root, manifest, url, timeout=allowance)
    checked(isinstance(report, dict) and report.get('result') in ('passed', 'failed')
            and report.get('manifest_sha256') == manifest and report.get('url') == url
            and report.get('transport') == 'https' and report.get('publication_verified') is False,
            "smoke report identity mismatch")
    if report['result'] == 'passed':
        checked(report.get('source_commit') == history.event.source_commit,
                "smoke report source mismatch")
    # A late HTTP pass is retained as evidence, but is not a passed smoke event.
    late = clock() >= observation_deadline
    kind = 'SmokePassed' if report['result'] == 'passed' and not late else 'SmokeFailed'
    evidence = encode(dict(version=1, report=report, deadline_exceeded=late))
    checked(len(evidence) <= MAX_EVIDENCE, "smoke journal evidence limit")
    head = append(mirror, expected_head, replace(history.event, kind=kind), evidence)
    remote.publish(mirror, expected_url, expected_head, head)
    checked(not late and clock() < deadline, "smoke deadline exceeded; reconcile recorded evidence")
    return head, report
