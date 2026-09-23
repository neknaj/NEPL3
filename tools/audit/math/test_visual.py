from dataclasses import replace
import json
import unittest

from tools.audit.math.visual import Node, Report, assets, compare, corpus, engine, nodes
from tools.serialization.json import JsonValue


class VisualTests(unittest.TestCase):
    def test_corpus_requires_constructor_and_svg_coverage(self) -> None:
        row = 'MATH_VISUAL_CASE ' + json.dumps(dict(original='<svg/>', html='<svg/>', css=''))
        self.assertEqual(len(corpus('\n'.join([row] * 30))), 30)
        for text in ('\n'.join([row] * 29), '\n'.join([row.replace('<svg/>', '<span/>')] * 30)):
            with self.assertRaisesRegex(ValueError, 'corpus'):
                _ = corpus(text)
        invalid = 'MATH_VISUAL_CASE ' + json.dumps(dict(original='<svg/>', html=1, css=''))
        with self.assertRaises(ValueError):
            _ = corpus('\n'.join([invalid] * 30))

    def test_dom_measurement_types_and_rectangle_arity(self) -> None:
        valid: dict[str, JsonValue] = dict(tag='SPAN', text='x', rect=[0, -1, 20.5, 10], style=['10px', 'serif'])
        self.assertEqual(nodes([valid]), (Node('SPAN', 'x', (0, -1, 20.5, 10), ('10px', 'serif')),))
        malformed: tuple[tuple[str, JsonValue], ...] = (
            ('rect', [0, 1, 2]), ('rect', [0, 1, 2, True]), ('style', [1]), ('text', 42))
        for field, value in malformed:
            with self.subTest(field=field), self.assertRaises(ValueError):
                _ = nodes([dict(valid, **{field: value})])

    def test_comparison_preserves_tolerance_and_exact_content_style(self) -> None:
        value = Node('SPAN', 'x', (0, 0, 20, 10), ('serif',))
        compare((value,), (replace(value, rect=(0.049, 0, 20, 10)),), 'chromium', 0)
        for changed in (replace(value, rect=(0.05, 0, 20, 10)), replace(value, tag='DIV'),
                        replace(value, text='y'), replace(value, style=('sans-serif',))):
            with self.subTest(node=changed), self.assertRaises(AssertionError):
                compare((value,), (changed,), 'chromium', 0)
        with self.assertRaises(AssertionError):
            compare((value,), (), 'chromium', 0)

    def test_asset_boundary_and_report_wire_format(self) -> None:
        self.assertEqual(assets(b'{"files":[{"source":"x","bytes":3,"sha256":"digest"}]}')[0].size, 3)
        with self.assertRaises(ValueError):
            _ = assets(b'{"files":[{"source":"x","bytes":true,"sha256":"digest"}]}')
        self.assertEqual(engine('firefox'), 'firefox')
        with self.assertRaises(ValueError):
            _ = engine('unknown')
        self.assertEqual(Report(('chromium', 'webkit'), 30, 120).representation(),
            dict(engines=['chromium', 'webkit'], cases=30, comparisons=120,
                 scope='fixed CSS, visual tree and computed style lowering; not Doc/Math accessibility acceptance'))
