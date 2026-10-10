"""Synthetic metadata validation only; no CI execution or acceptance claims."""
from copy import deepcopy
import json
import unittest
from tools.evidence.retrieval.metadata import timestamp, verify
from tools.evidence.retrieval.pin import Pin
from tools.serialization.json import JsonValue, decode


def fixture() -> tuple[Pin, dict[str, JsonValue], dict[str, JsonValue]]:
    expiry = timestamp('2026-10-24T12:00:00Z')
    pin = Pin(1, 2, 1, 'a' * 40, 3, 'synthetic-test', 'b' * 64, 100, 'run.stdout', expiry)
    artifact: dict[str, JsonValue] = {
        'id': 3, 'name': pin.artifact_name, 'url': pin.artifact_url,
        'archive_download_url': pin.artifact_url + '/zip', 'expired': False,
        'expires_at': '2026-10-24T12:00:00Z', 'created_at': '2026-10-10T12:01:00Z',
        'size_in_bytes': 100, 'digest': 'sha256:' + pin.archive_sha256,
        'workflow_run': {'id': 2, 'repository_id': 1, 'head_repository_id': 1,
                         'head_sha': pin.source_commit, 'head_branch': 'main'}}
    run: dict[str, JsonValue] = {
        'id': 2, 'run_attempt': 1, 'status': 'completed', 'conclusion': 'success',
        'event': 'push', 'head_branch': 'main', 'head_sha': pin.source_commit,
        'path': '.github/workflows/ci.yml', 'repository': {'id': 1, 'full_name': 'neknaj/NEPL3'},
        'head_repository': {'id': 1, 'full_name': 'neknaj/NEPL3'},
        'run_started_at': '2026-10-10T12:00:00Z', 'updated_at': '2026-10-10T12:02:00Z'}
    return pin, artifact, run


def raw(value: dict[str, JsonValue]) -> bytes:
    return json.dumps(value).encode()


class MetadataTests(unittest.TestCase):
    def test_selected_completed_attempt(self) -> None:
        pin, artifact, run = fixture()
        verify(pin, raw(artifact), raw(run), timestamp('2026-10-10T12:03:00Z'))

    def test_expired_or_changed_artifact(self) -> None:
        pin, artifact, run = fixture()
        changes: list[tuple[str, JsonValue]] = [
            ('id', 4), ('name', 'latest'), ('url', 'https://example.invalid/3'),
            ('archive_download_url', 'https://example.invalid/3.zip'), ('expired', True),
            ('expires_at', '2026-10-25T12:00:00Z'), ('size_in_bytes', 101),
            ('digest', 'sha256:' + 'c' * 64), ('created_at', '2026-10-10T11:59:59Z'),
            ('created_at', '2026-10-10T12:02:00Z'), ('created_at', '2026-10-10T12:02:01Z')]
        for key, value in changes:
            changed = deepcopy(artifact)
            changed[key] = value
            with self.assertRaises(ValueError, msg=key):
                verify(pin, raw(changed), raw(run), timestamp('2026-10-10T12:03:00Z'))
        for now in (pin.expires_at_unix, pin.expires_at_unix + 1, 0):
            with self.assertRaises(ValueError):
                verify(pin, raw(artifact), raw(run), now)

    def test_wrong_workflow_attempt_and_pr_source(self) -> None:
        pin, artifact, run = fixture()
        changes: list[tuple[str, JsonValue]] = [
            ('id', 4), ('run_attempt', 2), ('event', 'pull_request'), ('head_branch', 'feature'),
            ('head_sha', 'c' * 40), ('path', '.github/workflows/other.yml'),
            ('status', 'in_progress'), ('conclusion', 'failure'),
            ('repository', {'id': 9, 'full_name': 'other/repo'}),
            ('head_repository', {'id': 9, 'full_name': 'other/repo'}),
            ('run_started_at', '2026-10-10T12:01:00Z'), ('run_started_at', '2026-10-10T12:01:01Z'), ('updated_at', '2026-10-10T12:04:00Z')]
        for key, value in changes:
            changed = deepcopy(run)
            changed[key] = value
            with self.assertRaises(ValueError, msg=key):
                verify(pin, raw(artifact), raw(changed), timestamp('2026-10-10T12:03:00Z'))

    def test_closed_pin_and_bounded_metadata(self) -> None:
        pin, artifact, run = fixture()
        good = {'schema': 'nepl3.github-artifact/1', 'repository': 'neknaj/NEPL3',
                'repository_id': 1, 'run_id': 2, 'run_attempt': 1, 'source_commit': 'a' * 40,
                'artifact_id': 3, 'artifact_name': 'synthetic-test', 'archive_sha256': 'b' * 64,
                'archive_bytes': 100, 'member': 'run.stdout', 'expires_at_unix': pin.expires_at_unix}
        self.assertEqual(Pin.read(decode(json.dumps(good))), pin)
        for key in good:
            bad = dict(good)
            del bad[key]
            with self.assertRaises(ValueError):
                _ = Pin.read(decode(json.dumps(bad)))
        for value in (b'{}' + b' ' * 65536, b'{"id":3,"id":3}', b'{"id":NaN}'):
            with self.assertRaises(ValueError):
                verify(pin, value, raw(run), timestamp('2026-10-10T12:03:00Z'))
        with self.assertRaises(ValueError):
            verify(pin, raw(artifact), b'[]', timestamp('2026-10-10T12:03:00Z'))


if __name__ == '__main__':
    _ = unittest.main()
