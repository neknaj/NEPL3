import tempfile
import unittest
from unittest.mock import patch

from deployment.journal import record_created, status_history, wait_remote
from deployment.receipt import observed
from deployment.transport import Result
from deployment.poll import Stop
from journal import Event, append, load, remote
import test_journal_remote
from test_receipt_journal import RAW


class RemoteWaitTests(unittest.TestCase):
    def prepare(self, directory):
        server, (mirror, _) = test_journal_remote.RemoteJournalTests().setup_repositories(directory)
        intent = Event("DeployIntent", "tx1", 23, 1, "a" * 40, "b" * 64)
        first = append(mirror, None, intent, b'{}')
        remote.publish(mirror, str(server), None, first)
        head, receipt = record_created(mirror, first, intent, RAW, owner="neknaj", repository="NEPL3")
        remote.publish(mirror, str(server), first, head)
        return server, mirror, head, receipt

    def run_wait(self, server, mirror, head):
        return wait_remote(mirror, head, "test-token", expected_url=str(server),
                           owner="neknaj", repository="NEPL3", remaining_seconds=60)

    def test_next_request_and_success_require_remote_observation(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, head, receipt = self.prepare(directory)
            calls = []
            def fetch(*args, **kwargs):
                history = status_history(server, owner="neknaj", repository="NEPL3")
                self.assertEqual(len(history.observations), len(calls))
                raw = b'{"status":"deployment_in_progress"}' if not calls else b'{"status":"succeed"}'
                calls.append(raw)
                return Result(observed(raw, receipt=receipt, request_url=receipt.status_endpoint), raw)
            with patch('deployment.transport.status', side_effect=fetch), patch('time.sleep'):
                final, report = self.run_wait(server, mirror, head)
            self.assertEqual(report.stop, Stop.SUCCEEDED)
            self.assertEqual(load(server).head, final)
            self.assertEqual([r.raw_response for r in status_history(server, owner="neknaj", repository="NEPL3").observations], calls)

    def test_remote_failure_keeps_local_evidence_and_stops_requests(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, head, receipt = self.prepare(directory)
            raw = b'{"status":"deployment_in_progress"}'
            result = Result(observed(raw, receipt=receipt, request_url=receipt.status_endpoint), raw)
            with patch('deployment.transport.status', return_value=result) as fetch, \
                 patch.object(remote, 'publish', side_effect=ValueError('acknowledgement lost')):
                with self.assertRaisesRegex(ValueError, 'acknowledgement lost'):
                    self.run_wait(server, mirror, head)
            self.assertEqual(fetch.call_count, 1)
            self.assertEqual(load(server).head, head)
            history = status_history(mirror, owner="neknaj", repository="NEPL3")
            self.assertEqual(history.observations[0].raw_response, raw)
            # Retry cannot silently skip the locally recorded, unconfirmed event.
            with patch('deployment.transport.status') as fetch, self.assertRaises(ValueError):
                self.run_wait(server, mirror, history.head)
            fetch.assert_not_called()

    def test_initial_remote_confirmation_consumes_same_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            server, mirror, head, _ = self.prepare(directory)
            now = [0.0]
            def delayed_confirmation(*args):
                now[0] = 61.0
                return head
            with patch('time.monotonic', side_effect=lambda: now[0]), \
                 patch.object(remote, 'head', side_effect=delayed_confirmation), \
                 patch('deployment.transport.status') as fetch:
                final, report = self.run_wait(server, mirror, head)
            self.assertEqual((final, report.stop), (head, Stop.DEADLINE))
            fetch.assert_not_called()
