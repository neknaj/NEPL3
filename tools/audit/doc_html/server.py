"""Loopback-only, non-root mounted server for generated browser QA fixtures."""
from collections.abc import Iterator
from contextlib import contextmanager
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from socket import socket
from socketserver import BaseServer
from typing import final, override
from urllib.parse import urlsplit
import threading


@contextmanager
def serve(root: Path, mount: str) -> Iterator[str]:
    if not mount.startswith("/") or not mount.endswith("/") or "//" in mount or any(p in (".", "..") for p in mount.split("/")):
        raise ValueError("Expected a non-traversing absolute mount")
    if mount == "/":
        raise ValueError("This fixture requires a non-root mount")

    @final
    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, request: socket, client_address: tuple[str, int], server: BaseServer) -> None:
            super().__init__(request, client_address, server, directory=str(root))

        def mounted_path(self) -> bool:
            path = urlsplit(self.path).path
            if not path.startswith(mount):
                self.send_error(404)
                return False
            self.path = "/" + path[len(mount):]
            return True

        @override
        def do_GET(self) -> None:
            if self.mounted_path():
                super().do_GET()

        @override
        def do_HEAD(self) -> None:
            if self.mounted_path():
                super().do_HEAD()

        @override
        def log_message(self, format: str, *args: object) -> None:
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}{mount}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
