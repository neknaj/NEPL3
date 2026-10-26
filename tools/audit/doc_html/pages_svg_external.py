"""Verify native external SVG PageSet packages under a non-root HTTP mount in CI."""
from pathlib import Path
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from socket import socket
from socketserver import BaseServer
from typing import final, override
from urllib.parse import urlsplit
import threading
import hashlib
import json
import re
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import decode, object_value, array, string, integer


def materialize(corpus: bytes, root: Path) -> list[tuple[str, Path]]:
    if len(corpus) > 32 * 1024 * 1024:
        raise ValueError("Corpus byte limit")
    root.mkdir(parents=True, exist_ok=False)
    modes: set[str] = set()
    packages: list[tuple[str, Path]] = []
    for line in corpus.decode("utf-8").splitlines():
        line = re.sub(r"^test [A-Za-z0-9_:]+ \.\.\. (?=NEPL3_PAGES_EXTERNAL_SVG_CORPUS )", "", line)
        if not line.startswith("NEPL3_PAGES_EXTERNAL_SVG_CORPUS "):
            continue
        case = object_value(decode(line.partition("NEPL3_PAGES_EXTERNAL_SVG_CORPUS ")[2], reject_duplicates=True, reject_nonfinite=True))
        if set(case) != {"mode", "files", "manifest"}:
            raise ValueError("Unexpected corpus fields")
        mode = string(case["mode"])
        if mode not in ("Rows", "Columns", "Single") or mode in modes:
            raise ValueError("Unexpected or duplicate display mode")
        modes.add(mode)
        destination = root / mode
        manifest = string(case["manifest"])
        receipt = object_value(decode(manifest, reject_duplicates=True, reject_nonfinite=True))
        if string(receipt["format"]) != "nepl3.local-doc-pages.root-svg-external-package/1" or string(receipt["svg"]) != "external" or string(object_value(receipt["display"])["mode"]) != mode:
            raise ValueError("Receipt format/display mismatch")
        entries = [object_value(entry) for entry in array(receipt["files"])]
        expected = {string(entry["path"]): entry for entry in entries}
        files = object_value(case["files"])
        if len(entries) != 6 or len(files) != 6 or set(files) != set(expected):
            raise ValueError("Corpus file set mismatch")
        total = 0
        for name, content in files.items():
            parts = name.split("/")
            if any(part in ("", ".", "..") for part in parts) or "\\" in name or ":" in name:
                raise ValueError("Unsafe corpus path")
            if re.fullmatch(r"deep/(?:[ab]\.html|assets/doc-[a-f0-9]{64}\.css|assets/[a-f0-9]{64}\.svg)", name) is None:
                raise ValueError("Unexpected fixture path")
            data = string(content).encode("utf-8")
            total += len(data)
            if total > 8 * 1024 * 1024:
                raise ValueError("Package byte limit")
            if hashlib.sha256(data).hexdigest() != string(expected[name]["sha256"]) or len(data) != integer(expected[name]["bytes"]):
                raise ValueError("Corpus digest mismatch")
            if name.endswith(".svg") and Path(name).stem != hashlib.sha256(data).hexdigest():
                raise ValueError("SVG content-address mismatch")
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True)
            _ = path.write_bytes(data)
        _ = (destination / "manifest.json").write_text(manifest, encoding="utf-8")
        packages.append((mode, destination))
    if modes != {"Rows", "Columns", "Single"}:
        raise ValueError("Missing native packages")
    return packages


