"""The annotation audit consumes a single, independently generated HTML record."""
import json
import unittest

from tools.serialization.json import JsonValue
from tools.audit.math.annotations import Case, Report, Rect, check_geometry, fragment_from, observe


class AnnotationTests(unittest.TestCase):
    def test_corpus_marker_with_libtest_prefix_and_unicode(self) -> None:
        fragment = '<math><mtext>字</mtext></math>'
        record = 'MATH_RECURSIVE_HTML ' + fragment.encode('utf-8').hex()
        self.assertEqual(fragment_from('test fixture ... ' + record + '\nOK\n'), fragment)
        for invalid in ('no record', record + '\n' + record, 'MATH_RECURSIVE_HTML zz',
                        'MATH_RECURSIVE_HTML ff'):
            with self.subTest(corpus=invalid), self.assertRaises(ValueError):
                _ = fragment_from(invalid)

    def test_report_preserves_external_field_names_and_order(self) -> None:
        report = Report((Case('chromium', 'fixture-version', 375, 16),
                         Case('webkit', 'second-version', 1280, 32)))
        self.assertEqual(report.representation(), dict(passed=2, cases=[
            dict(engine='chromium', version='fixture-version', width=375, font_size=16),
            dict(engine='webkit', version='second-version', width=1280, font_size=32)]))


class GeometryTests(unittest.TestCase):
    def test_one_atomic_observation(self) -> None:
        class Page:
            calls: list[str]
            def __init__(self) -> None:
                self.calls = []
            def evaluate(self, expression: str) -> object:
                self.calls.append(expression)
                return json.dumps(dict(
                    base=dict(rect=dict(x=0, y=10, width=10, height=10)),
                    reading=dict(rect=dict(x=0, y=0, width=10, height=10)),
                    fonts="loaded", baselineSourceSupported=False))
        page = Page()
        base, reading, _ = observe(page)
        check_geometry(base, reading)
        self.assertEqual(len(page.calls), 1)

    def test_existing_firefox_failure_still_fails(self) -> None:
        base = Rect(176.21665954589844, 85.1500015258789, 22.566665649414062, 27.849998474121094)
        reading = Rect(173.96665954589844, 73.86666870117188, 27.050003051757812, 12.283332824707031)
        with self.assertRaises(AssertionError):
            check_geometry(base, reading)

    def test_exact_separation_and_center_boundaries(self) -> None:
        base = Rect(0, 10, 10, 10)
        check_geometry(base, Rect(0, 0.5, 10, 10))
        with self.assertRaises(AssertionError):
            check_geometry(base, Rect(0, 0.5001, 10, 10))
        check_geometry(base, Rect(0.999, 0, 10, 10))
        with self.assertRaises(AssertionError):
            check_geometry(base, Rect(1, 0, 10, 10))

    def test_malformed_or_empty_geometry_is_rejected(self) -> None:
        invalids: tuple[JsonValue, ...] = (None, [], {}, dict(x=True, y=0, width=1, height=1),
                        dict(x=float("nan"), y=0, width=1, height=1),
                        dict(x=10**400, y=0, width=1, height=1),
                        dict(x=0, y=0, width=float("inf"), height=1))
        for invalid in invalids:
            with self.subTest(value=invalid), self.assertRaises(ValueError):
                _ = Rect.read(invalid)
        for invalid in (Rect(0, 0, 0, 1), Rect(0, 0, 1, -1), Rect(float("nan"), 0, 1, 1)):
            with self.subTest(box=invalid), self.assertRaises(ValueError):
                check_geometry(Rect(0, 10, 10, 10), invalid)
