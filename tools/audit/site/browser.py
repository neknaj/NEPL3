"""Scriptless browser observations of the audited static site."""
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from socket import socket
from socketserver import BaseServer
import threading
from typing import Literal, final, override
from urllib.parse import urlsplit

from playwright.sync_api import Request, Response, sync_playwright
from tools.serialization.json import JsonValue, decode, integer, object_value, string

type Engine = Literal['chromium', 'firefox', 'webkit']

MEASURE = '''()=>JSON.stringify({scripts:document.scripts.length,
styles:document.styleSheets.length,
overflow:document.documentElement.scrollWidth>innerWidth,title:document.title})'''


@dataclass(frozen=True, slots=True)
class Measurement:
    scripts: int
    styles: int
    overflow: bool
    title: str


def measurement(value: object) -> Measurement:
    """Narrow Playwright's untyped JavaScript result at the adapter boundary."""
    if not isinstance(value, str):
        raise ValueError('browser measurement must be JSON text')
    record = object_value(decode(value, reject_duplicates=True, reject_nonfinite=True))
    overflow = record['overflow']
    if not isinstance(overflow, bool):
        raise ValueError('browser overflow must be boolean')
    return Measurement(integer(record['scripts']), integer(record['styles']),
                       overflow, string(record['title']))


@dataclass(frozen=True, slots=True)
class Case:
    engine: Engine
    version: str
    width: int
    page: str
    state: Measurement

    def representation(self) -> dict[str, JsonValue]:
        return dict(engine=self.engine, version=self.version, width=self.width, page=self.page,
                    scripts=self.state.scripts, styles=self.state.styles,
                    overflow=self.state.overflow, title=self.state.title)


def observe(root: Path, base: str, pages: Sequence[str],
            example_sources: Sequence[str]) -> tuple[Case, ...]:
    @final
    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, request: socket, client_address: tuple[str, int], server: BaseServer) -> None:
            super().__init__(request, client_address, server, directory=str(root))

        @override
        def do_GET(self) -> None:
            path = urlsplit(self.path).path
            if not path.startswith(base):
                self.send_error(404)
                return
            self.path = '/' + path[len(base):]
            super().do_GET()

        @override
        def log_message(self, format: str, *args: object) -> None:
            pass

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    rows: list[Case] = []
    try:
        with sync_playwright() as playwright:
            engines: tuple[Engine, ...] = ('chromium', 'firefox', 'webkit')
            implementations = (playwright.chromium, playwright.firefox, playwright.webkit)
            for engine, implementation in zip(engines, implementations, strict=True):
                browser = implementation.launch()
                try:
                    for width in (375, 1280):
                        context = browser.new_context(java_script_enabled=False,
                                                      viewport={'width': width, 'height': 900})
                        try:
                            page = context.new_page()
                            failures: list[str] = []

                            def failed(request: Request) -> None:
                                failures.append(request.url)

                            def responded(response: Response) -> None:
                                if response.status >= 400:
                                    failures.append(response.url)

                            page.on('requestfailed', failed)
                            page.on('response', responded)
                            for name in pages:
                                response = page.goto(f'http://127.0.0.1:{server.server_port}' + base + name,
                                                     wait_until='networkidle')
                                raw: object = page.evaluate(MEASURE)  # pyright: ignore[reportAny]
                                state = measurement(raw)
                                assert response is not None and response.status == 200 and not failures \
                                    and state.scripts == 0 and state.styles > 0 and not state.overflow, \
                                    (name, state, failures)
                                if name == 'examples/index.html':
                                    assert page.locator('pre > code').all_text_contents() == list(example_sources), \
                                        'example display differs from source'
                                rows.append(Case(engine, browser.version, width, name, state))
                        finally:
                            context.close()
                finally:
                    browser.close()
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
    return tuple(rows)
