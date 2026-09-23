from pathlib import Path
import unittest

from deployment.create import CreationUnknown
from deployment.poll import Report, Stop
from deployment.receipt import Receipt
from deployment.submit import execute_with
from journal import Event


class SubmitCallbackTests(unittest.TestCase):
    def test_typed_result_and_remaining_deadline_are_forwarded(self) -> None:
        mirror = Path('test-mirror')
        intent = Event('DeployIntent', 'tx1', 1, 1, 'a' * 40, 'b' * 64)
        receipt = Receipt('123', 'https://api.github.com/repos/neknaj/NEPL3/pages/deployments/123', 'c' * 64)
        report = Report('123', Stop.SUCCEEDED, ())
        calls: list[str] = []

        def submit(*args: object, **kwargs: object) -> tuple[str, Receipt]:
            self.assertEqual(args, (mirror, None, intent))
            self.assertEqual(kwargs['remaining_seconds'], 10)
            calls.append('submit')
            return 'd' * 40, receipt

        def wait(*args: object, **kwargs: object) -> tuple[str, Report]:
            self.assertEqual(args, (mirror, 'd' * 40, 'token'))
            self.assertEqual(kwargs['remaining_seconds'], 7)
            calls.append('wait')
            return 'e' * 40, report

        times = iter((20.0, 23.0))
        result = execute_with(mirror, None, intent, expected_url='test-origin', owner='neknaj',
                              repository='NEPL3', artifact_id=42, token='token', oidc_token='oidc',
                              remaining_seconds=10, clock=times.__next__, submit_attempt=submit, wait_attempt=wait)
        self.assertEqual(result, ('e' * 40, report))
        self.assertEqual(calls, ['submit', 'wait'])

    def test_unknown_submission_does_not_invoke_wait(self) -> None:
        def submit(*_args: object, **_kwargs: object) -> tuple[str, Receipt]:
            raise CreationUnknown('test uncertain result')

        def wait(*_args: object, **_kwargs: object) -> tuple[str, Report]:
            self.fail('wait must not follow an uncertain submission')

        intent = Event('DeployIntent', 'tx1', 1, 1, 'a' * 40, 'b' * 64)
        with self.assertRaises(CreationUnknown):
            _ = execute_with(Path('test-mirror'), None, intent, expected_url='test-origin', owner='neknaj',
                             repository='NEPL3', artifact_id=42, token='token', oidc_token='oidc',
                             remaining_seconds=10, clock=lambda: 0.0, submit_attempt=submit, wait_attempt=wait)


if __name__ == '__main__':
    _ = unittest.main()
