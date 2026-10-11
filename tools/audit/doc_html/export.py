"""Verify actual local exports and a detached inline HTML in three browsers.

Usage: python tools/audit/doc_html/export.py BINARY NEW_OUTPUT_DIRECTORY
Missing engines or mismatched layout fail. Layout uses no document JavaScript or network;
a separate font smoke check verifies the explicitly configured Google Fonts delivery.
"""
from pathlib import Path
import json
import re
import shutil
import subprocess
import sys

from playwright.sync_api import Route, sync_playwright
from fonts import wait_for_fonts


GOOGLE_FONTS = "https://fonts.googleapis.com/css2?family=Klee+One:wght@400;600&display=swap"

SOURCE = '''article ja "[文書/ぶんしょ]" body
cons section layout "[見出/みだ]し" body
cons paragraph cons "{[本文/ほんぶん]/body} & <script>window.untrusted=1</script>" nil
cons paragraph cons "{Please/依頼標識} {{create/V} {{an/Det} {issue/N}/NP O}/VP} {{on/P} {GitHub/N}/PP}" nil
cons paragraph cons "{{{long_annotation_scope/N}/NP}/VP}" nil
cons paragraph cons parallel cons variant en sentence cons text "The fraction " cons math Math frac 1 2 cons text " represents one of two equal parts of a whole." nil cons variant ja sentence cons text "分数 " cons math Math frac 1 2 cons text " は、全体を等しく二つに分けたうちの一つを表す。" nil nil nil
cons display Math label frac 3 4 Sentence "{[数/すう]/number}"
cons rawcode some "text" "example code"
cons rawcode none "long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_long_code_token_"
cons table cons left cons center cons right nil some row cons "Left" cons "Center" cons "Right" nil cons row cons "A" cons "B" cons "C" nil nil
nil nil'''

