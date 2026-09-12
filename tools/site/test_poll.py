import unittest

from deployment.poll import _wait, Stop
from deployment.receipt import Receipt, observed
from deployment.transport import Result, TransportError


class PollTests(unittest.TestCase):
    def run_poll(self, states, *, timeout=600, cost=1):
        receipt = Receipt("123", "https://api.github.com/repos/neknaj/NEPL3/pages/deployments/123", "a" * 64)
        now = [0]; calls = []; sleeps = []
        def clock(): return now[0]
        def sleep(delay): sleeps.append(delay); now[0] += delay
        def fetch(actual, token, *, timeout):
            self.assertEqual(actual, receipt)
            self.assertEqual(token, "token")
            calls.append(timeout); now[0] += cost
            state = states[min(len(calls)-1, len(states)-1)]
            if state is None: raise TransportError("unavailable")
            raw = ('{"status":"' + state + '"}').encode()
            return Result(observed(raw, receipt=receipt, request_url=receipt.status_endpoint), raw)
        report = _wait(receipt, "token", timeout=timeout, clock=clock, sleep=sleep, fetch=fetch)
        return report, calls, sleeps

    def test_pending_sequence_retains_original_responses(self):
        report, calls, sleeps = self.run_poll(["deployment_queued", "deployment_in_progress", "succeed"])
        self.assertEqual(report.stop, Stop.SUCCEEDED)
        self.assertEqual([r.observation.status for r in report.responses],
                         ["deployment_queued", "deployment_in_progress", "succeed"])
        self.assertEqual(report.responses[-1].raw_response, b'{"status":"succeed"}')
        self.assertEqual(calls, [10, 10, 10]); self.assertEqual(sleeps, [5, 5])

    def test_unknown_failed_and_transport_stop_without_retry(self):
        for state, stop in [("deployment_lost", Stop.UNKNOWN), ("future_state", Stop.UNKNOWN),
                             ("deployment_failed", Stop.FAILED), (None, Stop.TRANSPORT)]:
            with self.subTest(state=state):
                report, calls, sleeps = self.run_poll([state])
                self.assertEqual(report.stop, stop)
                self.assertEqual(len(calls), 1); self.assertEqual(sleeps, [])

    def test_remaining_budget_is_passed_and_late_success_is_not_success(self):
        report, calls, sleeps = self.run_poll(["deployment_queued", "succeed"], timeout=8, cost=2)
        self.assertEqual(calls, [8, 1])
        self.assertEqual(report.stop, Stop.DEADLINE)
        self.assertEqual(report.responses[-1].observation.status, "succeed")
        self.assertEqual(sleeps, [5])
        report, calls, sleeps = self.run_poll(["deployment_queued"], timeout=3, cost=1)
        self.assertEqual(report.stop, Stop.DEADLINE)
        self.assertEqual(calls, [3]); self.assertEqual(sleeps, [2])

    def test_invalid_budget_is_rejected(self):
        for timeout in [True, 0, -1, 601, float("nan")]:
            with self.subTest(timeout=timeout), self.assertRaises(ValueError):
                self.run_poll(["succeed"], timeout=timeout)

    def test_full_pending_wait_expires_instead_of_becoming_success(self):
        report, calls, sleeps = self.run_poll(["deployment_in_progress"])
        # One second per fetch followed by five seconds per sleep: 100 rounds
        # consume the 600-second contract, independently of implementation output.
        self.assertEqual(report.stop, Stop.DEADLINE)
        self.assertEqual(len(calls), 100)
        self.assertEqual(len(report.responses), 100)
        self.assertEqual(sum(sleeps), 500)
