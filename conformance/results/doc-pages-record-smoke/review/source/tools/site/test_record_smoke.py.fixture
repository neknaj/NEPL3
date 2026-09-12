import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from deployment.smoke import record, _record
from deployment.journal import record_created, record_status
from journal import Event, append, load, remote
from journal.model import encode, decode
from payload import digest, tar_bytes
import test_journal_remote
from test_receipt_journal import RAW


class RecordSmokeTests(unittest.TestCase):
    def prepare(self, directory):
        server, (mirror, _) = test_journal_remote.RemoteJournalTests().setup_repositories(directory)
        root = Path(directory) / 'site'; root.mkdir()
        files = {'index.html': b'<h1>Doc</h1>', 'build.json': encode(dict(source_commit='a'*40,
                    base_path='/NEPL3/', capability='docs-only')), '.nojekyll': b''}
        files['manifest.json'] = encode(dict(version=1, files=[dict(path=n, bytes=len(b), sha256=digest(b))
                                                           for n, b in files.items()]))
        for n, b in files.items(): (root/n).write_bytes(b)
        manifest = digest(files['manifest.json'])
        intent = Event('DeployIntent', 'tx1', 23, 1, 'a'*40, digest(tar_bytes(files)))
        head = append(mirror, None, intent, b'{}'); remote.publish(mirror, str(server), None, head)
        new, receipt = record_created(mirror, head, intent, RAW, owner='neknaj', repository='NEPL3')
        remote.publish(mirror, str(server), head, new); head = new
        new, _ = record_status(mirror, head, b'{"status":"succeed"}', owner='neknaj', repository='NEPL3', request_url=receipt.status_endpoint)
        remote.publish(mirror, str(server), head, new)
        kwargs = dict(expected_url=str(server), owner='neknaj', repository='NEPL3', remaining_seconds=60)
        args = (mirror, new, root, manifest, 'https://neknaj.github.io/NEPL3/')
        report = dict(result='passed', source_commit='a'*40, manifest_sha256=manifest,
                      url=args[-1], transport='https', publication_verified=False)
        return server, args, kwargs, report

    def test_pass_and_failure_are_recorded_remotely(self):
        for result in ('passed', 'failed'):
            with self.subTest(result=result), tempfile.TemporaryDirectory() as directory:
                server, args, kwargs, report = self.prepare(directory)
                report['result'] = result
                with patch('deployment.smoke.http_smoke.run', return_value=report):
                    head, actual = record(*args, **kwargs)
                state = load(server)
                self.assertEqual(state.head, head)
                self.assertEqual(state.events[-1].kind, 'SmokePassed' if result == 'passed' else 'SmokeFailed')
                self.assertEqual(decode(state.evidence[-1])['report'], actual)

    def test_altered_site_does_not_start_http(self):
        with tempfile.TemporaryDirectory() as directory:
            server, args, kwargs, _ = self.prepare(directory)
            (args[2]/'index.html').write_bytes(b'other document')
            with patch('deployment.smoke.http_smoke.run') as run, self.assertRaises(ValueError):
                record(*args, **kwargs)
            run.assert_not_called()
            self.assertEqual(load(server).head, args[1])

    def test_report_identity_mismatch_is_not_recorded(self):
        with tempfile.TemporaryDirectory() as directory:
            server, args, kwargs, report = self.prepare(directory)
            for key, value in [('source_commit', 'c'*40), ('manifest_sha256', 'c'*64),
                               ('url', 'https://other.invalid/'), ('transport', 'loopback-http'),
                               ('publication_verified', True)]:
                with self.subTest(key=key), patch('deployment.smoke.http_smoke.run', return_value=dict(report, **{key:value})), self.assertRaises(ValueError):
                    record(*args, **kwargs)
                self.assertEqual(load(server).head, args[1])

    def test_late_http_pass_is_retained_as_failed_event(self):
        with tempfile.TemporaryDirectory() as directory:
            server, args, kwargs, report = self.prepare(directory)
            now = [0]
            def observe(*a, **k):
                now[0] = 61
                return report
            with self.assertRaisesRegex(ValueError, 'deadline'):
                _record(*args, **kwargs, clock=lambda: now[0], observe=observe)
            state = load(server)
            self.assertEqual(state.events[-1].kind, 'SmokeFailed')
            evidence = decode(state.evidence[-1])
            self.assertTrue(evidence['deadline_exceeded'])
            self.assertEqual(evidence['report']['result'], 'passed')

    def test_smoke_cap_is_independent_of_longer_transaction_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            server, args, kwargs, report = self.prepare(directory)
            kwargs['remaining_seconds'] = 3600
            now = [0]
            def observe(*a, **k):
                self.assertEqual(k['timeout'], 300)
                now[0] = 301
                return report
            with self.assertRaisesRegex(ValueError, 'deadline'):
                _record(*args, **kwargs, clock=lambda: now[0], observe=observe)
            state = load(server)
            self.assertEqual(state.events[-1].kind, 'SmokeFailed')
            self.assertTrue(decode(state.evidence[-1])['deadline_exceeded'])