MEASURE = """() => {
  const maths = [...document.querySelectorAll('math')];
  if (maths.length !== 3) throw Error('Missing embedded Math');
  if (document.querySelectorAll('math[display="inline"]').length !== 2 || document.querySelectorAll('math[display="block"]').length !== 1) throw Error('Wrong Math placement');
  if (document.querySelector('p math[display="block"]')) throw Error('Display Math inside paragraph sentence run');
  for (const math of maths) {
    const box = math.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) throw Error('Invisible Math');
  }
  for (const frac of document.querySelectorAll('mfrac')) {
    const numerator = frac.children[0].getBoundingClientRect();
    const denominator = frac.children[1].getBoundingClientRect();
    if (numerator.bottom > denominator.top + 1) throw Error('Fraction layout missing');
  }
  const ruby = document.querySelector('.nepl-ruby');
  for (const node of document.querySelectorAll('.nepl-ruby,.nepl-anno')) {
    const base = node.querySelector(':scope > .nepl-base').getBoundingClientRect();
    const reading = node.querySelector(':scope > .nepl-reading');
    const notes = node.querySelector(':scope > .nepl-notes');
    if (reading && reading.getBoundingClientRect().bottom > base.top + 1) throw Error('Ruby reading overlaps base');
    if (notes && notes.getBoundingClientRect().top < base.bottom - 1) throw Error('Annotation notes overlap base');
  }
  for (const node of document.querySelectorAll('.nepl-anno')) {
    const base = node.querySelector(':scope > .nepl-base');
    const style = getComputedStyle(base);
    const edge = getComputedStyle(base, '::after');
    if (style.position !== 'relative' || edge.content === 'none' || edge.content === 'normal' || edge.pointerEvents !== 'none') throw Error('Annotation bracket is missing or intercepts input');
    if (edge.opacity !== '0.5') throw Error('Annotation bracket opacity must be 50%');
    if (edge.position !== 'absolute' || edge.bottom !== '0px' || edge.left !== '0px' || edge.right !== '0px') throw Error('Annotation bracket does not follow its scope');
    if (edge.borderBottomWidth !== '1px' || edge.borderLeftWidth !== '1px' || edge.borderRightWidth !== '1px') throw Error('Missing annotation endpoints');
    if (parseFloat(edge.borderBottomLeftRadius) <= 0 || parseFloat(edge.borderBottomRightRadius) <= 0) throw Error('Annotation bracket is not rounded');
    if (style.backgroundColor === 'rgba(0, 0, 0, 0)') throw Error('Missing scope background');
    if (base.scrollWidth > base.clientWidth + 1) throw Error('Annotation base clips nested contents');
  }
  for (const node of document.querySelectorAll('.nepl-ruby > .nepl-base')) {
    const style = getComputedStyle(node);
    if ([style.borderTopWidth, style.borderRightWidth, style.borderBottomWidth, style.borderLeftWidth].some(width => width !== '0px')) throw Error('Ruby base must remain borderless');
  }
  if (document.documentElement.scrollWidth > innerWidth) throw Error('Horizontal overflow');
  const main = getComputedStyle(document.querySelector('.nepl-doc'));
  const small = getComputedStyle(ruby.querySelector('.nepl-reading'));
  const rootSize = parseFloat(getComputedStyle(document.documentElement).fontSize);
  if (Math.abs(parseFloat(main.fontSize) / rootSize - 1.2) > 0.01) throw Error('Wrong document scale');
  if (!main.fontFamily.includes('Klee One')) throw Error('Missing font fallback stack');
  if (Math.abs(parseFloat(small.fontSize) / parseFloat(getComputedStyle(ruby).fontSize) - 0.6) > 0.01) throw Error('Wrong annotation scale');
  if (small.color !== 'rgb(122, 143, 166)') throw Error('Wrong annotation color');
  for (const pre of document.querySelectorAll('pre')) {
    const block = getComputedStyle(pre);
    const code = getComputedStyle(pre.querySelector('code'));
    const figure = getComputedStyle(pre.parentElement);
    const parent = getComputedStyle(pre.parentElement.parentElement);
    if (Math.abs(parseFloat(block.fontSize) / parseFloat(parent.fontSize) - 0.92) > 0.01) throw Error('Wrong code block scale');
    if (code.fontSize !== block.fontSize || code.fontFamily !== block.fontFamily) throw Error('Nested code shrinks or changes font');
    if (!code.fontFamily.includes('monospace')) throw Error('Code is not monospace');
    if (parseFloat(figure.marginLeft) || parseFloat(figure.marginRight)) throw Error('Code figure has browser default inset');
    const caption = pre.parentElement.querySelector('figcaption');
    if (caption) {
      const label = getComputedStyle(caption);
      if (Math.abs(parseFloat(label.lineHeight) / parseFloat(label.fontSize) - 1.4) > 0.01) throw Error('Code caption inherits prose leading');
    }
    if (pre.getBoundingClientRect().right > innerWidth + 1) throw Error('Code block escapes viewport');
  }
  const alignments = [...document.querySelectorAll('td')].map(node => getComputedStyle(node).textAlign);
  if (JSON.stringify(alignments) !== JSON.stringify(['left', 'center', 'right'])) throw Error('Column alignment changed');
  if (document.scripts.length) throw Error('Unexpected script');
  const selectors = ['.nepl-doc', 'h1', 'h2', '.nepl-paragraph', '.nepl-ruby',
                     '.nepl-reading', '.nepl-anno', '.nepl-notes', 'figure', 'figcaption', 'pre', 'pre>code', 'table', 'th', 'td'];
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
            print(f"Checking {implementation.name}", file=sys.stderr, flush=True)
            with implementation.launch(headless=True) as browser:
                for width in (320, 375, 1280):
                    with browser.new_context(java_script_enabled=False, color_scheme="dark",
                                             viewport={"width": width, "height": 900}) as context:
                        # WebKit's emulated offline mode rejects file:// navigation.
                        # Block network requests directly while preserving real file
                        # loading, CSP and stylesheet resolution in all engines.
                        _ = context.route(re.compile(r"^https?://"), block_network)
                        measurements: list[str] = []
                        for mode in ("external", "inline", "detached"):
                            print(f"{implementation.name}: offline {width} {mode}", file=sys.stderr, flush=True)
                            page = context.new_page()
                            _ = page.goto((root / mode / "document.html").as_uri())
                            value: object = page.evaluate(MEASURE)  # pyright: ignore[reportAny]
                            if not isinstance(value, str):
                                raise TypeError("Expected serialized layout")
                            measurements.append(value)
                            _ = page.screenshot(path=str(root / f"{implementation.name}-{width}-{mode}.png"), full_page=True, timeout=15000)
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
                                        "version": browser.version, "width": str(width)})
                with browser.new_context(java_script_enabled=True) as scripts:
                    _ = scripts.route(re.compile(r"^https?://"), block_network)
                    page = scripts.new_page()
                    _ = page.goto((detached / "script.html").as_uri())
                    script_blocked: object = page.evaluate("globalThis.__neplProbe === undefined")  # pyright: ignore[reportAny]
                    if script_blocked is not True:
                        raise AssertionError("Injected script was not blocked by CSP")
                for width in (320, 375, 1280):
                    with browser.new_context(java_script_enabled=False, color_scheme="dark",
                                             viewport={"width": width, "height": 900}) as online:
                        page = online.new_page()
                        _ = page.goto((detached / "document.html").as_uri())
                        print(f"{implementation.name}: online font {width}", file=sys.stderr, flush=True)
                        wait_for_fonts(page)
                        online_layout: object = page.evaluate(MEASURE)  # pyright: ignore[reportAny]
                        if not isinstance(online_layout, str):
                            raise TypeError("Expected online layout measurement")
                        _ = page.screenshot(path=str(root / f"{implementation.name}-{width}-google-fonts.png"), full_page=True, timeout=15000)
                        results.append({"engine": implementation.name, "font": "Klee One",
                                        "font_source": GOOGLE_FONTS, "width": str(width),
                                        "layout": online_layout, "result": "passed"})
    print(json.dumps({"document_javascript": False, "script_csp_negative_control": True, "layout_network": False, "font_network": "Google Fonts", "results": results}, indent=2))


if __name__ == "__main__":
    main()
