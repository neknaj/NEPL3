import tempfile
import unittest
from dataclasses import replace
from unittest.mock import patch

from deployment.submit import submit, _submit
from deployment.create import CreationUnknown
from deployment.journal import latest_created
from journal import Event, load, remote
from journal.model import decode
import test_journal_remote
from test_receipt_journal import RAW


class SubmitTests(unittest.TestCase):
    def setup(self, directory):
        server, (mirror, _) = test_journal_remote.RemoteJournalTests().setup_repositories(directory)
        intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
        kwargs = dict(expected_url=str(server), owner="neknaj", repository="NEPL3",
                      artifact_id=42, token="private-token", oidc_token="private.oidc", remaining_seconds=60)
        return server, mirror, intent, kwargs

    def test_post_observes_remote_intent_then_remote_receipt_is_returned(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent, kwargs = self.setup(directory)
            def send(*args, **options):
                state = load(server)
                self.assertEqual(state.events, (intent,))
                self.assertEqual(decode(state.evidence[0]), dict(version=1, owner="neknaj", repository="NEPL3",
                    artifact_id=42, pages_build_version="a"*40, environment="github-pages"))
                self.assertNotIn(b'private', state.evidence[0])
                return None, RAW
            with patch('deployment.submit.create', side_effect=send) as call:
                head, receipt = submit(mirror, None, intent, **kwargs)
            self.assertEqual(call.call_count, 1)
            self.assertEqual(load(server).head, head)
            self.assertEqual(latest_created(server, owner="neknaj", repository="NEPL3")[2:], (receipt, RAW))

    def test_lost_response_leaves_remote_intent_and_rejects_replay(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent, kwargs = self.setup(directory)
            with patch('deployment.submit.create', side_effect=CreationUnknown('lost')) as call:
                with self.assertRaises(CreationUnknown): submit(mirror, None, intent, **kwargs)
                state = load(server)
                self.assertEqual(state.events, (intent,))
                with self.assertRaisesRegex(ValueError, 'already attempted'):
                    submit(mirror, state.head, intent, **kwargs)
            self.assertEqual(call.call_count, 1)

    def test_intent_push_failure_prevents_post(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent, kwargs = self.setup(directory)
            with patch.object(remote, 'publish', side_effect=ValueError('push failed')), \
                 patch('deployment.submit.create') as call:
                with self.assertRaises(ValueError): submit(mirror, None, intent, **kwargs)
            call.assert_not_called()
            self.assertEqual(load(mirror).events, (intent,))
            self.assertIsNone(load(server).head)

    def test_receipt_push_failure_keeps_local_receipt_for_reconciliation(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent, kwargs = self.setup(directory)
            intent = replace(intent, kind="RecoveryIntent")
            original = remote.publish
            pushes = []
            def publish(*args):
                pushes.append(args)
                if len(pushes) == 2:
                    raise ValueError('receipt acknowledgement lost')
                return original(*args)
            with patch.object(remote, 'publish', side_effect=publish), \
                 patch('deployment.submit.create', return_value=(None, RAW)) as send:
                with self.assertRaisesRegex(ValueError, 'acknowledgement lost'):
                    submit(mirror, None, intent, **kwargs)
            self.assertEqual(send.call_count, 1)
            self.assertEqual(load(server).events, (intent,))
            local_head, event, _, raw = latest_created(mirror, owner='neknaj', repository='NEPL3')
            self.assertEqual(event.kind, 'RecoveryReceipt')
            self.assertEqual(raw, RAW)
            with patch('deployment.submit.create') as send, self.assertRaisesRegex(ValueError, 'already attempted'):
                submit(mirror, local_head, intent, **kwargs)
            send.assert_not_called()

    def test_budget_expired_by_intent_confirmation_prevents_post(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, intent, kwargs = self.setup(directory)
            with patch('deployment.submit.create') as send:
                with self.assertRaises(CreationUnknown):
                    _submit(mirror, None, intent, **kwargs, clock=iter([0, 61]).__next__, send=send)
            send.assert_not_called()
            self.assertEqual(load(server).events, (intent,))
