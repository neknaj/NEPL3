"""Check actual multi-page Code output, links and responsive themes in CI."""
from pathlib import Path
import json
import re
import subprocess
import sys
from playwright.sync_api import Route, expect, sync_playwright

GUESTS = [
    'article en "日本語 😀 é </code><script> &"\r\nbody\t nil',
    'article en sentence nil body cons paragraph cons sentence cons ruby text "" text "r" nil nil nil',
]
SOURCE = ('article en "Code page" body '
          + ''.join('cons paragraph cons code Doc ' + guest + ' nil\n' for guest in GUESTS)
          + 'cons paragraph cons sentence cons math Math frac 1 2 nil nil cons display Math frac 3 4 '
          + 'cons paragraph cons sentence cons link page "target" some "target" text "Target" nil nil nil')
TARGET = 'article en "Target" body cons section target "Target section" body cons paragraph cons sentence cons link page "code" none text "Back" nil nil nil nil'


def block_network(route: Route) -> None:
    route.abort()


def main() -> None:
    if len(sys.argv) != 3:
        raise ValueError('usage: pages.py BINARY NEW_OUTPUT_DIRECTORY')
    binary = Path(sys.argv[1]).resolve()
    root = Path(sys.argv[2]).resolve()
    root.mkdir(parents=True, exist_ok=False)
    _ = (root / 'code.nepld').write_bytes(SOURCE.encode())
    _ = (root / 'target.nepld').write_text(TARGET)
    manifest = {'version': 1, 'pages': [
        {'id': 'code', 'source': 'code.nepld', 'route': 'code/index.html'},
        {'id': 'target', 'source': 'target.nepld', 'route': 'target/index.html'},
    ]}
    _ = (root / 'pages.json').write_text(json.dumps(manifest))
    output = root / 'output'
    _ = subprocess.run([str(binary), 'doc-html', 'pages', str(root / 'pages.json'), str(output)], check=True)
    results: list[dict[str, str | int]] = []
    with sync_playwright() as p:
        for implementation in (p.chromium, p.firefox, p.webkit):
            browser = implementation.launch()
            try:
                for theme in ('light', 'dark'):
                    for width in (375, 1280):
                        with browser.new_context(java_script_enabled=False, color_scheme=theme,
                                                 viewport={'width': width, 'height': 900}) as context:
                            _ = context.route(re.compile(r'^https?://'), block_network)
                            page = context.new_page()
                            _ = page.goto((output / 'code/index.html').as_uri())
                            blocks = page.locator('pre > code')
                            if blocks.count() != len(GUESTS):
                                raise AssertionError('Missing Code guests')
                            for index, expected in enumerate(GUESTS):
                                if blocks.nth(index).text_content() != expected:
                                    raise AssertionError(('Source changed', index))
                            if page.locator('math[display="inline"]').count() != 1 or page.locator('math[display="block"]').count() != 1:
                                raise AssertionError('Missing page Math')
                            for math in page.locator('math').all():
                                box = math.bounding_box()
                                if box is None or box['width'] <= 0 or box['height'] <= 0:
                                    raise AssertionError('Invisible page Math')
                            if page.locator('script').count():
                                raise AssertionError('Code escaped its text context')
                            style: object = page.evaluate(  # pyright: ignore[reportAny]
                                """() => {
                                    const token = document.querySelector('.nepl-code-marker');
                                    if (!token || !token.dataset.neplId.startsWith('pc-')) throw Error('Missing highlight identity');
                                    if (document.documentElement.scrollWidth > innerWidth + 1) throw Error('Overflow');
                                    return getComputedStyle(token).color;
                                }""")
                            expected_color = 'rgb(163, 32, 53)' if theme == 'light' else 'rgb(255, 158, 171)'
                            if style != expected_color:
                                raise AssertionError(('Wrong page stylesheet', theme, style))
                            page.get_by_role('link', name='Target', exact=True).click()
                            expect(page).to_have_url((output / 'target/index.html').as_uri() + '#n-746172676574')
                            # URL navigation may be observed before fragment targeting
                            # settles. Retry the real DOM assertion, never skip it.
                            expect(page.locator(':target')).to_have_count(1)
                            expect(page.locator(':target')).to_have_attribute('id', 'n-746172676574')
                            page.get_by_role('link', name='Back', exact=True).click()
                            expect(page).to_have_url((output / 'code/index.html').as_uri())
                            expect(page.locator('pre > code')).to_have_count(len(GUESTS))
                            results.append({'engine': implementation.name, 'theme': theme,
                                            'width': width, 'result': 'passed'})
            finally:
                browser.close()
    print(json.dumps({'cases': results}))


if __name__ == '__main__':
    main()
