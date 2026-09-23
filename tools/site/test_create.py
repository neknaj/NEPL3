import base64
from collections.abc import Sequence
import subprocess
from typing import NamedTuple
import unittest
from unittest.mock import patch

from deployment.create import direct_create, create, CreationUnknown
from test_transport import server
from test_receipt_journal import RAW
from tools.serialization.json import decode


class Arguments(NamedTuple):
    owner: str = 'neknaj'
    repository: str = 'NEPL3'
    artifact_id: int = 42
    build_version: str = 'a' * 40
    token: str = 'private-token'
    oidc_token: str = 'private.oidc.token'


class CreateTests(unittest.TestCase):
    def args(self) -> Arguments:
        return Arguments()

    def test_actual_http_post_has_exact_artifact_environment_and_build(self) -> None:
        with server(body=RAW) as seen:
            raw = direct_create(*self.args(), timeout=10)
        self.assertEqual(raw, RAW)
        self.assertEqual(len(seen), 1)
        path, headers, body = seen[0]
        self.assertEqual(path, '/repos/neknaj/NEPL3/pages/deployments')
        self.assertEqual(headers['Authorization'], 'Bearer private-token')
        self.assertEqual(decode(body), dict(artifact_id=42, pages_build_version='a'*40,
                                               environment='github-pages', oidc_token='private.oidc.token'))

    def test_validation_does_not_start_process(self) -> None:
        invalid = (Arguments(owner='../evil'), Arguments(artifact_id=True), Arguments(artifact_id=0),
                   Arguments(artifact_id=2**53), Arguments(build_version='bad'),
                   Arguments(token='bad\r\nheader'), Arguments(oidc_token=''), Arguments(oidc_token='x'*16385))
        for index, args in enumerate(invalid):
            with self.subTest(case=index), patch('deployment.create.subprocess.run') as run:
                with self.assertRaises(ValueError): _ = create(*args)
                run.assert_not_called()

    def test_credentials_on_stdin_and_raw_response_revalidated(self) -> None:
        calls: list[tuple[tuple[str, ...], bytes]] = []
        def run(command: Sequence[str], *, input: bytes, capture_output: bool,
                timeout: float, check: bool) -> subprocess.CompletedProcess[bytes]:
            self.assertTrue(capture_output)
            self.assertFalse(check)
            self.assertEqual(timeout, 10)
            calls.append((tuple(command), input))
            return subprocess.CompletedProcess(command, 0, base64.b64encode(RAW), b'')
        with patch('deployment.create.subprocess.run', run):
            receipt, raw = create(*self.args())
            self.assertEqual((receipt.deployment_id, raw), ('123', RAW))
            self.assertEqual(len(calls), 1)
            self.assertNotIn('private', str(calls[0][0]))
            self.assertIn(b'private.oidc.token', calls[0][1])

    def test_uncertain_attempt_is_never_retried_and_secrets_not_reported(self) -> None:
        outcomes: tuple[Exception | subprocess.CompletedProcess[bytes], ...] = (
                    subprocess.TimeoutExpired('private-token', 10), OSError('private-token'),
                    subprocess.CompletedProcess([], 1, b'private-token', b'private.oidc.token'),
                    subprocess.CompletedProcess([], 0, base64.b64encode(b'{"id":"wrong"}'), b''))
        for outcome in outcomes:
            with self.subTest(outcome=type(outcome).__name__), patch('deployment.create.subprocess.run') as run:
                if isinstance(outcome, Exception): run.side_effect=outcome
                else: run.return_value=outcome
                with self.assertRaises(CreationUnknown) as failure: _ = create(*self.args())
                self.assertNotIn('private', str(failure.exception))
                self.assertIsNone(failure.exception.__cause__)
                self.assertEqual(run.call_count, 1)
