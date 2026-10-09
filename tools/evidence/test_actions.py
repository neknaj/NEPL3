"""Selection checks bind metadata and downloaded bytes, not acceptance results."""
import copy
from dataclasses import replace
from datetime import UTC, datetime
import json
import unittest
from unittest.mock import patch

from tools.evidence.actions import MAX_METADATA_BYTES, Conclusion, Selection, select_download
from tools.evidence.runner import digest
from tools.serialization.json import JsonValue


def encoded(value: dict[str, JsonValue]) -> bytes:
    return json.dumps(value).encode()


def fixture() -> tuple[Selection, dict[str, JsonValue], dict[str, JsonValue], bytes, datetime]:
    chosen = Selection('neknaj/NEPL3', 1, 2, 3, 4, 'a' * 40, 'b' * 64)
    # Format parsing is deliberately a separate boundary from transport checks.
    payload = b'transport bytes'
    artifact: dict[str, JsonValue] = {
        'id': 4, 'name': f'a01-command-{chosen.source_revision}-2-3',
        'url': 'https://api.github.com/repos/neknaj/NEPL3/actions/artifacts/4',
        'archive_download_url': 'https://api.github.com/repos/neknaj/NEPL3/actions/artifacts/4/zip',
        'expired': False, 'size_in_bytes': len(payload), 'digest': 'sha256:' + digest(payload),
        'created_at': '2026-10-09T00:00:00Z', 'updated_at': '2026-10-09T00:01:00Z',
        'expires_at': '2026-10-10T00:00:00Z',
        'workflow_run': {'id': 2, 'repository_id': 1, 'head_repository_id': 1, 'head_sha': chosen.source_revision},
    }
    run: dict[str, JsonValue] = {
        'id': 2, 'run_attempt': 3, 'event': 'workflow_dispatch', 'status': 'completed',
        'conclusion': 'success', 'path': '.github/workflows/ci.yml@main', 'head_sha': chosen.source_revision,
        'repository': {'id': 1, 'full_name': 'neknaj/NEPL3'},
        'head_repository': {'id': 1, 'full_name': 'neknaj/NEPL3'},
    }
    return chosen, artifact, run, payload, datetime(2026, 10, 9, 4, tzinfo=UTC)


def change(root: dict[str, JsonValue], path: tuple[str, ...], value: JsonValue) -> None:
    current = root
    for key in path[:-1]:
        nested = current[key]
        if not isinstance(nested, dict):
            raise ValueError('test path')
        current = nested
    current[path[-1]] = value


