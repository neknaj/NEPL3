"""Exercise typed browser observations with the pinned real engines."""
import json
from pathlib import Path
import tempfile
import unittest

from browser import Case, Measurement, measurement, observe
from tools.serialization.json import JsonValue


def site(root: Path, *, content: str = '', stylesheet: bool = True) -> None:
    (root / 'assets').mkdir()
    (root / 'examples').mkdir()
    _ = (root / 'assets/site.css').write_text('body { margin: 8px; }', encoding='utf-8')
    style = '<link rel="stylesheet" href="/NEPL3/assets/site.css">' if stylesheet else ''
    _ = (root / 'index.html').write_text('<!doctype html><meta charset="utf-8"><title>Overview</title>' + style + content,
                                       encoding='utf-8')
    _ = (root / 'examples/index.html').write_text(
        '<!doctype html><meta charset="utf-8"><title>Examples</title>' + style + '<pre><code>hello 世界\n</code></pre>',
        encoding='utf-8')


class BrowserTests(unittest.TestCase):
    def test_measurement_boundary_and_wire_fields(self) -> None:
        state = measurement('{"scripts":0,"styles":1,"overflow":false,"title":"Title"}')
        self.assertEqual(state, Measurement(0, 1, False, 'Title'))
        self.assertEqual(Case('chromium', 'fixture', 375, 'index.html', state).representation(),
                         dict(engine='chromium', version='fixture', width=375, page='index.html',
                              scripts=0, styles=1, overflow=False, title='Title'))
        fields: tuple[tuple[str, JsonValue], ...] = (
            ('scripts', True), ('styles', '1'), ('overflow', 0), ('title', None))
        for key, value in fields:
            raw: dict[str, JsonValue] = dict(scripts=0, styles=1, overflow=False, title='Title')
            raw[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                _ = measurement(json.dumps(raw))
        with self.assertRaises(ValueError):
            _ = measurement(None)

    def test_three_engines_two_widths_preserve_source_text(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            site(root)
            cases = observe(root, '/NEPL3/', ('index.html', 'examples/index.html'), ('hello 世界\n',))
        self.assertEqual(len(cases), 12)
        self.assertEqual({(row.engine, row.width) for row in cases},
                         {(engine, width) for engine in ('chromium', 'firefox', 'webkit')
                          for width in (375, 1280)})
        self.assertTrue(all(row.version and row.state.scripts == 0 and row.state.styles == 1
                            and not row.state.overflow for row in cases))
        self.assertEqual([row.state.title for row in cases], ['Overview', 'Examples'] * 6)

    def test_script_overflow_styles_and_example_mismatch_fail(self) -> None:
        scenarios = (('script', '<script>0</script>', True, 'hello 世界\n'),
                     ('overflow', '<div style="width:2000px">wide</div>', True, 'hello 世界\n'),
                     ('styles', '', False, 'hello 世界\n'),
                     ('example', '', True, 'different input'))
        for name, content, stylesheet, expected in scenarios:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                site(root, content=content, stylesheet=stylesheet)
                with self.assertRaises(AssertionError):
                    _ = observe(root, '/NEPL3/', ('index.html', 'examples/index.html'), (expected,))
