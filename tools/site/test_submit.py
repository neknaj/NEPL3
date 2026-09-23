import tempfile
import unittest
from pathlib import Path
from dataclasses import replace
from unittest.mock import patch

from deployment.submit import submit, submit_with, execute, execute_with
from deployment.create import CreationUnknown
from deployment.journal import latest_created, status_history
from deployment.receipt import Receipt, created, observed
from deployment.transport import Result
from deployment.poll import Report, Stop
from journal import Event, load, remote
from journal.model import Kind, decode
import test_journal_remote
from test_receipt_journal import RAW


class SubmitTests(unittest.TestCase):
    def test_execute_connects_post_to_remotely_recorded_status(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent = self.setup(directory)
            def status(receipt: Receipt, token: str, *, timeout: float) -> Result:
                self.assertEqual(token, 'private-token')
                self.assertGreater(timeout, 0)
                self.assertLessEqual(timeout, 10)
                self.assertEqual(load(server).events[-1].kind, 'DeployReceipt')
                raw = b'{ "status": "succeed" }'
                return Result(observed(raw, receipt=receipt, request_url=receipt.status_endpoint), raw)
            with patch('deployment.submit.create', return_value=(created(RAW, owner='neknaj', repository='NEPL3'), RAW)) as post, \
                 patch('deployment.transport.status', side_effect=status) as get:
                head, report = execute(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                                       artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
            self.assertEqual((post.call_count, get.call_count), (1, 1))
            self.assertEqual(report.stop, Stop.SUCCEEDED)
            self.assertEqual(load(server).head, head)
            self.assertEqual(status_history(server, owner='neknaj', repository='NEPL3').observations,
                             report.responses)

    def test_execute_passes_remaining_budget_and_never_polls_unknown_creation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent = self.setup(directory)
            expected = ('d'*40, Report('123', Stop.SUCCEEDED, ()))
            for budget, elapsed, allowance in [(60, 17, 43), (60, 60, None), (3600, 0, 600)]:
                waits: list[float] = []
                def post(actual: Path, prior: str | None, event: Event, *, expected_url: str,
                         owner: str, repository: str, artifact_id: int, token: str, oidc_token: str,
                         remaining_seconds: float) -> tuple[str, Receipt]:
                    self.assertEqual((actual, prior, event, expected_url, owner, repository, artifact_id,
                                      token, oidc_token, remaining_seconds),
                                     (mirror, None, intent, str(server), 'neknaj', 'NEPL3', 42,
                                      'private-token', 'private.oidc', budget))
                    return 'c'*40, created(RAW, owner=owner, repository=repository)
                def wait(actual: Path, head: str, token: str, *, expected_url: str, owner: str,
                         repository: str, remaining_seconds: float) -> tuple[str, Report]:
                    self.assertEqual((actual, head, token, expected_url, owner, repository),
                                     (mirror, 'c'*40, 'private-token', str(server), 'neknaj', 'NEPL3'))
                    waits.append(remaining_seconds)
                    return expected
                def call() -> tuple[str, Report]:
                    return execute_with(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                        artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=budget,
                        clock=iter([0, elapsed]).__next__, submit_attempt=post, wait_attempt=wait)
                if allowance is None:
                    with self.assertRaises(CreationUnknown): _ = call()
                    self.assertEqual(waits, [])
                else:
                    self.assertEqual(call(), expected)
                    self.assertEqual(waits, [allowance])
            with patch('deployment.submit.create', side_effect=CreationUnknown('lost')), \
                 patch('deployment.journal.wait_remote') as poll:
                with self.assertRaises(CreationUnknown):
                    _ = execute(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                                artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
            poll.assert_not_called()

    def setup(self, directory: str) -> tuple[Path, Path, Event]:
        server, (mirror, _) = test_journal_remote.RemoteJournalTests().setup_repositories(directory)
        intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
        return server, mirror, intent

    def test_post_observes_remote_intent_then_remote_receipt_is_returned(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent = self.setup(directory)
            def send(owner: str, repository: str, artifact_id: int, build_version: str,
                     token: str, oidc_token: str, *, timeout: float) -> tuple[Receipt, bytes]:
                self.assertEqual((owner, repository, artifact_id, build_version, token, oidc_token),
                                 ('neknaj', 'NEPL3', 42, 'a'*40, 'private-token', 'private.oidc'))
                self.assertGreater(timeout, 0)
                self.assertLessEqual(timeout, 10)
                state = load(server)
                self.assertEqual(state.events, (intent,))
                self.assertEqual(decode(state.evidence[0]), dict(version=1, owner="neknaj", repository="NEPL3",
                    artifact_id=42, pages_build_version="a"*40, environment="github-pages"))
                self.assertNotIn(b'private', state.evidence[0])
                return created(RAW, owner=owner, repository=repository), RAW
            with patch('deployment.submit.create', side_effect=send) as call:
                head, receipt = submit(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                                       artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
            self.assertEqual(call.call_count, 1)
            self.assertEqual(load(server).head, head)
            self.assertEqual(latest_created(server, owner="neknaj", repository="NEPL3")[2:], (receipt, RAW))

    def test_lost_response_leaves_remote_intent_and_rejects_replay(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent = self.setup(directory)
            with patch('deployment.submit.create', side_effect=CreationUnknown('lost')) as call:
                with self.assertRaises(CreationUnknown):
                    _ = submit(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                               artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
                state = load(server)
                self.assertEqual(state.events, (intent,))
                with self.assertRaisesRegex(ValueError, 'already attempted'):
                    _ = submit(mirror, state.head, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                               artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
                kinds: tuple[Kind, ...] = ('DeployIntent', 'RecoveryIntent')
                for kind in kinds:
                    with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, 'unresolved creation intent'):
                        _ = submit(mirror, state.head, replace(intent, transaction='new-name', kind=kind),
                                   expected_url=str(server), owner='neknaj', repository='NEPL3', artifact_id=42,
                                   token='private-token', oidc_token='private.oidc', remaining_seconds=60)
                self.assertEqual(load(server).head, state.head)
            self.assertEqual(call.call_count, 1)

    def test_intent_push_failure_prevents_post(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent = self.setup(directory)
            with patch.object(remote, 'publish', side_effect=ValueError('push failed')), \
                 patch('deployment.submit.create') as call:
                with self.assertRaises(ValueError):
                    _ = submit(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                               artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
            call.assert_not_called()
            self.assertEqual(load(mirror).events, (intent,))
            self.assertIsNone(load(server).head)

    def test_receipt_push_failure_keeps_local_receipt_for_reconciliation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent = self.setup(directory)
            intent = replace(intent, kind="RecoveryIntent")
            original = remote.publish
            pushes: list[tuple[Path, str, str | None, str]] = []
            def publish(repo: Path, expected_url: str, expected_remote_head: str | None, new_head: str) -> remote.Confirmation:
                pushes.append((repo, expected_url, expected_remote_head, new_head))
                if len(pushes) == 2:
                    raise ValueError('receipt acknowledgement lost')
                return original(repo, expected_url, expected_remote_head, new_head)
            with patch.object(remote, 'publish', side_effect=publish), \
                 patch('deployment.submit.create', return_value=(created(RAW, owner='neknaj', repository='NEPL3'), RAW)) as send:
                with self.assertRaisesRegex(ValueError, 'acknowledgement lost'):
                    _ = submit(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                               artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
            self.assertEqual(send.call_count, 1)
            self.assertEqual(load(server).events, (intent,))
            local_head, event, _, raw = latest_created(mirror, owner='neknaj', repository='NEPL3')
            self.assertEqual(event.kind, 'RecoveryReceipt')
            self.assertEqual(raw, RAW)
            with patch('deployment.submit.create') as send, self.assertRaisesRegex(ValueError, 'already attempted'):
                _ = submit(mirror, local_head, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                           artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60)
            send.assert_not_called()

    def test_budget_expired_by_intent_confirmation_prevents_post(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent = self.setup(directory)
            with patch('deployment.submit.create') as send:
                with self.assertRaises(CreationUnknown):
                    _ = submit_with(mirror, None, intent, expected_url=str(server), owner='neknaj', repository='NEPL3',
                                    artifact_id=42, token='private-token', oidc_token='private.oidc', remaining_seconds=60,
                                    clock=iter([0, 61]).__next__, send=send)
            send.assert_not_called()
            self.assertEqual(load(server).events, (intent,))
