"""The annotation audit consumes a single, independently generated HTML record."""
import unittest

from tools.audit.math.annotations import Case, Report, fragment_from


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
