"""Bind exact successful main-push CI metadata to a previously selected pin.

PR branch heads are not execution merge commits. This initial backend deliberately
requires main push CI; it never infers their equivalence or selects a latest run.
"""
from datetime import datetime, timezone
import re
from tools.evidence.archive import checked
from tools.evidence.retrieval.pin import Pin
from tools.serialization.json import decode, object_value, integer, string


def timestamp(value: str) -> int:
    checked(re.fullmatch(r'[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z', value),
            'unsupported GitHub timestamp')
    parsed = datetime.strptime(value, '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=timezone.utc)
    return int(parsed.timestamp())


def verify(pin: Pin, artifact_bytes: bytes, run_bytes: bytes, now: int) -> None:
    pin.validate()
    checked(type(now) is int and 0 < now < pin.expires_at_unix, 'artifact pin expired')
    checked(0 < len(artifact_bytes) <= 65536 and 0 < len(run_bytes) <= 65536, 'artifact metadata bound')
    artifact = object_value(decode(artifact_bytes, reject_duplicates=True, reject_nonfinite=True))
    run = object_value(decode(run_bytes, reject_duplicates=True, reject_nonfinite=True))
    checked(integer(artifact.get('id')) == pin.artifact_id and
            artifact.get('name') == pin.artifact_name, 'artifact identity mismatch')
    checked(artifact.get('url') == pin.artifact_url and
            artifact.get('archive_download_url') == pin.artifact_url + '/zip', 'artifact URL mismatch')
    checked(artifact.get('expired') is False and
            timestamp(string(artifact.get('expires_at'))) == pin.expires_at_unix, 'artifact advertised expiry mismatch')
    checked(integer(artifact.get('size_in_bytes')) == pin.archive_bytes and
            artifact.get('digest') == 'sha256:' + pin.archive_sha256, 'artifact size or digest pin mismatch')
    origin = object_value(artifact.get('workflow_run'))
    for key, expected in [('id', pin.run_id), ('repository_id', pin.repository_id), ('head_repository_id', pin.repository_id)]:
        checked(integer(origin.get(key)) == expected, 'artifact workflow identity mismatch')
    checked(origin.get('head_sha') == pin.source_commit and origin.get('head_branch') == 'main', 'artifact source mismatch')
    checked(integer(run.get('id')) == pin.run_id and integer(run.get('run_attempt')) == pin.run_attempt,
            'workflow attempt mismatch')
    checked(run.get('status') == 'completed' and run.get('conclusion') == 'success', 'workflow is not successful')
    checked(run.get('event') == 'push' and run.get('head_branch') == 'main' and
            run.get('head_sha') == pin.source_commit and run.get('path') == '.github/workflows/ci.yml',
            'unsupported workflow source')
    for key in ('repository', 'head_repository'):
        repository = object_value(run.get(key))
        checked(integer(repository.get('id')) == pin.repository_id and
                repository.get('full_name') == 'neknaj/NEPL3', 'workflow repository mismatch')
    created = timestamp(string(artifact.get('created_at')))
    started = timestamp(string(run.get('run_started_at')))
    completed = timestamp(string(run.get('updated_at')))
    checked(started < created < completed <= now and created < pin.expires_at_unix,
            'artifact outside selected run attempt interval')
