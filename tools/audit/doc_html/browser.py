"""Measure production Doc HTML in Chromium, Firefox and WebKit, without document JS.

Supply the --nocapture log of browser_layout_corpus_from_real_doc_source and the
production stylesheet. Full exported documents retain their CSP and load the
stylesheet through a same-origin intercepted request. No network server is used.
By default use the pinned Playwright browser binaries;
optional executable paths allow explicit local runners. Missing engines fail.
This checks static document layout, not browser Wasm or Playground completion.
"""
import argparse
import hashlib
import json
import re
from importlib.metadata import version
from pathlib import Path
from collections.abc import Mapping
from types import MappingProxyType
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.audit.doc_html.layout import (Browser, CaseName, Engine, Failed, Incomplete, LineHeight,
                                       Outcome, Passed, Report, Row, case_name, geometry, valid_measurement)

CASES: frozenset[CaseName] = frozenset({"ruby", "anno", "anno-ruby", "ruby-anno", "ruby-ruby", "table-ruby",
         "list-ruby", "ruby-multiline", "anno-multiline", "reading-ruby",
         "notes-ruby", "anno-anno", "line-reservation"})
BROWSERS: tuple[Engine, ...] = ('chromium', 'firefox', 'webkit')


def extract_cases(corpus: bytes) -> Mapping[CaseName, str]:
    cases: dict[CaseName, str] = {}
    for line in corpus.decode("utf-8-sig").splitlines():
        # libtest can put the serialized test name before the first marker.
        line = re.sub(r"^test [A-Za-z0-9_:]+ \.\.\. (?=DOC_HTML_CASE )", "", line)
        if line.startswith("DOC_HTML_CASE "):
            _, name, text = line.split()
            if name in cases:
                raise ValueError("duplicate case: " + name)
            cases[case_name(name)] = bytes.fromhex(text).decode("utf-8")
    if frozenset(cases) != CASES:
        raise ValueError("missing or unexpected corpus cases")
    return MappingProxyType(cases)


# A and B use the same font and size: their text rectangle bottoms must match.
# Multiline B is placed on Ruby's last / Anno's first line by the source corpus.
# Property support is recorded, not assumed to prove correct layout.
MEASURE = """([caseName, fontSize]) => {
  const article = document.querySelector('article');
  if(getComputedStyle(article).fontSize !== `${fontSize}px`)
    throw Error('requested stylesheet parameters were not applied');
  if(document.styleSheets.length !== 1 ||
     document.styleSheets[0].href !== 'https://nepl3-doc.invalid/assets/doc.css')
    throw Error('expected the exported same-origin stylesheet');
  if(document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.content !==
     "default-src 'none'; style-src 'self'; base-uri 'none'; form-action 'none'")
    throw Error('exported content security policy changed');
  const walker = document.createTreeWalker(article, NodeFilter.SHOW_TEXT);
  const texts = []; while(walker.nextNode()) texts.push(walker.currentNode);
  const one = text => {
    const found = texts.filter(n => n.data === text);
    if(found.length !== 1) throw Error('expected one original text: ' + text);
    return found[0];
  };
  const rect = n => {const r = document.createRange();r.selectNodeContents(n);return r.getBoundingClientRect();};
  const annotations = [...article.querySelectorAll('.nepl-ruby,.nepl-anno')];
  // The corpus has no implicit breaks in individual annotation text nodes.
  // Long readings must enlarge the atomic box instead of wrapping in captions.
  const textFragments = texts.filter(n => n.parentElement.closest('.nepl-ruby,.nepl-anno')).map(n => {
    const r = document.createRange();r.selectNodeContents(n);return r.getClientRects().length;
  });
  const gaps = annotations.map(n => {
    const base = n.querySelector(':scope > .nepl-base');
    const reading = n.querySelector(':scope > .nepl-reading');
    const notes = n.querySelector(':scope > .nepl-notes');
    if(!base || (!reading && !notes)) throw Error('missing annotation structure');
    const b = base.getBoundingClientRect();
    return reading ? b.top - reading.getBoundingClientRect().bottom
                   : notes.getBoundingClientRect().top - b.bottom;
  });
  const breaks = article.querySelectorAll('br').length;
  const multiline = caseName === 'ruby-multiline' || caseName === 'anno-multiline';
  const expectedBreaks = multiline ? 1 : caseName === 'line-reservation' ? 2 : 0;
  if(breaks !== expectedBreaks) throw Error('unexpected explicit break count');
  let multilineGap = null;
  if(multiline) {
    const b = rect(one('B')), d = rect(one('D'));
    multilineGap = caseName === 'ruby-multiline' ? b.top - d.top : d.top - b.top;
  }
  let lineGaps = [];
  if(caseName === 'line-reservation') {
    const annotation = annotations[0].getBoundingClientRect();
    lineGaps = [annotation.top - rect(one('Z')).bottom,
                rect(one('Y')).top - annotation.bottom];
  }
  return JSON.stringify({difference: rect(one('B')).bottom - rect(one('A')).bottom,
          baseline_source_supported: CSS.supports('baseline-source','first') && CSS.supports('baseline-source','last'),
          annotation_gaps: gaps, line_gaps: lineGaps, multiline_gap: multilineGap,
          annotation_text_fragments: textFragments,
          display: getComputedStyle(annotations[0]).display,
          scripts: document.scripts.length});
}"""