class ActionsSelectionTests(unittest.TestCase):
    def test_bare_and_ref_qualified_workflow_paths_keep_outcome_separate(self) -> None:
        chosen, artifact, run, payload, now = fixture()
        for path in ['.github/workflows/ci.yml', '.github/workflows/ci.yml@main']:
            for outcome in ['success', 'failure', 'cancelled']:
                run['path'] = path; run['conclusion'] = outcome
                result = select_download(encoded(artifact), encoded(run), payload, chosen, now)
                self.assertEqual(result.conclusion, Conclusion(outcome))
                self.assertEqual(result.zip_sha256, digest(payload))
                self.assertEqual(result.selection.inner_sha256, 'b' * 64)
                self.assertTrue(result.download_api_url.endswith('/artifacts/4/zip'))

    def test_artifact_identity_expiry_endpoint_and_bytes_fail_closed(self) -> None:
        chosen, original, run, payload, now = fixture()
        cases: list[tuple[tuple[str, ...], JsonValue]] = [
            (('id',), 5), (('id',), True), (('name',), f'a01-command-{chosen.source_revision}-2-2'),
            (('url',), 'https://example.invalid/4'), (('archive_download_url',), 'http://api.github.com/zip'),
            (('expired',), True), (('expired',), None), (('size_in_bytes',), len(payload) + 1),
            (('digest',), None), (('digest',), 'sha256:' + '0' * 64),
            (('expires_at',), '2026-10-09T04:00:00Z'), (('expires_at',), 'invalid'),
            (('workflow_run', 'id'), 9), (('workflow_run', 'repository_id'), 9),
            (('workflow_run', 'head_repository_id'), 9), (('workflow_run', 'head_sha'), 'c' * 40),
        ]
        for path, value in cases:
            artifact = copy.deepcopy(original); change(artifact, path, value)
            with self.subTest(path=path, value=value), self.assertRaises(ValueError):
                _ = select_download(encoded(artifact), encoded(run), payload, chosen, now)
        with self.assertRaises(ValueError):
            _ = select_download(encoded(original), encoded(run), payload + b'x', chosen, now)

    def test_run_source_attempt_repository_and_completion_fail_closed(self) -> None:
        chosen, artifact, original, payload, now = fixture()
        cases: list[tuple[tuple[str, ...], JsonValue]] = [
            (('id',), 9), (('run_attempt',), 2), (('head_sha',), 'c' * 40),
            (('event',), 'pull_request'), (('status',), 'in_progress'), (('conclusion',), None),
            (('path',), '.github/workflows/other.yml@main'), (('path',), '.github/workflows/ci.yml@'),
            (('repository', 'id'), 9), (('repository', 'full_name'), 'other/NEPL3'),
            (('head_repository',), None), (('head_repository', 'id'), 9),
        ]
        for path, value in cases:
            run = copy.deepcopy(original); change(run, path, value)
            with self.subTest(path=path, value=value), self.assertRaises(ValueError):
                _ = select_download(encoded(artifact), encoded(run), payload, chosen, now)
        with self.assertRaises(ValueError):
            _ = select_download(encoded(artifact), encoded(original), payload, chosen, now.replace(tzinfo=None))

    def test_metadata_json_and_byte_boundaries_fail_closed(self) -> None:
        chosen, artifact, run, payload, now = fixture()
        invalid = [b'', b'null', b'[]', b'{"id":4,"id":4}', b'{"x":NaN}',
                   b'{"x":1e999}', b'[' * 2000 + b']' * 2000,
                   b' ' * (MAX_METADATA_BYTES + 1), b'\xff']
        for raw in invalid:
            for side in ['artifact', 'run']:
                with self.subTest(side=side, size=len(raw)), self.assertRaises(ValueError):
                    _ = select_download(raw if side == 'artifact' else encoded(artifact),
                                        raw if side == 'run' else encoded(run), payload, chosen, now)
        # Exact cap is permitted; JSON whitespace does not change metadata.
        raw = encoded(artifact)
        _ = select_download(raw + b' ' * (MAX_METADATA_BYTES - len(raw)), encoded(run), payload, chosen, now)

    def test_independent_selection_validation(self) -> None:
        chosen, artifact, run, payload, now = fixture()
        for invalid in [replace(chosen, repository='../NEPL3'), replace(chosen, repository='a/b/c'),
                        replace(chosen, repository_id=True), replace(chosen, run_id=0),
                        replace(chosen, run_attempt=-1), replace(chosen, artifact_id=2**53),
                        replace(chosen, source_revision='A' * 40), replace(chosen, inner_sha256='b' * 63)]:
            with self.subTest(selection=invalid), self.assertRaises(ValueError):
                _ = select_download(encoded(artifact), encoded(run), payload, invalid, now)

    def test_timestamp_order_and_utc_syntax(self) -> None:
        chosen, original, run, payload, now = fixture()
        for key, value in [('created_at', '2026-10-09T00:02:00Z'),
                           ('updated_at', '2026-10-09T04:00:01Z'),
                           ('created_at', '2026-02-30T00:00:00Z'),
                           ('expires_at', '2026-10-10T09:00:00+09:00')]:
            artifact = copy.deepcopy(original); artifact[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                _ = select_download(encoded(artifact), encoded(run), payload, chosen, now)
        original['updated_at'] = '2026-10-09T04:00:00.000000+00:00'
        _ = select_download(encoded(original), encoded(run), payload, chosen, now)

    def test_download_cap_and_same_length_tampering(self) -> None:
        chosen, artifact, run, payload, now = fixture()
        with patch('tools.evidence.actions.MAX_DOWNLOAD_BYTES', len(payload)):
            _ = select_download(encoded(artifact), encoded(run), payload, chosen, now)
            with self.assertRaisesRegex(ValueError, 'download byte limit'):
                _ = select_download(encoded(artifact), encoded(run), payload + b'x', chosen, now)
        with self.assertRaisesRegex(ValueError, 'download digest mismatch'):
            _ = select_download(encoded(artifact), encoded(run), b'x' * len(payload), chosen, now)
        with self.assertRaisesRegex(ValueError, 'download byte limit'):
            _ = select_download(encoded(artifact), encoded(run), b'', chosen, now)

    def test_malformed_metadata_field_types(self) -> None:
        chosen, original, original_run, payload, now = fixture()
        artifact_cases: list[tuple[str, JsonValue]] = [('workflow_run', []), ('created_at', None), ('digest', {}),
                           ('size_in_bytes', True), ('id', '4'), ('name', [])]
        for key, value in artifact_cases:
            artifact = copy.deepcopy(original); artifact[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = select_download(encoded(artifact), encoded(original_run), payload, chosen, now)
        run_cases: list[tuple[str, JsonValue]] = [('repository', []), ('path', None), ('conclusion', 'unknown'),
                           ('run_attempt', True), ('head_repository', 'neknaj/NEPL3')]
        for key, value in run_cases:
            run = copy.deepcopy(original_run); run[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = select_download(encoded(original), encoded(run), payload, chosen, now)

    def test_strict_json_policy_with_otherwise_valid_metadata(self) -> None:
        chosen, artifact, run, payload, now = fixture()
        for side in ['artifact', 'run']:
            valid = encoded(artifact if side == 'artifact' else run)
            suffixes = [b', "id": ' + (b'4' if side == 'artifact' else b'2'),
                        b', "unused": NaN', b', "unused": 1e999',
                        b', "unused": ' + b'[' * 2000 + b']' * 2000]
            for suffix in suffixes:
                raw = valid[:-1] + suffix + b'}'
                with self.subTest(side=side, suffix=suffix[:30]), self.assertRaises(ValueError):
                    _ = select_download(raw if side == 'artifact' else encoded(artifact),
                                        raw if side == 'run' else encoded(run), payload, chosen, now)
