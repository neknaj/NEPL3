"""Exercise the explicit MathML-only SVG PageSet CLI and its shipped example."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import decode, object_value, array, string, integer


def main() -> None:
    from playwright.sync_api import Route, expect, sync_playwright

    def block_network(route: Route) -> None:
        route.abort()

    if len(sys.argv) != 3:
        raise ValueError("pages_svg_cli.py BINARY NEW_OUTPUT_DIRECTORY")
    binary = Path(sys.argv[1]).resolve()
    root = Path(sys.argv[2]).resolve()
    root.mkdir(parents=True, exist_ok=False)
    repo = Path(__file__).resolve().parents[3]
    manifest = repo / "examples/document/pages-svg/manifest.json"
    output = root / "output"
    _ = subprocess.run([str(binary), "doc-html", "pages-svg", "--math-renderer", "mathml-only", str(manifest), str(output)], check=True)
    receipt = object_value(decode((output / "manifest.json").read_bytes(), reject_duplicates=True, reject_nonfinite=True))
    if receipt["katex_generation"] is not False or string(object_value(receipt["source_input"])["math_renderer"]) != "mathml-only-explicit":
        raise AssertionError("Wrong explicit renderer provenance")
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
            raise AssertionError("Payload identity mismatch")
    results: list[dict[str, str | int]] = []
    with sync_playwright() as p:
        for implementation in (p.chromium, p.firefox, p.webkit):
            browser = implementation.launch()
            try:
                for theme in ("light", "dark"):
                    for width in (375, 1280):
                        with browser.new_context(java_script_enabled=False, color_scheme=theme, viewport={"width": width, "height": 900}) as context:
                            _ = context.route(re.compile(r"^https?://"), block_network)
                            page = context.new_page()
                            _ = page.goto((output / "one/index.html").as_uri())
                            for name, image_width, image_height in (("one", 100, 60), ("two", 600, 200)):
                                expect(page.locator(".nepl-image-group")).to_have_count(1)
                                expect(page.locator("script")).to_have_count(0)
                                if name == "one":
                                    expect(page.locator("math")).to_have_count(2)
                                measured: object = page.evaluate(  # pyright: ignore[reportAny]
                                    """([width,height]) => {
                                      const image = document.querySelector('.nepl-image-preview > img');
                                      const group = document.querySelector('.nepl-image-group');
                                      const details = group.querySelector(':scope > details');
                                      const csp = document.querySelector('meta[http-equiv="Content-Security-Policy"]').content;
                                      if (!csp.includes('img-src data:;')) throw Error('Wrong CSP');
                                      if (!image.complete || image.naturalWidth !== width || image.naturalHeight !== height) throw Error('Image unavailable');
                                      const box = image.getBoundingClientRect();
                                      const scale = Math.min(1,group.clientWidth/width,360/height);
                                      if (Math.abs(box.width-width*scale)>0.3 || Math.abs(box.height-height*scale)>0.3) throw Error('Wrong image dimensions');
                                      if ((getComputedStyle(details).display !== 'none') !== (scale < 1)) throw Error('Wrong disclosure');
                                      if (document.documentElement.scrollWidth > innerWidth+1) throw Error('Horizontal overflow');
                                      return 'passed';
                                    }""", [image_width, image_height])
                                if measured != "passed":
                                    raise AssertionError("Unexpected measurement")
                                details = page.locator(".nepl-image-details")
                                if details.is_visible():
                                    details.locator("summary").click()
                                    expect(details).to_have_attribute("open", "")
                                    original = details.locator(".nepl-image-original > img")
                                    expect(original).to_be_visible()
                                    box = original.bounding_box()
                                    if box is None or abs(box["width"] - image_width) > 0.3 or abs(box["height"] - image_height) > 0.3:
                                        raise AssertionError("Wrong expanded original size")
                                    overflow: object = page.evaluate("document.documentElement.scrollWidth > innerWidth + 1")  # pyright: ignore[reportAny]
                                    if overflow is not False:
                                        raise AssertionError("Expanded image caused page overflow")
                                    details.locator("summary").click()
                                    expect(details).not_to_have_attribute("open", "")
                                results.append({"engine": implementation.name, "theme": theme, "width": width, "page": name, "result": "passed"})
                                if name == "one":
                                    page.get_by_role("link", name="Open the wide diagram.", exact=True).click()
                                    expect(page).to_have_url((output / "two/index.html").as_uri()+"#n-746172676574")
                                    expect(page.locator(":target")).to_have_count(1)
                            page.get_by_role("link", name="Return to the first page.", exact=True).click()
                            expect(page).to_have_url((output / "one/index.html").as_uri())
            finally:
                browser.close()
    if len(results) != 24:
        raise AssertionError("Incomplete CLI browser matrix")
    print(json.dumps({"cases": results}))


if __name__ == "__main__":
    main()