def main() -> None:
    from playwright.sync_api import Route, expect, sync_playwright

    if len(sys.argv) != 3:
        raise ValueError("pages_svg_external.py CORPUS_LOG NEW_OUTPUT_DIRECTORY")
    packages = materialize(Path(sys.argv[1]).read_bytes(), Path(sys.argv[2]).resolve())
    root = Path(sys.argv[2]).resolve()
    mount = "/mounted-docs/"
    @final
    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, request: socket, client_address: tuple[str, int], server: BaseServer) -> None:
            super().__init__(request, client_address, server, directory=str(root))

        @override
        def do_GET(self) -> None:
            path = urlsplit(self.path).path
            if not path.startswith(mount):
                self.send_error(404)
                return
            self.path = "/" + path[len(mount):]
            super().do_GET()

        @override
        def log_message(self, format: str, *args: object) -> None:
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{server.server_port}{mount}"
    def block_network(route: Route) -> None:
        if route.request.url.startswith(base):
            route.continue_()
        else:
            route.abort()

    screenshots = root / "screenshots"
    screenshots.mkdir()
    results: list[dict[str, str | int]] = []
    try:
        with sync_playwright() as p:
            for implementation in (p.chromium, p.firefox, p.webkit):
                browser = implementation.launch()
                try:
                    for mode, _output in packages:
                        for theme in ("light", "dark"):
                            for width in (375, 1280):
                                with browser.new_context(java_script_enabled=False, color_scheme=theme,
                                                         viewport={"width": width, "height": 900}) as context:
                                    _ = context.route(re.compile(r"^https?://"), block_network)
                                    page = context.new_page()
                                    _ = page.goto(f"{base}{mode}/deep/a.html")
                                    for name, intrinsic_width, height, groups in (("a", 100, 50, 1), ("b", 400, 80, 1)):
                                        expect(page.locator(".nepl-image-group")).to_have_count(groups)
                                        expect(page.locator("script")).to_have_count(0)
                                        measurements: object = page.evaluate(  # pyright: ignore[reportAny]
                                            """([width, height, name]) => {
                                          const csp = document.querySelector('meta[http-equiv="Content-Security-Policy"]').content;
                                          if (!csp.includes("img-src 'self';")) throw Error('Missing external-image CSP');
                                          for (const img of document.querySelectorAll('img')) {
                                            const url = new URL(img.src, document.baseURI);
                                            if (url.origin !== location.origin || !url.pathname.startsWith('/mounted-docs/') || !img.complete || img.naturalWidth === 0) throw Error('External image not loaded from mount');
                                          }
                                          const inline = [...document.querySelectorAll('img')].filter(img => !img.closest('.nepl-image-group'));
                                      if (inline.length !== (name === 'a' ? 1 : 0)) throw Error('Missing or unexpected inline SVG');
                                      if (name === 'a' && (inline[0].alt !== '漢' || inline[0].naturalWidth !== width || inline[0].naturalHeight !== height)) throw Error('Wrong inline SVG alt or intrinsic dimensions');
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
                                        }""", [intrinsic_width, height, name])
                                        if measurements != "passed":
                                            raise AssertionError("Unexpected measurements")
                                        for details in page.locator(".nepl-image-group > details").all():
                                            if details.is_visible():
                                                details.locator("summary").click()
                                                if details.get_attribute("open") is None:
                                                    raise AssertionError("Disclosure did not open")
                                                details.locator("summary").click()
                                                if details.get_attribute("open") is not None:
                                                    raise AssertionError("Disclosure did not close")
                                        snapshot = screenshots / f"{implementation.name}-{mode}-{theme}-{width}-{name}.png"
                                        png = page.screenshot(path=str(snapshot), full_page=True)
                                        results.append({"engine": implementation.name, "mode": mode, "theme": theme,
                                                        "width": width, "page": name, "result": "passed", "screenshot": snapshot.name, "sha256": hashlib.sha256(png).hexdigest()})
                                        if name == "a":
                                            page.get_by_role("link", name="Next", exact=True).click()
                                            expect(page).to_have_url(f"{base}{mode}/deep/b.html" + "#n-6775657374")
                                            expect(page.locator(":target")).to_have_count(1)
                                    page.get_by_role("link", name="Back", exact=True).click()
                                    expect(page).to_have_url(f"{base}{mode}/deep/a.html" + "#n-636170")
                                    expect(page.locator(":target")).to_have_count(1)
                finally:
                    browser.close()
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
    if len(results) != 72:
        raise AssertionError("Incomplete browser matrix")
    print(json.dumps({"cases": results}))


if __name__ == "__main__":
    main()
