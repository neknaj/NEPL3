"""Check actual SVG exports, conditional disclosures and resize in three browsers.

Usage: python tools/audit/doc_html/svg.py BINARY NEW_OUTPUT_DIRECTORY
Run in the permitted CI browser environment. No page JavaScript or network is used.
"""
from pathlib import Path
import json
import re
import subprocess
import sys

from playwright.sync_api import Page, Route, sync_playwright

CASES = (("small", 100, 60), ("medium", 400, 200), ("wide", 1200, 200), ("tall", 100, 800), ("glyphs", 100, 60))

MEASURE = """() => {
  const expected = [[100,60],[400,200],[1200,200],[100,800],[100,60]];
  const groups = [...document.querySelectorAll('.nepl-image-group')];
  if (groups.length !== expected.length) throw Error('Missing SVG groups');
  for (const [index, group] of groups.entries()) {
    const preview = group.querySelector('.nepl-image-preview > img');
    const details = group.querySelector(':scope > details');
    const original = details.querySelector('.nepl-image-original > img');
    const [width, height] = expected[index];
    if (!preview.complete || !preview.naturalWidth || !original.complete || !original.naturalWidth) throw Error('SVG did not load');
    const box = preview.getBoundingClientRect();
    const scale = Math.min(1, group.clientWidth / width, 360 / height);
    if (Math.abs(box.width - width * scale) > 0.2 || Math.abs(box.height - height * scale) > 0.2) throw Error('Preview scale or aspect ratio is wrong: '+index);
    const shrunk = box.width + 0.1 < width || box.height + 0.1 < height;
    const shown = getComputedStyle(details).display !== 'none';
    if (shrunk !== shown) throw Error('Disclosure disagrees with actual scale: '+index);
    if (box.height > 360.1 || box.width > group.clientWidth + 0.1) throw Error('Preview exceeds bound');
    if (box.width > width + 0.1 || box.height > height + 0.1) throw Error('Small image enlarged');
    if (details.open && shown) {
      const full = original.getBoundingClientRect();
      if (Math.abs(full.width-width) > 0.1 || Math.abs(full.height-height) > 0.1) throw Error('Expanded original has wrong size');
      if (!full.width || !full.height) throw Error('Expanded image is not rendered');
    }
  }
  if (document.scripts.length) throw Error('Unexpected page script');
  if (document.documentElement.scrollWidth > innerWidth + 1) throw Error('Page horizontal overflow');
  return 'passed';
}"""


def block_network(route: Route) -> None:
    route.abort()


def check_layout(page: Page) -> None:
    value: object = page.evaluate(MEASURE)  # pyright: ignore[reportAny]
    if value != "passed":
        raise AssertionError("Unexpected layout result")



def check_glyph_pixels(page: Page) -> None:
    # A complete image can still have missing use glyphs. Inspect fixed interior
    # pixels of the actual embedded image, including both reference spellings.
    result: object = page.evaluate(  # pyright: ignore[reportAny]
        """() => {
      const img = document.querySelectorAll('.nepl-image-preview > img')[4];
      if (!img.complete || img.naturalWidth !== 100) throw Error('Glyph image not loaded');
      const canvas = document.createElement('canvas');
      canvas.width = 100; canvas.height = 60;
      const ctx = canvas.getContext('2d');
      ctx.drawImage(img, 0, 0);
      for (const [x, y, color] of [[20,20,0],[55,20,0],[1,1,255],[35,20,255]]) {
        const p = ctx.getImageData(x,y,1,1).data;
        if (p[0] !== color || p[1] !== color || p[2] !== color || p[3] !== 255)
          throw Error('Missing or displaced path glyph at '+x+','+y+': '+[...p]);
      }
      return 'passed';
    }""")
    if result != "passed":
        raise AssertionError("Unexpected glyph pixel result")

