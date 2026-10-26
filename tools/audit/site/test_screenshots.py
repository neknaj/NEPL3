"""Pure screenshot-evidence boundary tests; no browser or server is launched."""
import hashlib
from pathlib import Path
import struct
import tempfile
import unittest

from screenshots import ENGINES, THEMES, WIDTHS, Screenshots, valid_route
from tools.serialization.json import array, decode, object_value, string


def png(width: int) -> bytes:
    # Header-only synthetic data tests recorder boundaries, not PNG decoding.
    return b'\x89PNG\r\n\x1a\n' + b'\x00\x00\x00\x0dIHDR' + struct.pack('>II', width, 900)


class ScreenshotTests(unittest.TestCase):
    def test_path_selection_and_output_boundary(self) -> None:
        self.assertTrue(valid_route('docs/project/contributing.html'))
        for route in ('../a.html', '/a.html', 'a//b.html', './a.html', 'a\\b.html',
                      'a.html?x', 'a%2fb.html', 'CON.html', 'a.txt', 'a/../b.html'):
            self.assertFalse(valid_route(route), route)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); site = root / 'site'; site.mkdir()
            _ = (site / 'manifest.json').write_bytes(b'{}')
            for selected in ((), ('missing.html',), ('a.html', 'a.html'), ('../a.html',)):
                with self.assertRaises(ValueError):
                    _ = Screenshots(site, root / 'bad', ('a.html',), selected)
                self.assertFalse((root / 'bad').exists())
            with self.assertRaises(ValueError):
                _ = Screenshots(site, site / 'shots', ('a.html',), ('a.html',))
            self.assertFalse((site / 'shots').exists())
            output = root / 'existing'; output.mkdir()
            with self.assertRaises(FileExistsError):
                _ = Screenshots(site, output, ('a.html',), ('a.html',))

    def test_complete_matrix_binds_payloads_and_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); site = root / 'site'; site.mkdir()
            manifest = site / 'manifest.json'; _ = manifest.write_bytes(b'{"site":1}')
            shots = Screenshots(site, root / 'shots', ('docs/a.html',), ('docs/a.html',))
            with self.assertRaises(ValueError): shots.finish()
            self.assertFalse((shots.destination / 'manifest.json').exists())
            for engine in ENGINES:
                for width in WIDTHS:
                    for theme in THEMES:
                        target = shots.path(engine, width, theme, 'docs/a.html')
                        data = png(width); _ = target.write_bytes(data)
                        shots.record(engine, 'fixture-version', width, theme, 'docs/a.html', target, data)
                        with self.assertRaises(ValueError):
                            shots.record(engine, 'fixture-version', width, theme, 'docs/a.html', target, data)
                        with self.assertRaises(ValueError): _ = shots.path(engine, width, theme, 'docs/a.html')
            original = manifest.read_bytes(); _ = manifest.write_bytes(b'changed')
            with self.assertRaises(ValueError): shots.finish()
            _ = manifest.write_bytes(original)
            extra = shots.destination / 'extra.png'; _ = extra.write_bytes(b'extra')
            with self.assertRaises(ValueError): shots.finish()
            extra.unlink()
            first = next(shots.destination.rglob('*.png')); saved = first.read_bytes()
            _ = first.write_bytes(b'changed')
            with self.assertRaises(ValueError): shots.finish()
            _ = first.write_bytes(saved); shots.finish()
            result = object_value(decode((shots.destination / 'manifest.json').read_text()))
            self.assertEqual(result['site_manifest_sha256'], hashlib.sha256(original).hexdigest())
            self.assertEqual(len(array(result['images'])), 12)
            for value in array(result['images']):
                row = object_value(value)
                data = (shots.destination / string(row['path'])).read_bytes()
                self.assertEqual(row['sha256'], hashlib.sha256(data).hexdigest())
                self.assertEqual(row['bytes'], len(data))
            with self.assertRaises(FileExistsError): shots.finish()

    def test_incorrect_coordinate_png_or_written_bytes_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); site = root / 'site'; site.mkdir()
            _ = (site / 'manifest.json').write_bytes(b'{}')
            shots = Screenshots(site, root / 'shots', ('a.html',), ('a.html',))
            for engine, width, theme, route in [('other', 375, 'light', 'a.html'),
                    ('chromium', 320, 'light', 'a.html'), ('chromium', 375, 'other', 'a.html'),
                    ('chromium', 375, 'light', 'missing.html')]:
                with self.assertRaises(ValueError): _ = shots.path(engine, width, theme, route)
            target = shots.path('chromium', 375, 'light', 'a.html')
            for data in (b'not png', png(1280)):
                _ = target.write_bytes(data)
                with self.assertRaises(ValueError):
                    shots.record('chromium', 'fixture', 375, 'light', 'a.html', target, data)
            _ = target.write_bytes(png(375) + b'different')
            with self.assertRaises(ValueError):
                shots.record('chromium', 'fixture', 375, 'light', 'a.html', target, png(375))
            self.assertFalse(shots.seen)
