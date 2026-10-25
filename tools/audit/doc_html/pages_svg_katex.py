"""Check default KaTeX-preferred SVG PageSet CLI offline in real browsers."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue, array, decode, integer, object_value, string


def main() -> None:
    from playwright.sync_api import Route, expect, sync_playwright

    def block_network(route: Route) -> None:
        route.abort()

    if len(sys.argv) != 4:
        raise ValueError("pages_svg_katex.py BINARY NEW_OUTPUT_DIRECTORY PACKAGES")
    binary, root, packages = (Path(arg).resolve() for arg in sys.argv[1:])
    root.mkdir(parents=True, exist_ok=False)
    repo = Path(__file__).resolve().parents[3]
    example = repo / "examples/document/pages-svg"
    selections: tuple[tuple[str, JsonValue, int, str, str], ...] = (
        ("Rows", {"mode": "rows"}, 2, "Open the wide diagram.", "Return to the first page."),
        ("Columns", {"mode": "columns"}, 2, "Open the wide diagram.", "Return to the first page."),
        ("Single", {"mode": "single", "language": "fr", "fallbacks": ["ja"]}, 1, "横長の図を開く。", "最初のページへ戻る。"),
    )
    cases: list[tuple[str, Path, int, str, str]] = []
    for mode, selection, count, next_label, back_label in selections:
        inputs = root / mode / "inputs"
        inputs.mkdir(parents=True)
        for name in ("one.nepld", "two.nepld", "small.svg", "wide.svg"):
            _ = (inputs / name).write_bytes((example / name).read_bytes())
        manifest = dict(object_value(decode((example / "manifest.json").read_bytes())))
        manifest["display"] = selection
        _ = (inputs / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        output = root / mode / "output"
        env = dict(os.environ, NEPL3_KATEX_NODE_MODULES=str(packages))
        _ = subprocess.run([str(binary), "doc-html", "pages-svg", str(inputs / "manifest.json"), str(output)], env=env, check=True)
        receipt = object_value(decode((output / "manifest.json").read_bytes(), reject_duplicates=True, reject_nonfinite=True))
        if string(receipt["format"]) != "nepl3.local-doc-pages.root-svg-package/2":
            raise AssertionError("Preferred receipt contract")
        generation = object_value(receipt["katex_generation"])
        if integer(generation["successful_visual_pages"]) != 1 or integer(generation["successful_visuals"]) != count or generation["host_invocation_recorded"] is not False:
            raise AssertionError("Visual evidence mismatch")
        source = object_value(receipt["source_input"])
        if string(source["math_renderer"]) != "katex-preferred":
            raise AssertionError("Requested renderer mismatch")
        records = array(receipt["files"])
        if len(records) != 4:
            raise AssertionError("Unexpected payload count")
        for raw in records:
            record = object_value(raw)
            name = string(record["path"])
            if re.fullmatch(r"(?:one|two)/(?:index\.html|assets/doc-[a-f0-9]{64}\.css)", name) is None:
                raise AssertionError("Unexpected payload path")
            data = (output / name).read_bytes()
            if len(data) != integer(record["bytes"]) or hashlib.sha256(data).hexdigest() != string(record["sha256"]):
                raise AssertionError("Payload digest mismatch")
        cases.append((mode, output, count, next_label, back_label))
    results: list[dict[str, str | int]] = []
    with sync_playwright() as p:
        for implementation in (p.chromium, p.firefox, p.webkit):
            browser = implementation.launch()
            try:
                for mode, output, math_count, next_label, back_label in cases:
                    for theme in ("light", "dark"):
                        for width in (375, 1280):
                            with browser.new_context(java_script_enabled=False, color_scheme=theme, viewport={"width": width, "height": 900}) as context:
                                _ = context.route(re.compile(r"^https?://"), block_network)
                                page = context.new_page()
                                _ = page.goto((output / "one/index.html").as_uri())
                                for name, image_width, image_height, formulas in (("one", 100, 60, math_count), ("two", 600, 200, 0)):
                                    deadline = time.monotonic() + 15
                                    while time.monotonic() < deadline:
                                        loaded: object = page.evaluate("document.fonts.status === 'loaded'")  # pyright: ignore[reportAny]
                                        if loaded is True:
                                            break
                                        time.sleep(0.05)
                                    else:
                                        raise AssertionError("font deadline")
                                    expect(page.locator(".nepl-image-group")).to_have_count(1)
                                    expect(page.locator("math")).to_have_count(formulas)
                                    expect(page.locator(".katex")).to_have_count(formulas)
                                    measured: object = page.evaluate(  # pyright: ignore[reportAny]
                                        """([width,height,count]) => {
                                          const csp = document.querySelector('meta[http-equiv="Content-Security-Policy"]').content;
                                          if (!csp.includes('img-src data:;') || csp.includes('font-src data:') !== (count>0)) throw Error('CSP leakage');
                                          if (document.scripts.length || document.querySelector('[style]')) throw Error('Active content');
                                          for (const math of document.querySelectorAll('math')) {
                                            const owner=math.closest('.nepl-math-accessible');
                                            if (!owner || math.closest('[aria-hidden="true"]')) throw Error('Independent MathML lost');
                                            if (getComputedStyle(owner).position!=='absolute' || getComputedStyle(owner).width!=='1px') throw Error('MathML CSS absent');
                                          }
                                          for (const visual of document.querySelectorAll('.katex')) {
                                            const box=visual.getBoundingClientRect();
                                            if (!visual.closest('[aria-hidden="true"]') || box.width<=0 || box.height<=0) throw Error('Visual missing or duplicate accessibility');
                                          }
                                          if (count>0 && (!document.fonts.check('16px KaTeX_Main') || ![...document.fonts].some(f=>f.family.startsWith('KaTeX_')&&f.status==='loaded'))) throw Error('Embedded font absent');
                                          const group=document.querySelector('.nepl-image-group');
                                          const image=group.querySelector('.nepl-image-preview > img');
                                          const details=group.querySelector(':scope > details');
                                          if (!image.complete || image.naturalWidth!==width || image.naturalHeight!==height) throw Error('SVG unavailable');
                                          const box=image.getBoundingClientRect(), scale=Math.min(1,group.clientWidth/width,360/height);
                                          if (Math.abs(box.width-width*scale)>0.3 || Math.abs(box.height-height*scale)>0.3) throw Error('SVG dimensions');
                                          if ((getComputedStyle(details).display!=='none')!==(scale<1)) throw Error('Disclosure CSS');
                                          if (document.documentElement.scrollWidth>innerWidth+1) throw Error('Page overflow');
                                          return 'passed';
                                        }""", [image_width, image_height, formulas])
                                    if measured != "passed":
                                        raise AssertionError("Unexpected layout result")
                                    details = page.locator(".nepl-image-details")
                                    if details.is_visible():
                                        details.locator("summary").click()
                                        expect(details).to_have_attribute("open", "")
                                        original = details.locator(".nepl-image-original > img")
                                        expect(original).to_be_visible()
                                        box = original.bounding_box()
                                        if box is None or abs(box["width"]-image_width)>0.3 or abs(box["height"]-image_height)>0.3:
                                            raise AssertionError("Original dimensions")
                                        overflow: object = page.evaluate("document.documentElement.scrollWidth>innerWidth+1")  # pyright: ignore[reportAny]
                                        if overflow is not False:
                                            raise AssertionError("Expanded overflow")
                                        details.locator("summary").click()
                                        expect(details).not_to_have_attribute("open", "")
                                    _ = page.screenshot(path=str(root / f"{implementation.name}-{mode}-{theme}-{width}-{name}.png"), full_page=True)
                                    results.append({"engine": implementation.name, "mode": mode, "theme": theme, "width": width, "page": name, "result": "passed"})
                                    if name == "one":
                                        page.get_by_role("link", name=next_label, exact=True).click()
                                        expect(page).to_have_url((output / "two/index.html").as_uri()+"#n-746172676574")
                                        expect(page.locator(":target")).to_have_count(1)
                                page.get_by_role("link", name=back_label, exact=True).click()
                                expect(page).to_have_url((output / "one/index.html").as_uri())
            finally:
                browser.close()
    if len(results) != 72:
        raise AssertionError("Incomplete preferred browser matrix")
    print(json.dumps({"cases": results, "document_javascript": False, "network": False}))


if __name__ == "__main__":
    main()
