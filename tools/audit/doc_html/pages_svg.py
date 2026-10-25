"""Verify actual native PageSet SVG packages in the permitted CI browser environment."""
from pathlib import Path
import hashlib
import json
import re
import sys
from playwright.sync_api import Route, expect, sync_playwright
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import decode, object_value, array, string, integer


def block_network(route: Route) -> None:
    route.abort()


def materialize(corpus: bytes, root: Path) -> list[tuple[str, Path]]:
    if len(corpus) > 32 * 1024 * 1024:
        raise ValueError("Corpus byte limit")
    root.mkdir(parents=True, exist_ok=False)
    modes: set[str] = set()
    packages: list[tuple[str, Path]] = []
    for line in corpus.decode("utf-8").splitlines():
        line = re.sub(r"^test [A-Za-z0-9_:]+ \.\.\. (?=NEPL3_PAGES_SVG_CORPUS )", "", line)
        if not line.startswith("NEPL3_PAGES_SVG_CORPUS "):
            continue
        case = object_value(decode(line.partition("NEPL3_PAGES_SVG_CORPUS ")[2], reject_duplicates=True, reject_nonfinite=True))
        if set(case) != {"mode", "files", "manifest"}:
            raise ValueError("Unexpected corpus fields")
        mode = string(case["mode"])
        if mode not in ("Rows", "Columns", "Single") or mode in modes:
            raise ValueError("Unexpected or duplicate display mode")
        modes.add(mode)
        destination = root / mode
        manifest = string(case["manifest"])
        receipt = object_value(decode(manifest, reject_duplicates=True, reject_nonfinite=True))
        if string(receipt["format"]) != "nepl3.local-doc-pages.root-svg-package/1" or string(object_value(receipt["display"])["mode"]) != mode:
            raise ValueError("Receipt format/display mismatch")
        entries = [object_value(entry) for entry in array(receipt["files"])]
        expected = {string(entry["path"]): entry for entry in entries}
        files = object_value(case["files"])
        if len(entries) != 4 or len(files) != 4 or set(files) != set(expected):
            raise ValueError("Corpus file set mismatch")
        total = 0
        for name, content in files.items():
            parts = name.split("/")
            if any(part in ("", ".", "..") for part in parts) or "\\" in name or ":" in name:
                raise ValueError("Unsafe corpus path")
            if re.fullmatch(r"deep/(?:[ab]\.html|assets/doc-[a-f0-9]{64}\.css)", name) is None:
                raise ValueError("Unexpected fixture path")
            data = string(content).encode("utf-8")
            total += len(data)
            if total > 8 * 1024 * 1024:
                raise ValueError("Package byte limit")
            if hashlib.sha256(data).hexdigest() != string(expected[name]["sha256"]) or len(data) != integer(expected[name]["bytes"]):
                raise ValueError("Corpus digest mismatch")
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True)
            _ = path.write_bytes(data)
        _ = (destination / "manifest.json").write_text(manifest, encoding="utf-8")
        packages.append((mode, destination))
    if modes != {"Rows", "Columns", "Single"}:
        raise ValueError("Missing native packages")
    return packages


def main() -> None:
    if len(sys.argv) != 3:
        raise ValueError("pages_svg.py CORPUS_LOG NEW_OUTPUT_DIRECTORY")
    packages = materialize(Path(sys.argv[1]).read_bytes(), Path(sys.argv[2]).resolve())
    results: list[dict[str, str | int]] = []
    with sync_playwright() as p:
        for implementation in (p.chromium, p.firefox, p.webkit):
            browser = implementation.launch()
            try:
                for mode, output in packages:
                    for theme in ("light", "dark"):
                        for width in (375, 1280):
                            with browser.new_context(java_script_enabled=False, color_scheme=theme,
                                                     viewport={"width": width, "height": 900}) as context:
                                _ = context.route(re.compile(r"^https?://"), block_network)
                                page = context.new_page()
                                _ = page.goto((output / "deep/a.html").as_uri())
                                for name, intrinsic_width, height, groups in (("a", 100, 50, 1), ("b", 400, 80, 1)):
                                    expect(page.locator(".nepl-image-group")).to_have_count(groups)
                                    expect(page.locator("script")).to_have_count(0)
                                    measurements: object = page.evaluate("""([width, height]) => {
                                      const csp = document.querySelector('meta[http-equiv="Content-Security-Policy"]').content;
                                      if (!csp.includes('img-src data:;')) throw Error('Missing embedded-image CSP');
                                      for (const group of document.querySelectorAll('.nepl-image-group')) {
                                        const image = group.querySelector('.nepl-image-preview > img');
                                        const details = group.querySelector(':scope > details');
                                        if (!image.complete || image.naturalWidth !== width || image.naturalHeight !== height) throw Error('Image not loaded');
                                        const box = image.getBoundingClientRect();
                                        const scale = Math.min(1, group.clientWidth / width, 360 / height);
                                        if (Math.abs(box.width-width*scale)>0.3 || Math.abs(box.height-height*scale)>0.3) throw Error('Wrong image size');
                                        const shown = getComputedStyle(details).display !== 'none';
                                        if (shown !== (box.width + 0.1 < width || box.height + 0.1 < height)) throw Error('Wrong page-specific disclosure CSS');
                                        if (shown) {
                                          details.open = true;
                                          const full = details.querySelector('.nepl-image-original > img').getBoundingClientRect();
                                          if (Math.abs(full.width-width)>0.3 || Math.abs(full.height-height)>0.3) throw Error('Wrong original size');
                                          details.open = false;
                                        }
                                      }
                                      if (!document.querySelector('math')) throw Error('Missing MathML');
                                      if (document.documentElement.scrollWidth > innerWidth + 1) throw Error('Horizontal overflow');
                                      return 'passed';
                                    }""", [intrinsic_width, height])  # pyright: ignore[reportAny]
                                    if measurements != "passed":
                                        raise AssertionError("Unexpected measurements")
                                    results.append({"engine": implementation.name, "mode": mode, "theme": theme,
                                                    "width": width, "page": name, "result": "passed"})
                                    if name == "a":
                                        page.get_by_role("link", name="Next", exact=True).click()
                                        expect(page).to_have_url((output / "deep/b.html").as_uri() + "#n-6775657374")
                                        expect(page.locator(":target")).to_have_count(1)
                                page.get_by_role("link", name="Back", exact=True).click()
                                expect(page).to_have_url((output / "deep/a.html").as_uri() + "#n-636170")
                                expect(page.locator(":target")).to_have_count(1)
            finally:
                browser.close()
    if len(results) != 72:
        raise AssertionError("Incomplete browser matrix")
    print(json.dumps({"cases": results}))


if __name__ == "__main__":
    main()