class Arguments(argparse.Namespace):
    corpus: Path = Path()
    css: Path = Path()
    output: Path = Path()
    chromium: Path | None = None
    firefox: Path | None = None
    webkit: Path | None = None


def main() -> None:
    from playwright.sync_api import Route, sync_playwright

    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["corpus", "css", "output"]:
        _ = parser.add_argument("--" + name, required=True, type=Path)
    for name in BROWSERS:
        _ = parser.add_argument("--" + name, type=Path)
    args = parser.parse_args(namespace=Arguments())
    corpus, css = args.corpus.read_bytes(), args.css.read_bytes()
    cases = extract_cases(corpus)
    corpus_sha256, css_sha256 = hashlib.sha256(corpus).hexdigest(), hashlib.sha256(css).hexdigest()
    playwright_version = version('playwright')
    browsers: list[Browser] = []
    outcome: Outcome = Incomplete()
    try:
        with sync_playwright() as playwright:
            implementations = (playwright.chromium, playwright.firefox, playwright.webkit)
            paths = (args.chromium, args.firefox, args.webkit)
            for name, implementation, executable in zip(BROWSERS, implementations, paths, strict=True):
                browser = implementation.launch(headless=True, executable_path=executable)
                rows: list[Row] = []
                try:
                    for width in [375, 1280]:
                        context = browser.new_context(java_script_enabled=False, service_workers='block',
                                                      viewport={"width": width, "height": 900})
                        page = context.new_page()
                        document = ""
                        stylesheet = ""

                        def serve(route: Route) -> None:
                            match route.request.url:
                                case "https://nepl3-doc.invalid/document.html":
                                    route.fulfill(content_type="text/html; charset=utf-8", body=document)
                                case "https://nepl3-doc.invalid/assets/doc.css":
                                    route.fulfill(content_type="text/css; charset=utf-8", body=stylesheet)
                                case _:
                                    route.abort()

                        _ = page.route("**/*", serve)
                        for case, html in cases.items():
                            for size in [12, 20, 32]:
                                heights: tuple[LineHeight, ...] = ('normal', '1.2', '2')
                                for height in heights:
                                    document = html
                                    stylesheet = css.decode("utf-8") + f"\n.nepl-doc{{font-family:Arial,sans-serif;font-size:{size}px;line-height:{height}}}"
                                    _ = page.goto("https://nepl3-doc.invalid/document.html", wait_until="load")
                                    measured: object = page.evaluate(MEASURE, [case, size])  # pyright: ignore[reportAny]
                                    rows.append(Row(case, width, size, height, geometry(measured)))
                        context.close()
                finally:
                    browsers.append(Browser(name, browser.version, tuple(rows)))
                    browser.close()
        if any(not valid_measurement(row) for browser in browsers for row in browser.measurements):
            raise ValueError("annotation layout or scriptless document check failed; see output")
        outcome = Passed()
    except Exception as error:
        outcome = Failed(str(error))
        raise
    finally:
        report = Report(corpus_sha256, css_sha256, playwright_version, tuple(browsers), outcome)
        _ = args.output.write_text(json.dumps(report.representation(), indent=2) + "\n", encoding="utf-8", newline="\n")
    count = sum(len(browser.measurements) for browser in browsers)
    print(f"Doc HTML: {count} actual-output layout checks passed in three browsers with document JavaScript disabled.")


if __name__ == "__main__":
    main()
