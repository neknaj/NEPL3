"""Check actual Doc HTML corpus output in explicitly supplied real browsers.

Install Playwright in the host environment, then supply the --nocapture log of
doc::html::browser_layout_corpus_from_real_doc_source, the production stylesheet,
and both browser executables. Missing browsers/cases are failures, not skips.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path

def extract_cases(corpus):
    cases = {}
    for line in corpus.decode("utf-8-sig").splitlines():
        # libtest writes the test name before the first nocapture output when
        # tests are serialized. Accept that exact harness prefix, not arbitrary
        # log text containing a marker.
        line = re.sub(r"^test [A-Za-z0-9_:]+ \.\.\. (?=DOC_HTML_CASE )", "", line)
        if line.startswith("DOC_HTML_CASE "):
            _, name, text = line.split()
            if name in cases:
                raise ValueError("duplicate case: " + name)
            cases[name] = bytes.fromhex(text).decode("utf-8")
    expected = {"ruby", "anno", "anno-ruby", "ruby-anno", "ruby-ruby", "table-ruby", "list-ruby"}
    if cases.keys() != expected:
        raise ValueError("missing or unexpected corpus cases")
    return cases


def main():
    from playwright.sync_api import sync_playwright

    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["corpus", "css", "chromium", "firefox", "output"]:
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    corpus = args.corpus.read_bytes()
    css = args.css.read_bytes()
    cases = extract_cases(corpus)
    result = {"corpus_sha256": hashlib.sha256(corpus).hexdigest(),
              "css_sha256": hashlib.sha256(css).hexdigest(), "browsers": {}}
    with sync_playwright() as playwright:
        for name in ["chromium", "firefox"]:
            browser = getattr(playwright, name).launch(executable_path=str(getattr(args, name)), headless=True)
            # The generated document runs no scripts. Playwright's inspection
            # evaluates in its automation context, without changing the DOM.
            context = browser.new_context(java_script_enabled=False)
            page = context.new_page()
            rows = []
            for case, html in cases.items():
                for size in [12, 20, 32]:
                    for height in ["normal", "1.2", "2"]:
                        page.set_content("<!DOCTYPE html><meta charset=utf-8><style>" + css.decode("utf-8")
                                         + f".nepl-doc{{font-family:Arial,sans-serif;font-size:{size}px;line-height:{height}}}"
                                         + "</style>" + html)
                        measured = page.evaluate("""() => {
                          const bases = [...document.querySelectorAll('.nepl-base')];
                          const target = bases.find(n => n.childNodes.length === 1 && n.firstChild.nodeType === 3 && n.firstChild.data === 'B');
                          const walker = document.createTreeWalker(document.querySelector('article'), NodeFilter.SHOW_TEXT);
                          let reference; while(walker.nextNode()) if(walker.currentNode.data === 'A') {reference=walker.currentNode;break;}
                          if(!target || !reference) throw Error('missing original A/B text');
                          const rect = n => {const r=document.createRange();r.selectNodeContents(n);return r.getBoundingClientRect();};
                          return {difference:rect(target.firstChild).bottom-rect(reference).bottom,
                                  supported:CSS.supports('baseline-source','last') && CSS.supports('baseline-source','first'),
                                  scripts:document.scripts.length};
                        }""")
                        rows.append({"case": case, "size": size, "line_height": height, **measured})
            result["browsers"][name] = {"version": browser.version, "measurements": rows}
            browser.close()
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    for browser in result["browsers"].values():
        if any(not r["supported"] or r["scripts"] or abs(r["difference"]) >= .1 for r in browser["measurements"]):
            raise ValueError("baseline or scriptless document check failed; see output")
    print("Doc HTML: 126 actual-output baseline checks passed in two browsers with document JavaScript disabled.")


if __name__ == "__main__":
    main()
