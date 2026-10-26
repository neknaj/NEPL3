"""Optional, bounded visual evidence outside the immutable site payload."""
from collections.abc import Sequence
import hashlib
import json
from pathlib import Path, PurePosixPath
from typing import final

from tools.serialization.json import JsonValue, integer, string

ENGINES = ('chromium', 'firefox', 'webkit')
WIDTHS = (375, 1280)
THEMES = ('light', 'dark')


def valid_route(route: str) -> bool:
    parts = route.split('/')
    reserved = {'CON', 'PRN', 'AUX', 'NUL', *(f'COM{n}' for n in range(1, 10)), *(f'LPT{n}' for n in range(1, 10))}
    return (route.endswith('.html') and PurePosixPath(route).as_posix() == route
            and not route.startswith('/')
            and all(c.isascii() and (c.isalnum() or c in '/-_.') for c in route)
            and all(part not in ('', '.', '..') and not part.endswith('.')
                    and part.split('.')[0].upper() not in reserved for part in parts))


@final
class Screenshots:
    def __init__(self, site: Path, destination: Path, pages: Sequence[str], selected: Sequence[str]) -> None:
        if not selected or len(selected) > 16 or len(set(selected)) != len(selected):
            raise ValueError('select one to sixteen unique screenshot routes')
        if any(not valid_route(route) or route not in pages for route in selected):
            raise ValueError('screenshot routes must be audited HTML pages')
        if destination.resolve().is_relative_to(site.resolve()):
            raise ValueError('screenshots must be outside the immutable site')
        self.site = site
        self.destination = destination
        self.selected = frozenset(selected)
        self.manifest_sha256 = hashlib.sha256((site / 'manifest.json').read_bytes()).hexdigest()
        self.rows: list[dict[str, JsonValue]] = []
        self.seen: set[tuple[str, int, str, str]] = set()
        destination.mkdir(parents=True, exist_ok=False)

    def path(self, engine: str, width: int, theme: str, route: str) -> Path:
        if engine not in ENGINES or width not in WIDTHS or theme not in THEMES or route not in self.selected:
            raise ValueError('screenshot coordinate is outside the selected matrix')
        path = self.destination / engine / str(width) / theme / (route[:-5] + '.png')
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists():
            raise ValueError('screenshot output already exists')
        return path

    def record(self, engine: str, version: str, width: int, theme: str, route: str,
               path: Path, png: bytes) -> None:
        key = (engine, width, theme, route)
        expected = self.destination / engine / str(width) / theme / (route[:-5] + '.png')
        if (engine not in ENGINES or width not in WIDTHS or theme not in THEMES
                or route not in self.selected or not version or key in self.seen or path != expected):
            raise ValueError('invalid or duplicate screenshot evidence')
        if len(png) < 24 or png[:8] != b'\x89PNG\r\n\x1a\n' or png[12:16] != b'IHDR':
            raise ValueError('missing PNG header')
        actual_width = int.from_bytes(png[16:20], 'big')
        height = int.from_bytes(png[20:24], 'big')
        if actual_width != width or height < 1 or path.read_bytes() != png:
            raise ValueError('PNG bytes or dimensions differ')
        self.seen.add(key)
        self.rows.append(dict(engine=engine, version=version, width=width, theme=theme,
                              page=route, path=path.relative_to(self.destination).as_posix(),
                              bytes=len(png), sha256=hashlib.sha256(png).hexdigest(),
                              png_width=actual_width, png_height=height))

    def finish(self) -> None:
        expected = {(engine, width, theme, page) for engine in ENGINES for width in WIDTHS
                    for theme in THEMES for page in self.selected}
        if self.seen != expected:
            raise ValueError('screenshot matrix is incomplete')
        if hashlib.sha256((self.site / 'manifest.json').read_bytes()).hexdigest() != self.manifest_sha256:
            raise ValueError('site manifest changed during screenshot capture')
        output = self.destination / 'manifest.json'
        if output.exists():
            raise FileExistsError(output)
        paths = tuple(self.destination.rglob('*'))
        if any(path.is_symlink() for path in paths):
            raise ValueError('symlink in screenshot output')
        actual = {path.relative_to(self.destination).as_posix() for path in paths if path.is_file()}
        if actual != {string(row['path']) for row in self.rows}:
            raise ValueError('screenshot output membership changed')
        for row in self.rows:
            data = (self.destination / string(row['path'])).read_bytes()
            if len(data) != integer(row['bytes']) or hashlib.sha256(data).hexdigest() != string(row['sha256']):
                raise ValueError('screenshot bytes changed after capture')
        selected: list[JsonValue] = [page for page in sorted(self.selected)]
        images: list[JsonValue] = [row for row in self.rows]
        result: dict[str, JsonValue] = dict(format='nepl3.site-screenshots/1',
            site_manifest_sha256=self.manifest_sha256, pages=selected,
            scripts_enabled=False, font_profile='existing site audit fallback', images=images)
        with output.open('x', encoding='utf-8') as stream:
            _ = stream.write(json.dumps(result, indent=2) + '\n')
