import tempfile
from dataclasses import replace
import unittest
from pathlib import Path
from unittest.mock import patch
from typing import NamedTuple

from deployment.smoke import record, record_with
from observation import Context, Deadline, Failed, Passed, Report
from deployment.journal import record_created, record_status
from journal import Event, append, load, remote
from journal.model import encode, decode
from payload import digest, tar_bytes
import test_journal_remote
from test_receipt_journal import RAW
from tools.serialization.json import object_value


class Arguments(NamedTuple):
    mirror: Path
    head: str
    root: Path
    manifest: str
    url: str


class RecordSmokeTests(unittest.TestCase):
    def prepare(self, directory: str) -> tuple[Path, Arguments, Passed]:
        # macOS temporary roots can have symlink ancestors (/var -> /private/var).
        # Resolve the host fixture location, not the production payload input.
        base = Path(directory).resolve(strict=True)
        server, (mirror, _) = test_journal_remote.RemoteJournalTests().setup_repositories(str(base))
        root = base / 'site'; root.mkdir()
        files = {'index.html': b'<h1>Doc</h1>', 'build.json': encode(dict(source_commit='a'*40,
                    base_path='/NEPL3/', capability='docs-only')), '.nojekyll': b''}
        files['manifest.json'] = encode(dict(version=1, files=[dict(path=n, bytes=len(b), sha256=digest(b))
                                                           for n, b in files.items()]))
        for n, b in files.items(): _ = (root/n).write_bytes(b)
        manifest = digest(files['manifest.json'])
        intent = Event('DeployIntent', 'tx1', 23, 1, 'a'*40, digest(tar_bytes(files)))
        head = append(mirror, None, intent, b'{}'); _ = remote.publish(mirror, str(server), None, head)
        new, receipt = record_created(mirror, head, intent, RAW, owner='neknaj', repository='NEPL3')
        _ = remote.publish(mirror, str(server), head, new); head = new
        new, _ = record_status(mirror, head, b'{"status":"succeed"}', owner='neknaj', repository='NEPL3', request_url=receipt.status_endpoint)
        _ = remote.publish(mirror, str(server), head, new)
        args = Arguments(mirror, new, root, manifest, 'https://neknaj.github.io/NEPL3/')
        report = Passed(Context(args.url, manifest, 'https'), 'a'*40, ())
        return server, args, report

    def test_pass_and_failure_are_recorded_remotely(self) -> None:
        for result in ('passed', 'failed'):
            with self.subTest(result=result), tempfile.TemporaryDirectory() as directory:
                server, args, passed = self.prepare(directory)
                report: Report = Failed(Deadline(), passed.context) if result == 'failed' else passed
                with patch('deployment.smoke.http_smoke.run', return_value=report):
                    head, actual = record(*args, expected_url=str(server), owner='neknaj', repository='NEPL3', remaining_seconds=60)
                state = load(server)
                self.assertEqual(state.head, head)
                self.assertEqual(state.events[-1].kind, 'SmokePassed' if result == 'passed' else 'SmokeFailed')
                self.assertEqual(object_value(decode(state.evidence[-1]))['report'], actual.representation())

    def test_altered_site_does_not_start_http(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, args, _ = self.prepare(directory)
            _ = (args.root/'index.html').write_bytes(b'other document')
            with patch('deployment.smoke.http_smoke.run') as run, self.assertRaises(ValueError):
                _ = record(*args, expected_url=str(server), owner='neknaj', repository='NEPL3', remaining_seconds=60)
            run.assert_not_called()
            self.assertEqual(load(server).head, args.head)

    def test_report_identity_mismatch_is_not_recorded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, args, report = self.prepare(directory)
            for altered in [replace(report, source_commit='c'*40),
                            replace(report, context=replace(report.context, manifest_sha256='c'*64)),
                            replace(report, context=replace(report.context, url='https://other.invalid/')),
                            replace(report, context=replace(report.context, transport='loopback-http')),
                            Failed(Deadline())]:
                with self.subTest(report=altered), patch('deployment.smoke.http_smoke.run', return_value=altered), self.assertRaises(ValueError):
                    _ = record(*args, expected_url=str(server), owner='neknaj', repository='NEPL3', remaining_seconds=60)
                self.assertEqual(load(server).head, args.head)

    def test_late_http_pass_is_retained_as_failed_event(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, args, report = self.prepare(directory)
            now = [0]
            def observe(root: Path, manifest: str, url: str, *, timeout: float) -> Report:
                self.assertEqual((root, manifest, url), (args.root, args.manifest, args.url))
                self.assertEqual(timeout, 60)
                now[0] = 61
                return report
            with self.assertRaisesRegex(ValueError, 'deadline'):
                _ = record_with(*args, expected_url=str(server), owner='neknaj', repository='NEPL3',
                                remaining_seconds=60, clock=lambda: now[0], observe=observe)
            state = load(server)
            self.assertEqual(state.events[-1].kind, 'SmokeFailed')
            evidence = object_value(decode(state.evidence[-1]))
            self.assertTrue(evidence['deadline_exceeded'])
            self.assertEqual(object_value(evidence['report'])['result'], 'passed')

    def test_smoke_cap_is_independent_of_longer_transaction_budget(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, args, report = self.prepare(directory)
            now = [0]
            def observe(root: Path, manifest: str, url: str, *, timeout: float) -> Report:
                self.assertEqual((root, manifest, url), (args.root, args.manifest, args.url))
                self.assertEqual(timeout, 300)
                now[0] = 301
                return report
            with self.assertRaisesRegex(ValueError, 'deadline'):
                _ = record_with(*args, expected_url=str(server), owner='neknaj', repository='NEPL3',
                                remaining_seconds=3600, clock=lambda: now[0], observe=observe)
            state = load(server)
            self.assertEqual(state.events[-1].kind, 'SmokeFailed')
            self.assertTrue(object_value(decode(state.evidence[-1]))['deadline_exceeded'])
