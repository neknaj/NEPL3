"""Select authenticated Actions metadata against independent caller expectations.

The caller fetches these bytes from the corresponding GitHub REST endpoints and
establishes workflow/source eligibility. This module performs no network access,
executes no payload, and makes no acceptance decision. Artifact names bind our
uploader's attempt label; GitHub's artifact workflow_run has no attempt field.
"""
from collections.abc import Mapping
from dataclasses import dataclass
from datetime import UTC, datetime
from enum import Enum
import re

from tools.evidence.archive import MAX_ARCHIVE_BYTES, checked
from tools.evidence.runner import digest
from tools.serialization.json import JsonValue, decode, integer, object_value, string

MAX_METADATA_BYTES = 65536
MAX_DOWNLOAD_BYTES = 2 * MAX_ARCHIVE_BYTES + 65536
WORKFLOW = '.github/workflows/ci.yml'


class Conclusion(str, Enum):
    SUCCESS = 'success'
    FAILURE = 'failure'
    CANCELLED = 'cancelled'
    TIMED_OUT = 'timed_out'
    ACTION_REQUIRED = 'action_required'
    NEUTRAL = 'neutral'
    SKIPPED = 'skipped'
    STALE = 'stale'


@dataclass(frozen=True, slots=True)
class Selection:
    repository: str
    repository_id: int
    run_id: int
    run_attempt: int
    artifact_id: int
    source_revision: str
    inner_sha256: str

    def validate(self) -> None:
        pieces = self.repository.split('/')
        checked(len(pieces) == 2 and all(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,99}', p) for p in pieces),
                'invalid selected repository')
        for value in (self.repository_id, self.run_id, self.run_attempt, self.artifact_id):
            checked(type(value) is int and 0 < value < 2**53, 'invalid selected Actions ID')
        checked(re.fullmatch(r'[0-9a-f]{40}', self.source_revision), 'invalid selected source')
        checked(re.fullmatch(r'[0-9a-f]{64}', self.inner_sha256), 'invalid selected inner archive digest')


@dataclass(frozen=True, slots=True)
class SelectedDownload:
    selection: Selection
    zip_sha256: str
    expires_at: datetime
    conclusion: Conclusion
    # This API location is not the short-lived signed redirect URL.
    download_api_url: str


def timestamp(raw: str) -> datetime:
    checked(len(raw) <= 40 and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,6})?(?:Z|\+00:00)', raw),
            'invalid Actions UTC timestamp')
    return datetime.fromisoformat(raw).astimezone(UTC)


def metadata(raw: bytes) -> Mapping[str, JsonValue]:
    checked(0 < len(raw) <= MAX_METADATA_BYTES, 'Actions metadata byte limit')
    try:
        return object_value(decode(raw, reject_duplicates=True, reject_nonfinite=True))
    except RecursionError as error:
        raise ValueError('Actions metadata nesting limit') from error


def select_download(artifact_bytes: bytes, run_bytes: bytes, zip_bytes: bytes,
                    selected: Selection, now: datetime) -> SelectedDownload:
    """Check transport identity and ZIP bytes, not the execution claims inside."""
    selected.validate()
    checked(now.tzinfo is not None and now.utcoffset() == UTC.utcoffset(now), 'caller UTC time required')
    checked(0 < len(zip_bytes) <= MAX_DOWNLOAD_BYTES, 'Actions download byte limit')
    artifact = metadata(artifact_bytes)
    run = metadata(run_bytes)
    api = f'https://api.github.com/repos/{selected.repository}'
    endpoint = f'{api}/actions/artifacts/{selected.artifact_id}'
    checked(integer(artifact.get('id')) == selected.artifact_id, 'artifact ID mismatch')
    checked(artifact.get('url') == endpoint and artifact.get('archive_download_url') == endpoint + '/zip',
            'artifact endpoint mismatch')
    checked(artifact.get('name') == f'a01-command-{selected.source_revision}-{selected.run_id}-{selected.run_attempt}',
            'artifact source/run/attempt label mismatch')
    checked(artifact.get('expired') is False, 'artifact expired or expiry state absent')
    created = timestamp(string(artifact.get('created_at')))
    updated = timestamp(string(artifact.get('updated_at')))
    expires = timestamp(string(artifact.get('expires_at')))
    checked(created <= updated <= now < expires, 'artifact timestamps or expiry mismatch')
    checked(integer(artifact.get('size_in_bytes')) == len(zip_bytes), 'download size mismatch')
    actual_digest = digest(zip_bytes)
    checked(artifact.get('digest') == 'sha256:' + actual_digest, 'download digest mismatch')
    origin = object_value(artifact.get('workflow_run'))
    for key, expected in [('id', selected.run_id), ('repository_id', selected.repository_id),
                          ('head_repository_id', selected.repository_id)]:
        checked(integer(origin.get(key)) == expected, 'artifact workflow identity mismatch')
    checked(origin.get('head_sha') == selected.source_revision, 'artifact source mismatch')
    checked(integer(run.get('id')) == selected.run_id and integer(run.get('run_attempt')) == selected.run_attempt,
            'selected run or attempt mismatch')
    checked(run.get('head_sha') == selected.source_revision, 'run source mismatch')
    checked(run.get('event') == 'workflow_dispatch' and run.get('status') == 'completed',
            'run is not a completed explicit dispatch')
    conclusion = Conclusion(string(run.get('conclusion')))
    path = string(run.get('path'))
    # REST may append @ref. It is informational here and is never a filesystem
    # path or an authorization to use that ref; source_revision is pinned above.
    file, separator, ref = path.partition('@')
    checked(file == WORKFLOW and (not separator or bool(ref)) and len(path) <= 512 and
            not any(c.isspace() or ord(c) < 32 for c in path), 'workflow path mismatch')
    for key in ('repository', 'head_repository'):
        repository = object_value(run.get(key))
        checked(integer(repository.get('id')) == selected.repository_id and
                repository.get('full_name') == selected.repository, 'run repository mismatch')
    return SelectedDownload(selected, actual_digest, expires, conclusion, endpoint + '/zip')
