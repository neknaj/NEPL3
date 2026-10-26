"""Verify external SVG source CLI in both math modes under an HTTP mount."""
from pathlib import Path
from html.parser import HTMLParser
from typing import override
import hashlib
import json
import os
import re
import subprocess
import time
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue, array, decode, integer, object_value, string
from tools.audit.doc_html.server import serve


# The exporter intentionally retains this one legacy font stylesheet. Network
# interception still blocks it; permitting its DOM reference is not fetching it.
GOOGLE_FONTS = "https://fonts.googleapis.com/css2?family=Klee+One:wght@400;600&display=swap"


class StylesheetLinks(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.hrefs: list[str] = []

    @override
    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag != "link":
            return
        values = dict(attrs)
        if len(values) != len(attrs):
            raise ValueError("Duplicate link attribute")
        rel = values.get("rel")
        if rel is not None and "stylesheet" in rel.split():
            href = values.get("href")
            if rel != "stylesheet" or href is None:
                raise ValueError("Invalid stylesheet link")
            self.hrefs.append(href)


def stylesheet_path(page: str) -> str:
    if page not in ("one", "two"):
        raise ValueError("Unknown fixture page")
    digest = hashlib.sha256(f"{page}/index.html".encode()).hexdigest()
    return f"assets/doc-{digest}.css"


def verify_stylesheets(html: str, page: str, recorded_paths: set[str]) -> None:
    relative = stylesheet_path(page)
    if f"{page}/{relative}" not in recorded_paths:
        raise ValueError("Page stylesheet is not recorded")
    parser = StylesheetLinks()
    parser.feed(html)
    parser.close()
    if sorted(parser.hrefs) != sorted([GOOGLE_FONTS, relative]):
        raise ValueError("Unexpected stylesheet references")


def verify_package(output: Path, renderer: str, mode: str, count: int) -> None:
    if renderer not in ("katex-preferred", "mathml-only") or mode not in ("Rows", "Columns", "Single") or count not in (1, 2):
        raise ValueError("Unknown fixture selection")
    receipt = object_value(decode((output / "manifest.json").read_bytes(), reject_duplicates=True, reject_nonfinite=True))
    preferred = renderer == "katex-preferred"
    version = "2" if preferred else "1"
    if string(receipt["format"]) != f"nepl3.local-doc-pages.root-svg-external-package/{version}" or string(receipt["svg"]) != "external":
        raise ValueError("External receipt mismatch")
    if string(object_value(receipt["display"])["mode"]) != mode:
        raise ValueError("Display selection mismatch")
    source = object_value(receipt["source_input"])
    if string(source["math_renderer"]) != ("katex-preferred" if preferred else "mathml-only-explicit"):
        raise ValueError("Renderer selection mismatch")
    if preferred:
        generation = object_value(receipt["katex_generation"])
        if integer(generation["successful_visual_pages"]) != 1 or integer(generation["successful_visuals"]) != count or generation["host_invocation_recorded"] is not False:
            raise ValueError("Actual native visuals mismatch")
        native = object_value(source["native_generation"])
        if string(native["contract"]) != "nepl3.local-doc-pages.svg-katex-context/1" or string(native["requested_renderer"]) != "katex-preferred" or string(native["pinned_version"]) != "0.18.7":
            raise ValueError("Native context contract mismatch")
        if native["host_invocation_recorded"] is not False or string(native["host_internal_work"]) != "unobserved" or string(native["host_internal_allocation"]) != "unobserved":
            raise ValueError("Overclaimed native accounting")
        native_pages = [object_value(page) for page in array(native["pages"])]
        if len(native_pages) != 2 or [string(page["page"]) for page in native_pages] != ["one", "two"] or [len(array(page["visuals"])) for page in native_pages] != [count, 0]:
            raise ValueError("Page-scoped native visuals mismatch")
        for page in native_pages:
            if re.fullmatch(r"[a-f0-9]{64}", string(page["request_context"])) is None:
                raise ValueError("Invalid native request context")
            visuals = [object_value(v) for v in array(page["visuals"])]
            roots = [integer(v["root"]) for v in visuals]
            scopes = [string(v["scope"]) for v in visuals]
            if any(n < 0 for n in roots) or len(set(roots)) != len(roots) or len(set(scopes)) != len(scopes) or any(re.fullmatch(r"nepl-math-[a-f0-9]{64}-k[0-9]+", scope) is None for scope in scopes):
                raise ValueError("Invalid native visual scope")
            formulas = [object_value(v) for v in array(page["formulas"])]
            if [integer(v["root"]) for v in formulas] != roots or any(not array(v["owner_path"]) or v["fallback_detail"] is not None or v["unsupported"] is not None for v in formulas):
                raise ValueError("Native formula correspondence mismatch")
            files = [object_value(v) for v in array(page["files"])]
            if visuals and not files:
                raise ValueError("Missing pinned native file metadata")
            for record in files:
                if not string(record["path"]) or not string(record["mime"]) or integer(record["bytes"]) <= 0 or re.fullmatch(r"[a-f0-9]{64}", string(record["sha256"])) is None:
                    raise ValueError("Invalid native file metadata")
    elif receipt["katex_generation"] is not False or source.get("native_generation") is not None:
        raise ValueError("Explicit MathML used native evidence")
    records = [object_value(v) for v in array(receipt["files"])]
    paths = {string(record["path"]) for record in records}
    if len(records) != 6 or len(paths) != 6:
        raise ValueError("Payload count or duplicates")
    actual: set[str] = set()
    for path in output.rglob("*"):
        if path.is_symlink():
            raise ValueError("Unexpected symlink")
        if path.is_file() and path != output / "manifest.json":
            actual.add(path.relative_to(output).as_posix())
    if actual != paths:
        raise ValueError("Unrecorded or missing payload")
    svg_count = 0
    for record in records:
        name = string(record["path"])
        if re.fullmatch(r"(?:one|two)/(?:index\.html|assets/doc-[a-f0-9]{64}\.css|assets/[a-f0-9]{64}\.svg)", name) is None:
            raise ValueError("Unexpected fixture path")
        data = (output / name).read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if len(data) != integer(record["bytes"]) or digest != string(record["sha256"]):
            raise ValueError("Payload identity mismatch")
        if name.endswith(".svg"):
            svg_count += 1
            if Path(name).stem != digest:
                raise ValueError("SVG content address mismatch")
    if svg_count != 2:
        raise ValueError("Missing external SVGs")
    for page in ("one", "two"):
        verify_stylesheets((output / page / "index.html").read_text(encoding="utf-8"), page, paths)



def prepare_cases(binary: Path, root: Path, packages: Path) -> list[tuple[str, str, Path, int, str, str]]:
    root.mkdir(parents=True, exist_ok=False)
    example = Path(__file__).resolve().parents[3] / "examples/document/pages-svg"
    selections: tuple[tuple[str, JsonValue, int, str, str], ...] = (
        ("Rows", {"mode": "rows"}, 2, "Open the wide diagram.", "Return to the first page."),
        ("Columns", {"mode": "columns"}, 2, "Open the wide diagram.", "Return to the first page."),
        ("Single", {"mode": "single", "language": "fr", "fallbacks": ["ja"]}, 1, "横長の図を開く。", "最初のページへ戻る。"),
    )
    cases: list[tuple[str, str, Path, int, str, str]] = []
    for renderer in ("katex-preferred", "mathml-only"):
        for mode, selection, count, next_label, back_label in selections:
            inputs = root / renderer / mode / "inputs"
            inputs.mkdir(parents=True)
            for name in ("one.nepld", "two.nepld", "small.svg", "wide.svg"):
                data = (example / name).read_bytes()
                if name == "one.nepld":
                    text = data.decode("utf-8")
                    marker = "  cons paragraph\n"
                    if text.count(marker) != 1:
                        raise ValueError("Inline probe insertion site changed")
                    text = text.replace(marker, marker + '    cons sentence cons image asset "small" none "Inline diagram" nil\n', 1)
                    data = text.encode("utf-8")
                _ = (inputs / name).write_bytes(data)
            manifest = dict(object_value(decode((example / "manifest.json").read_bytes())))
            manifest["display"] = selection
            _ = (inputs / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            output = inputs.parent / "output"
            command = [str(binary), "doc-html", "pages-svg", "--svg", "external"]
            if renderer == "mathml-only":
                command.extend(["--math-renderer", "mathml-only"])
            command.extend([str(inputs / "manifest.json"), str(output)])
            _ = subprocess.run(command, env=dict(os.environ, NEPL3_KATEX_NODE_MODULES=str(packages)), check=True)
            verify_package(output, renderer, mode, count)
            for page_name, svg_name in (("one", "small.svg"), ("two", "wide.svg")):
                data = (inputs / svg_name).read_bytes()
                expected = output / page_name / "assets" / (hashlib.sha256(data).hexdigest() + ".svg")
                if expected.read_bytes() != data:
                    raise ValueError("External file does not match captured source")
            cases.append((renderer, mode, output, count, next_label, back_label))
    return cases


def main() -> None:
    from playwright.sync_api import Route, expect, sync_playwright
    if len(sys.argv) != 4:
        raise ValueError("pages_svg_external_cli.py BINARY NEW_OUTPUT_DIRECTORY PACKAGES")
    binary, root, packages = (Path(arg).resolve() for arg in sys.argv[1:])
    cases = prepare_cases(binary, root, packages)
    screenshots = root / "screenshots"
    screenshots.mkdir()
    results: list[dict[str, str | int]] = []
    with serve(root, "/external-svg-cli/") as base, sync_playwright() as p:
        def network(route: Route) -> None:
            if route.request.url.startswith(base):
                route.continue_()
            else:
                route.abort()
        for implementation in (p.chromium, p.firefox, p.webkit):
            browser = implementation.launch()
            try:
                for renderer, mode, output, math_count, next_label, back_label in cases:
                    prefix = base + output.relative_to(root).as_posix() + "/"
                    preferred = renderer == "katex-preferred"
                    for theme in ("light", "dark"):
                        for width in (375, 1280):
                            with browser.new_context(java_script_enabled=False, color_scheme=theme, viewport={"width": width, "height": 900}) as context:
                                _ = context.route(re.compile(r"^https?://"), network)
                                page = context.new_page()
                                _ = page.goto(prefix + "one/index.html")
                                for name, image_width, image_height, formulas in (("one", 100, 60, math_count), ("two", 600, 200, 0)):
                                    deadline = time.monotonic() + 15
                                    while time.monotonic() < deadline:
                                        loaded: object = page.evaluate("document.fonts.status === 'loaded'")  # pyright: ignore[reportAny]
                                        if loaded is True:
                                            break
                                        time.sleep(0.05)
                                    else:
                                        raise AssertionError("font deadline")
                                    expect(page.locator("math")).to_have_count(formulas)
                                    expect(page.locator(".katex-html")).to_have_count(formulas if preferred else 0)
                                    expect(page.locator(".nepl-image-group")).to_have_count(1)
                                    measured: object = page.evaluate(  # pyright: ignore[reportAny]
                                        """([width,height,count,preferred,name,prefix,fontCss,localCss]) => {
                                        const csp=document.querySelector('meta[http-equiv="Content-Security-Policy"]').content;
                                        if (!csp.includes("img-src 'self';") || csp.includes('font-src data:') !== (preferred && count>0)) throw Error('CSP mismatch');
                                        if (document.scripts.length || document.querySelector('[style]')) throw Error('Active content');
                                        const styles=[...document.querySelectorAll('link[rel="stylesheet"]')].map(link=>link.href);
                                        if (styles.length!==2 || styles.filter(href=>href===new URL(fontCss).href).length!==1 || styles.filter(href=>href===localCss).length!==1 || !localCss.startsWith(prefix)) throw Error('Unexpected stylesheet references');
                                        for (const image of document.querySelectorAll('img')) {
                                            if (!image.src.startsWith(prefix) || !image.complete || image.naturalWidth!==width || image.naturalHeight!==height) throw Error('External SVG not loaded from its package');
                                        }
                                        const inline=[...document.querySelectorAll('img')].filter(i=>!i.closest('.nepl-image-group'));
                                        if (inline.length!==(name==='one'?1:0) || (inline.length && inline[0].alt!=='Inline diagram')) throw Error('Inline SVG lost');
                                        for (const math of document.querySelectorAll('math')) {
                                            if (math.closest('[aria-hidden="true"]')) throw Error('MathML hidden from accessibility');
                                            const owner=math.closest('.nepl-math-accessible');
                                            if (preferred) { if (!owner || getComputedStyle(owner).position!=='absolute' || getComputedStyle(owner).width!=='1px') throw Error('Independent MathML CSS absent'); }
                                            else { const box=math.getBoundingClientRect(); if (owner || box.width<=0 || box.height<=0) throw Error('Explicit MathML not visible'); }
                                        }
                                        for (const visual of document.querySelectorAll('.katex')) { const box=visual.getBoundingClientRect(); if (!visual.closest('[aria-hidden="true"]') || box.width<=0 || box.height<=0) throw Error('KaTeX visual absent'); }
                                        if (preferred && count>0 && (!document.fonts.check('16px KaTeX_Main') || ![...document.fonts].some(f=>f.family.startsWith('KaTeX_')&&f.status==='loaded'))) throw Error('Native fonts absent');
                                        const group=document.querySelector('.nepl-image-group'), image=group.querySelector('.nepl-image-preview > img'), details=group.querySelector(':scope > details');
                                        const box=image.getBoundingClientRect(), scale=Math.min(1,group.clientWidth/width,360/height);
                                        if (Math.abs(box.width-width*scale)>0.3 || Math.abs(box.height-height*scale)>0.3) throw Error('Preview size');
                                        if ((getComputedStyle(details).display!=='none')!==(scale<1)) throw Error('Disclosure CSS');
                                        if (document.documentElement.scrollWidth>innerWidth+1) throw Error('Overflow');
                                        return 'passed';
                                        }""", [image_width, image_height, formulas, preferred, name, prefix, GOOGLE_FONTS, prefix + name + "/" + stylesheet_path(name)])
                                    if measured != "passed":
                                        raise AssertionError("Unexpected measurements")
                                    details = page.locator(".nepl-image-details")
                                    if details.is_visible():
                                        details.locator("summary").click()
                                        expect(details).to_have_attribute("open", "")
                                        box = details.locator(".nepl-image-original > img").bounding_box()
                                        if box is None or abs(box["width"]-image_width)>0.3 or abs(box["height"]-image_height)>0.3:
                                            raise AssertionError("Original size")
                                        overflow: object = page.evaluate("document.documentElement.scrollWidth>innerWidth+1")  # pyright: ignore[reportAny]
                                        if overflow is not False:
                                            raise AssertionError("Expanded overflow")
                                        details.locator("summary").click()
                                        expect(details).not_to_have_attribute("open", "")
                                    snapshot = screenshots / f"{implementation.name}-{renderer}-{mode}-{theme}-{width}-{name}.png"
                                    png = page.screenshot(path=str(snapshot), full_page=True)
                                    results.append({"engine": implementation.name, "renderer": renderer, "mode": mode, "theme": theme, "width": width, "page": name, "result": "passed", "screenshot": snapshot.name, "sha256": hashlib.sha256(png).hexdigest()})
                                    if name == "one":
                                        page.get_by_role("link", name=next_label, exact=True).click()
                                        expect(page).to_have_url(prefix + "two/index.html#n-746172676574")
                                        expect(page.locator(":target")).to_have_count(1)
                                page.get_by_role("link", name=back_label, exact=True).click()
                                expect(page).to_have_url(prefix + "one/index.html")
            finally:
                browser.close()
    if len(results) != 144:
        raise AssertionError("Incomplete external CLI browser matrix")
    print(json.dumps({"cases": results, "document_javascript": False, "network": "localhost fixture mount only; image/CSS package prefixes checked"}))


if __name__ == "__main__":
    main()
