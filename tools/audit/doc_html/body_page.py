"""Verify the actual offline Body-page CLI in three browsers, with CSP controls.

Only run in the authorized hosted CI browser environment. No document JavaScript
or external resource is needed for normal layout; automation evaluates probes.
"""
import base64
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Literal

from playwright.sync_api import Route, sync_playwright

SOURCE = '''body
cons paragraph cons "{[本文/ほんぶん]/body} & <script>untrusted</script>" nil
cons section first "First" body cons section nested "Nested" body nil nil
cons paragraph cons parallel cons variant en "English" cons variant ja "日本語" nil nil
cons paragraph cons sentence cons ref target text "forward" cons anchor target text "destination" nil nil
cons section second "Second" body nil nil'''
TITLE = '</title><script>untrusted</script>&'
MEASURE = """() => {
  const root = document.querySelector('body > div.nepl-doc');
  if (!root || document.querySelector('article,script,link,img,iframe')) throw Error('Unexpected page structure');
  if (document.documentElement.lang !== 'en') throw Error('Lost metadata language');
  if ([...root.querySelectorAll('h1')].map(n => n.textContent).join('|') !== 'First|Second') throw Error('Body headings changed');
  if (!root.textContent.includes('English') || !root.textContent.includes('日本語')) throw Error('Metadata selected a variant');
  if (!root.querySelector('.nepl-parallel-rows')) throw Error('Wrong parallel mode');
  if (!root.querySelector('a[href="#n-746172676574"]') || !document.getElementById('n-746172676574')) throw Error('Lost local reference');
  if (document.documentElement.scrollWidth > innerWidth) throw Error('Horizontal overflow');
  for (const node of root.querySelectorAll('.nepl-ruby,.nepl-anno')) {
    const base = node.querySelector(':scope > .nepl-base').getBoundingClientRect();
    const reading = node.querySelector(':scope > .nepl-reading');
    const notes = node.querySelector(':scope > .nepl-notes');
    if (base.width <= 0 || base.height <= 0) throw Error('Invisible annotation base');
    if (reading && reading.getBoundingClientRect().bottom > base.top + 1) throw Error('Ruby overlaps base');
    if (notes && notes.getBoundingClientRect().top < base.bottom - 1) throw Error('Anno overlaps base');
  }
  const style = getComputedStyle(root);
  return JSON.stringify({background:style.backgroundColor,color:style.color,display:getComputedStyle(root.querySelector('.nepl-ruby')).display});
}"""


def main() -> None:
    binary = Path(sys.argv[1]).resolve()
    root = Path(sys.argv[2]).resolve()
    root.mkdir(parents=True, exist_ok=False)
    source, metadata, output = root / "body.nepld", root / "metadata.json", root / "page.html"
    _ = source.write_text(SOURCE, encoding="utf-8")
    _ = metadata.write_text(json.dumps({"title": TITLE, "language": "en"}), encoding="utf-8")
    _ = subprocess.run([str(binary), "doc-html", "body-page", "--metadata", str(metadata), str(source), str(output)], check=True)
    html = output.read_text(encoding="utf-8")
    style = re.search(r"<style>(.*?)</style>", html, flags=re.DOTALL)
    if style is None:
        raise AssertionError("Missing inline stylesheet")
    digest = base64.b64encode(hashlib.sha256(style.group(1).encode("utf-8")).digest()).decode("ascii")
    policy = f"default-src 'none'; style-src 'sha256-{digest}'; base-uri 'none'; form-action 'none'"
    if f'content="{policy}"' not in html:
        raise AssertionError("Unexpected CSP or stylesheet hash")
    if any(token in html for token in ("<script", "<link", "https://fonts", "<article")):
        raise AssertionError("Unexpected resource or structural tag")
    changed = root / "changed-style.html"
    injected = root / "script.html"
    _ = changed.write_text(html.replace("<style>", "<style>/* tampered */", 1), encoding="utf-8")
    _ = injected.write_text(html.replace("</body>", "<script>globalThis.__bodyProbe=1</script></body>"), encoding="utf-8")
    results: list[dict[str, str | int]] = []
    schemes: tuple[Literal["dark"], Literal["light"]] = ("dark", "light")
    with sync_playwright() as playwright:
        for implementation in (playwright.chromium, playwright.firefox, playwright.webkit):
            with implementation.launch(headless=True) as browser:
                requests: list[str] = []

                def block_network(route: Route) -> None:
                    requests.append(route.request.url)
                    route.abort()

                for color in schemes:
                    for width in (320, 1280):
                        with browser.new_context(java_script_enabled=False, color_scheme=color, viewport={"width": width, "height": 900}) as context:
                            _ = context.route(re.compile(r"^https?://"), block_network)
                            page = context.new_page()
                            _ = page.goto(output.as_uri())
                            if page.title() != TITLE:
                                raise AssertionError("Title metadata was not escaped/preserved")
                            measurement: object = page.evaluate(MEASURE)  # pyright: ignore[reportAny]
                            if not isinstance(measurement, str):
                                raise TypeError("Expected serialized layout")
                            expected = "rgb(255, 255, 255)" if color == "light" else "rgb(13, 17, 23)"
                            if expected not in measurement:
                                raise AssertionError("Color scheme not applied")
                            _ = page.screenshot(path=str(root / f"{implementation.name}-{color}-{width}.png"), full_page=True, timeout=15000)
                            results.append({"engine": implementation.name, "version": browser.version, "color": color, "width": width, "layout": measurement})
                with browser.new_context(java_script_enabled=True) as context:
                    _ = context.route(re.compile(r"^https?://"), block_network)
                    page = context.new_page()
                    _ = page.goto(injected.as_uri())
                    blocked: object = page.evaluate("globalThis.__bodyProbe === undefined")  # pyright: ignore[reportAny]
                    if blocked is not True:
                        raise AssertionError("Injected script executed")
                    _ = page.goto(changed.as_uri())
                    css_blocked: object = page.evaluate("getComputedStyle(document.querySelector('.nepl-ruby')).display === 'inline'")  # pyright: ignore[reportAny]
                    if css_blocked is not True:
                        raise AssertionError("Changed stylesheet was accepted")
                if requests:
                    raise AssertionError(f"Unexpected external requests: {requests}")
    print(json.dumps({"document_javascript": False, "network": False, "csp_negative_controls": True, "results": results}, indent=2))


if __name__ == "__main__":
    main()
