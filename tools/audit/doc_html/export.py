"""Verify actual local exports and a detached inline HTML in three browsers.

Usage: python tools/audit/doc_html/export.py BINARY NEW_OUTPUT_DIRECTORY
Missing engines or mismatched layout fail; no document JavaScript or network.
"""
from pathlib import Path
import json
import re
import shutil
import subprocess
import sys

from playwright.sync_api import Route, sync_playwright

SOURCE = '''article ja "[文書/ぶんしょ]" body
cons section layout "[見出/みだ]し" body
cons paragraph cons "{[本文/ほんぶん]/body} & <script>window.untrusted=1</script>" nil
cons rawcode some "text" "example code"
cons table cons left nil some row cons "Heading" nil cons row cons "Cell" nil nil
nil nil'''

MEASURE = """() => {
  const ruby = document.querySelector('.nepl-ruby');
  const base = ruby.querySelector('.nepl-base').getBoundingClientRect();
  const reading = ruby.querySelector('.nepl-reading').getBoundingClientRect();
  if (reading.bottom > base.top + 1) throw Error('Ruby reading is not above base');
  if (document.scripts.length) throw Error('Unexpected script');
  const selectors = ['.nepl-doc', 'h1', 'h2', '.nepl-paragraph', '.nepl-ruby',
                     '.nepl-reading', '.nepl-anno', '.nepl-notes', 'pre', 'table', 'th', 'td'];
  return JSON.stringify(selectors.map(selector => {
    const node = document.querySelector(selector);
    if (!node) throw Error('Missing ' + selector);
    const style = getComputedStyle(node);
    const box = node.getBoundingClientRect();
    return [selector, box.x, box.y, box.width, box.height, style.color,
            style.backgroundColor, style.fontSize, style.margin, style.padding,
            style.display, style.borderTopWidth];
  }));
}"""


def block_network(route: Route) -> None:
    route.abort()


def main() -> None:
    if len(sys.argv) != 3:
        raise ValueError(__doc__)
    binary = Path(sys.argv[1]).resolve()
    root = Path(sys.argv[2]).resolve()
    root.mkdir(parents=True)
    source = root / "source.nepld"
    _ = source.write_text(SOURCE, encoding="utf-8")
    for mode in ("external", "inline"):
        _ = subprocess.run([str(binary), "doc-html", "export", "--css", mode,
                            str(source), str(root / mode)], check=True)
    detached = root / "detached"
    detached.mkdir()
    _ = shutil.copyfile(root / "inline/document.html", detached / "document.html")
    inline = (detached / "document.html").read_text(encoding="utf-8")
    _ = (detached / "changed-style.html").write_text(
        inline.replace("<style>", "<style>/* changed */", 1), encoding="utf-8")
    _ = (detached / "script.html").write_text(
        inline.replace("</body>", "<script>globalThis.__neplProbe=1</script></body>"),
        encoding="utf-8")
    results: list[dict[str, str]] = []
    with sync_playwright() as playwright:
        for implementation in (playwright.chromium, playwright.firefox, playwright.webkit):
            with implementation.launch(headless=True) as browser:
                with browser.new_context(java_script_enabled=False,
                                         viewport={"width": 1000, "height": 900}) as context:
                    # WebKit's emulated offline mode rejects file:// navigation.
                    # Block network requests directly while preserving real file
                    # loading, CSP and stylesheet resolution in all engines.
                    _ = context.route(re.compile(r"^https?://"), block_network)
                    measurements: list[str] = []
                    for mode in ("external", "inline", "detached"):
                        page = context.new_page()
                        _ = page.goto((root / mode / "document.html").as_uri())
                        value: object = page.evaluate(MEASURE)  # pyright: ignore[reportAny]
                        if not isinstance(value, str):
                            raise TypeError("Expected serialized layout")
                        measurements.append(value)
                        page.close()
                    if len(set(measurements)) != 1:
                        raise AssertionError(f"CSS modes differ in {implementation.name}")
                    page = context.new_page()
                    _ = page.goto((detached / "changed-style.html").as_uri())
                    style_probe = "getComputedStyle(document.querySelector('.nepl-ruby')).display === 'inline'"
                    style_blocked: object = page.evaluate(style_probe)  # pyright: ignore[reportAny]
                    if style_blocked is not True:
                        raise AssertionError("Changed CSS was not blocked by CSP")
                    page.close()
                    results.append({"engine": implementation.name, "result": "passed",
                                    "version": browser.version})
                with browser.new_context(java_script_enabled=True) as scripts:
                    _ = scripts.route(re.compile(r"^https?://"), block_network)
                    page = scripts.new_page()
                    _ = page.goto((detached / "script.html").as_uri())
                    script_blocked: object = page.evaluate("globalThis.__neplProbe === undefined")  # pyright: ignore[reportAny]
                    if script_blocked is not True:
                        raise AssertionError("Injected script was not blocked by CSP")
    print(json.dumps({"document_javascript": False, "script_csp_negative_control": True, "network": False, "results": results}, indent=2))


if __name__ == "__main__":
    main()