def main() -> None:
    if len(sys.argv) != 3:
        raise ValueError(__doc__)
    binary = Path(sys.argv[1]).resolve()
    root = Path(sys.argv[2]).resolve()
    root.mkdir(parents=True, exist_ok=False)
    blocks: list[str] = []
    assets: list[dict[str, str]] = []
    for name, width, height in CASES:
        content = f"<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'><path d='M0 0L{width} 0L{width} {height}L0 {height}Z' fill='#fff'/><path d='M0 0L{width} {height}' stroke='#000'/></svg>"
        if name == "glyphs":
            content = "<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' width='100' height='60' viewBox='0 0 100 60'><defs><path id='g' d='M0 0L10 0L10 20L0 20Z'/></defs><path d='M0 0L100 0L100 60L0 60Z' fill='#fff'/><g transform='matrix(1 0 0 1 10 10)'><use href='#g' x='5' y='7'/><use xlink:href='#g' x='40' y='7'/></g></svg>"
        _ = (root / f"{name}.svg").write_text(content, encoding="utf-8")
        assets.append({"id": name, "source": f"{name}.svg", "mime": "image/svg+xml"})
        blocks.append(f'cons image asset "{name}" none "{name}" none')
    _ = (root / "source.nepld").write_text('article ja "SVG layout" body\n'+"\n".join(blocks)+'\nnil\n', encoding="utf-8")
    _ = (root / "assets.json").write_text(json.dumps({"version": 1, "assets": assets}), encoding="utf-8")
    for css in ("external", "inline"):
        for svg in ("external", "embedded"):
            _ = subprocess.run([str(binary), "doc-html", "svg", "--css", css, "--svg", svg,
                                str(root / "source.nepld"), str(root / "assets.json"), str(root / f"{css}-{svg}")], check=True)
    results: list[dict[str, str]] = []
    with sync_playwright() as playwright:
        for implementation in (playwright.chromium, playwright.firefox, playwright.webkit):
            with implementation.launch(headless=True) as browser:
                for css in ("external", "inline"):
                    for svg in ("external", "embedded"):
                        with browser.new_context(java_script_enabled=False, viewport={"width": 375, "height": 900}) as context:
                            _ = context.route(re.compile(r"^https?://"), block_network)
                            page = context.new_page()
                            _ = page.goto((root / f"{css}-{svg}/document.html").as_uri())
                            if svg == "embedded":
                                check_glyph_pixels(page)
                            for width in (375, 1280, 320, 375):
                                print(f"{implementation.name} css={css} svg={svg} width={width}", file=sys.stderr, flush=True)
                                page.set_viewport_size({"width": width, "height": 900})
                                check_layout(page)
                                details = page.locator('.nepl-image-details')
                                for i in range(details.count()):
                                    item = details.nth(i)
                                    if item.is_visible():
                                        summary = item.locator('summary')
                                        summary.click()
                                        if item.get_attribute("open") is None:
                                            raise AssertionError("Disclosure did not open after click")
                                        check_layout(page)
                                        if width == 375 and i == 2:
                                            _ = page.screenshot(path=str(root / f"{implementation.name}-{css}-{svg}-expanded.png"), full_page=True)
                                        summary.click()
                                        if item.get_attribute("open") is not None:
                                            raise AssertionError("Disclosure did not close after click")
                                        check_layout(page)
                            medium = page.locator('.nepl-image-details').nth(1)
                            medium.locator('summary').click()
                            if medium.get_attribute("open") is None:
                                raise AssertionError("Medium disclosure did not open")
                            page.set_viewport_size({"width": 1280, "height": 900})
                            check_layout(page)
                            if medium.is_visible():
                                raise AssertionError("Full-size medium still exposes disclosure")
                            page.set_viewport_size({"width": 375, "height": 900})
                            check_layout(page)
                            if not medium.is_visible() or medium.get_attribute("open") is None:
                                raise AssertionError("Open disclosure state was lost on resize")
                            medium.locator('summary').click()
                            if medium.get_attribute("open") is not None:
                                raise AssertionError("Medium disclosure did not close")
                            _ = page.screenshot(path=str(root / f"{implementation.name}-{css}-{svg}.png"), full_page=True)
                            results.append({"engine": implementation.name, "version": browser.version, "css": css, "svg": svg, "result": "passed"})
    print(json.dumps({"document_javascript": False, "network": False, "resize_sequence": [375, 1280, 320, 375], "results": results}, indent=2))


if __name__ == "__main__":
    main()
