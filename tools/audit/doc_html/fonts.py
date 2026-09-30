"""Bounded host observations of native font loading with page scripts disabled."""
import time
from typing import Protocol


class FontPage(Protocol):
    def evaluate(self, expression: str) -> object: ...


FONT_LOADED = """() => {
  const faces = [...document.fonts].filter(face => face.family.replace(/["']/g, '') === 'Klee One');
  return document.fonts.status === 'loaded' && [400, 600].every(weight =>
    faces.some(face => face.status === 'loaded' && face.weight === String(weight)));
}"""


def wait_for_fonts(page: FontPage) -> None:
    # Native font loading continues with document JavaScript disabled. Use a
    # host deadline and synchronous observations; page timers may be disabled.
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        loaded: object = page.evaluate(FONT_LOADED)
        if loaded is True:
            return
        time.sleep(0.1)
    raise AssertionError("Google Fonts Klee One 400/600 did not load within 30 seconds")
