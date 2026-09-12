import base64
import json
import subprocess
import unittest
from unittest.mock import patch

from deployment.create import _create, create, CreationUnknown
from test_transport import server
from test_receipt_journal import RAW


class CreateTests(unittest.TestCase):
    def args(self):
        return ('neknaj', 'NEPL3', 42, 'a' * 40, 'private-token', 'private.oidc.token')

    def test_actual_http_post_has_exact_artifact_environment_and_build(self):
        with server(body=RAW) as seen:
            raw = _create(*self.args(), timeout=10)
        self.assertEqual(raw, RAW)
        self.assertEqual(len(seen), 1)
        path, headers, body = seen[0]
        self.assertEqual(path, '/repos/neknaj/NEPL3/pages/deployments')
        self.assertEqual(headers['Authorization'], 'Bearer private-token')
        self.assertEqual(json.loads(body), dict(artifact_id=42, pages_build_version='a'*40,
                                               environment='github-pages', oidc_token='private.oidc.token'))

    def test_validation_does_not_start_process(self):
        for index, values in [(0, ['../evil']), (2, [True, 0, 2**53]),
                              (3, ['bad']), (4, ['bad\r\nheader']), (5, ['', 'x'*16385])]:
            for value in values:
                args=list(self.args());args[index]=value
                with self.subTest(index=index, value=str(value)[:30]), patch('deployment.create.subprocess.run') as run:
                    with self.assertRaises(ValueError): create(*args)
                    run.assert_not_called()

    def test_credentials_on_stdin_and_raw_response_revalidated(self):
        with patch('deployment.create.subprocess.run') as run:
            run.return_value = subprocess.CompletedProcess([], 0, base64.b64encode(RAW), b'')
            receipt, raw = create(*self.args())
            self.assertEqual((receipt.deployment_id, raw), ('123', RAW))
            self.assertNotIn('private', str(run.call_args.args))
            self.assertIn(b'private.oidc.token', run.call_args.kwargs['input'])
            self.assertEqual(run.call_count, 1)

    def test_uncertain_attempt_is_never_retried_and_secrets_not_reported(self):
        outcomes = [subprocess.TimeoutExpired('private-token', 10), OSError('private-token'),
                    subprocess.CompletedProcess([], 1, b'private-token', b'private.oidc.token'),
                    subprocess.CompletedProcess([], 0, base64.b64encode(b'{"id":"wrong"}'), b'')]
        for outcome in outcomes:
            with self.subTest(outcome=type(outcome).__name__), patch('deployment.create.subprocess.run') as run:
                if isinstance(outcome, Exception): run.side_effect=outcome
                else: run.return_value=outcome
                with self.assertRaises(CreationUnknown) as failure: create(*self.args())
                self.assertNotIn('private', str(failure.exception))
                self.assertIsNone(failure.exception.__cause__)
                self.assertEqual(run.call_count, 1)
