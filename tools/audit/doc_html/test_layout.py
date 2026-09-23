import json
import unittest

from tools.audit.doc_html.layout import Browser, Failed, Geometry, Incomplete, Passed, Report, Row, geometry
from tools.serialization.json import JsonValue


class LayoutTests(unittest.TestCase):
    def test_geometry_boundary_and_observation_wire_format(self) -> None:
        raw: dict[str, JsonValue] = dict(difference=0, baseline_source_supported=False,
            annotation_gaps=[1, 2.5], line_gaps=[], multiline_gap=None,
            annotation_text_fragments=[1, 1], display='inline-grid', scripts=0)
        measured = geometry(json.dumps(raw))
        self.assertEqual(measured, Geometry(0, False, (1, 2.5), (), None, (1, 1), 'inline-grid', 0))
        row = Row('ruby', 375, 12, 'normal', measured)
        expected = dict(case='ruby', width=375, size=12, line_height='normal', **raw)
        self.assertEqual(json.dumps(row.representation()), json.dumps(expected))
        changes: tuple[tuple[str, JsonValue], ...] = (
            ('difference', True), ('baseline_source_supported', 1), ('annotation_gaps', ['1']),
            ('multiline_gap', '0'), ('annotation_text_fragments', [1.0]), ('scripts', False))
        for field, value in changes:
            with self.subTest(field=field), self.assertRaises(ValueError):
                _ = geometry(json.dumps(dict(raw, **{field: value})))
        with self.assertRaises(ValueError):
            _ = geometry(json.dumps(dict(raw, difference=float('nan'))))

    def test_failure_retains_partial_observations_and_exact_status_fields(self) -> None:
        row = Row('ruby', 375, 12, 'normal', Geometry(0, False, (0,), (), None, (1,), 'inline-grid', 0))
        browsers = (Browser('chromium', 'fixture-version', (row,)),)
        common: dict[str, JsonValue] = {'corpus_sha256': 'corpus', 'css_sha256': 'css', 'playwright': 'version',
            'browsers': {'chromium': {'version': 'fixture-version', 'measurements': [row.representation()]}}}
        self.assertEqual(Report('corpus', 'css', 'version', browsers, Failed('timeout')).representation(),
                         dict(common, result='failed', error='timeout'))
        self.assertEqual(Report('corpus', 'css', 'version', browsers, Passed()).representation(),
                         dict(common, result='passed'))
        self.assertEqual(Report('corpus', 'css', 'version', browsers, Incomplete()).representation(),
                         dict(common, result='failed'))
