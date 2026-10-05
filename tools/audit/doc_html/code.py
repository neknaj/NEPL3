"""Verify shared-frontend Code output and its themes in CI browsers only."""
from pathlib import Path
import json
import re
import subprocess
import sys
from playwright.sync_api import Route, sync_playwright

GUESTS = [
    'article en "日本語 😀 é </code><script> &"\r\nbody\t nil',
    'article en "Outer" body cons paragraph cons code Doc article en "Inner" body nil nil nil',
]
SOURCE = 'article en "Host" body ' + ''.join(
    'cons paragraph cons code Doc ' + guest + ' nil\n' for guest in GUESTS) + 'nil'


def block_network(route: Route) -> None:
    route.abort()


def main() -> None:
    if len(sys.argv) != 3:
        raise ValueError('usage: code.py BINARY NEW_OUTPUT_DIRECTORY')
    binary = Path(sys.argv[1]).resolve()
    root = Path(sys.argv[2]).resolve()
    root.mkdir(parents=True, exist_ok=False)
    source = root / 'code.nepld'
    _ = source.write_bytes(SOURCE.encode())
    for mode in ('external', 'inline'):
        _ = subprocess.run([str(binary), 'doc-html', 'export', '--css', mode,
                            str(source), str(root / mode)], check=True)
    results: list[dict[str, str]] = []
    with sync_playwright() as p:
        for implementation in (p.chromium, p.firefox, p.webkit):
            browser = implementation.launch()
            try:
                for theme in ('light', 'dark'):
                    with browser.new_context(java_script_enabled=False, color_scheme=theme,
                                             viewport={'width': 375, 'height': 900}) as context:
                        _ = context.route(re.compile(r'^https?://'), block_network)
                        for mode in ('external', 'inline'):
                            page = context.new_page()
                            _ = page.goto((root / mode / 'document.html').as_uri())
                            code = page.locator('pre > code')
                            if code.count() != len(GUESTS):
                                raise AssertionError('Missing Code guests')
                            for index, expected in enumerate(GUESTS):
                                if code.nth(index).text_content() != expected:
                                    raise AssertionError(('Source changed', index, code.nth(index).text_content(), expected))
                            if page.locator('script').count():
                                raise AssertionError('Code escaped its text context')
                            value: object = page.evaluate(  # pyright: ignore[reportAny]
                                """() => {
                                const token = document.querySelector('.nepl-code-marker');
                                if (!token || !token.dataset.neplId.startsWith('pc-')) throw Error('Missing category or identity');
                                const pre = token.closest('pre');
                                const color = getComputedStyle(token).color;
                                const background = getComputedStyle(pre).backgroundColor;
                                if (document.documentElement.scrollWidth > innerWidth + 1) throw Error('Overflow');
                                // A stylesheet-only probe complements the real
                                // frontend token above; it does not claim every
                                // role was emitted by this particular guest.
                                const categories = {};
                                const sentinel = document.createElement('span');
                                sentinel.style.color = 'rgb(1, 2, 3)';
                                const probe = document.createElement('span');
                                sentinel.append(probe);
                                document.querySelector('.nepl-doc').append(sentinel);
                                for (const role of ['content','marker','delimiter','name','quantity','annotation']) {
                                    probe.className = 'nepl-code-' + role;
                                    categories[role] = getComputedStyle(probe).color;
                                }
                                sentinel.remove();
                                return JSON.stringify({color,background,categories});
                            }""")
                            expected_style = {'color': 'rgb(163, 32, 53)', 'background': 'rgb(244, 246, 248)'} if theme == 'light' else {'color': 'rgb(255, 158, 171)', 'background': 'rgb(11, 16, 22)'}
                            palette = {
                                'light': ['rgb(31, 41, 55)', 'rgb(163, 32, 53)', 'rgb(70, 85, 105)', 'rgb(7, 87, 168)', 'rgb(34, 107, 53)', 'rgb(113, 67, 155)'],
                                'dark': ['rgb(226, 234, 243)', 'rgb(255, 158, 171)', 'rgb(193, 206, 219)', 'rgb(155, 201, 255)', 'rgb(168, 221, 181)', 'rgb(197, 181, 238)'],
                            }
                            expected_values: dict[str, object] = dict(expected_style)
                            expected_values['categories'] = dict(zip(
                                ('content', 'marker', 'delimiter', 'name', 'quantity', 'annotation'), palette[theme], strict=True))
                            if not isinstance(value, str) or json.loads(value) != expected_values:
                                raise AssertionError(('Wrong theme', theme, value))
                            results.append({'engine': implementation.name, 'theme': theme, 'css': mode, 'result': 'passed'})
                            page.close()
            finally:
                browser.close()
    print(json.dumps({'cases': results}))


if __name__ == '__main__':
    main()
