import json
import unittest

from observation import Content, Context, Deadline, Failed, Missing, ObservationError, Passed, WorkerExit, worker_report
from tools.serialization.json import JsonValue


class SmokeModelTests(unittest.TestCase):
    def test_observations_preserve_existing_wire_fields(self) -> None:
        context = Context('https://example.invalid/', 'a' * 64, 'https')
        report = Passed(context, 'b' * 40, (
            Content('index.html', context.url + 'index.html', 3, 'c' * 64, 'text/html'),
            Missing('missing', context.url + 'missing')))
        # Independently specify the publication artifact, including the 404's
        # absence of byte/digest/MIME fields and the .nojekyll omission.
        expected: dict[str, JsonValue] = dict(
            version=1, result='passed', kind='docs-only-http-byte-check', transport='https',
            url=context.url, source_commit='b' * 40, manifest_sha256='a' * 64,
            observations=[dict(route='index.html', url=context.url + 'index.html', status=200,
                               bytes=3, sha256='c' * 64, mime='text/html'),
                          dict(route='missing', url=context.url + 'missing', status=404)],
            omitted_control_files=['.nojekyll'], publication_verified=False)
        self.assertEqual(report.representation(), expected)
        self.assertEqual(worker_report(json.dumps(expected).encode(), context), report)

    def test_failure_variants_and_worker_context(self) -> None:
        context = Context('http://127.0.0.1:80/', 'a' * 64, 'loopback-http')
        common = dict(version=1, result='failed', **context.representation())
        self.assertEqual(Failed(Deadline(), context).representation(), dict(common, reason='deadline'))
        self.assertEqual(Failed(WorkerExit(2), context).representation(),
                         dict(common, reason='worker-failed', exit_code=2))
        raw = b'{"version":1,"result":"failed","reason":"ValueError","detail":"wrong MIME","publication_verified":false}'
        self.assertEqual(worker_report(raw, context), Failed(ObservationError('ValueError', 'wrong MIME'), context))

    def test_untrusted_worker_fields_are_validated(self) -> None:
        context = Context('https://example.invalid/', 'a' * 64, 'https')
        valid = Passed(context, 'b' * 40, ()).representation()
        invalid: tuple[tuple[str, JsonValue], ...] = (
            ('version', True), ('result', 'unknown'), ('publication_verified', True),
            ('observations', [dict(route='x', url='x', status=201)]),
            ('observations', [dict(route='x', url='x', status=200, bytes=True, sha256='c', mime='text/html')]),
            ('omitted_control_files', []), ('source_commit', 42))
        for key, value in invalid:
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                _ = worker_report(json.dumps(dict(valid, **{key: value})).encode(), context)
        with self.assertRaises(ValueError):
            _ = worker_report(b'{"version":1,"version":2}', context)
