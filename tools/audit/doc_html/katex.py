"""Check real native KaTeX Doc packaging offline in all three browser engines.

Usage: python tools/audit/doc_html/katex.py BINARY NEW_OUTPUT_DIRECTORY PACKAGES
PACKAGES is the locked audit/math/node_modules installation, not document input.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
from typing import Literal

from playwright.sync_api import Route, sync_playwright

SOURCE = '''article en "Native KaTeX" body
cons paragraph cons sentence cons text "Inline " cons math Math frac 1 2 cons text " and " cons math Math add x y nil nil
cons display Math frac 3 4
nil'''
MEASURE = """() => {
  const visuals = [...document.querySelectorAll('.katex')];
  const maths = [...document.querySelectorAll('math')];
  if (visuals.length !== 3 || maths.length !== 3) throw Error('lost math');
  if (document.scripts.length || document.querySelector('[style]')) throw Error('active or inline style');
  if (document.querySelectorAll('.nepl-math-accessible').length !== 3) throw Error('missing accessible MathML');
  for (const math of maths) {
    if (math.closest('[aria-hidden="true"]')) throw Error('accessible math hidden from AT');
    const owner = math.closest('.nepl-math-accessible');
    const style = getComputedStyle(owner);
    if (style.position !== 'absolute' || style.width !== '1px') throw Error('accessible CSS not applied');
  }
  for (const visual of visuals) {
    if (!visual.closest('[aria-hidden="true"]')) throw Error('duplicate accessible visual');
    const box = visual.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) throw Error('invisible visual');
  }
  if (!document.fonts.check('16px KaTeX_Main')) throw Error('KaTeX font unavailable');
  const loaded = [...document.fonts].filter(f => f.family.startsWith('KaTeX_') && f.status === 'loaded');
  if (loaded.length === 0) throw Error('no embedded font loaded');
  if (document.documentElement.scrollWidth > innerWidth + 1) throw Error('viewport overflow');
  return JSON.stringify(visuals.map(v => {
    const b = v.getBoundingClientRect();
    return [b.x,b.y,b.width,b.height,getComputedStyle(v).fontFamily];
  }));
}"""


def block_network(route: Route) -> None:
    route.abort()


def main() -> None:
    if len(sys.argv) != 4:
        raise ValueError(__doc__)
    binary, root, packages = (Path(arg).resolve() for arg in sys.argv[1:])
    root.mkdir(parents=True)
    source = root / 'source.nepld'
    _ = source.write_text(SOURCE, encoding='utf-8')
    env = dict(os.environ, NEPL3_KATEX_NODE_MODULES=str(packages))
    for mode in ('inline', 'external'):
        _ = subprocess.run([str(binary), 'doc-html', 'export', '--css', mode,
                            str(source), str(root / mode)], env=env, check=True)
    results: list[dict[str, str | int]] = []
    themes: tuple[Literal['light', 'dark'], ...] = ('light', 'dark')
    with sync_playwright() as p:
        for implementation in (p.chromium, p.firefox, p.webkit):
            with implementation.launch() as browser:
                for width in (375, 1280):
                    for theme in themes:
                        with browser.new_context(java_script_enabled=False, color_scheme=theme,
                                                 viewport={'width': width, 'height': 900}) as context:
                            _ = context.route(re.compile(r'^https?://'), block_network)
                            values: list[str] = []
                            for mode in ('inline', 'external'):
                                page = context.new_page()
                                _ = page.goto((root / mode / 'document.html').as_uri())
                                deadline = time.monotonic() + 15
                                while time.monotonic() < deadline:
                                    loaded: object = page.evaluate("document.fonts.status === 'loaded'")  # pyright: ignore[reportAny]
                                    if loaded is True:
                                        break
                                    time.sleep(0.05)
                                else:
                                    raise AssertionError('font load deadline')
                                value: object = page.evaluate(MEASURE)  # pyright: ignore[reportAny]
                                if not isinstance(value, str):
                                    raise TypeError('layout must be JSON text')
                                values.append(value)
                                _ = page.screenshot(path=str(root / f'{implementation.name}-{width}-{theme}-{mode}.png'), full_page=True)
                                page.close()
                            if values[0] != values[1]:
                                raise AssertionError('inline/external layout mismatch')
                            results.append({'engine': implementation.name, 'version': browser.version,
                                            'width': width, 'theme': theme, 'result': 'passed'})
    print(json.dumps({'document_javascript': False, 'network': False,
                      'independent_accessible_mathml': True, 'results': results}, indent=2))


if __name__ == '__main__':
    main()
