"""Check actual recursive Math/Sentence output in the pinned browser engines.

Input is the --nocapture log of math_sentence_math_printing_preserves_recursive_source.
This verifies DOM ownership and Ruby geometry with document JavaScript disabled.
It does not establish general Math accessibility or Playground acceptance.
"""
import argparse
import json
from pathlib import Path
from dataclasses import dataclass
from typing import Literal, Protocol
import sys

from playwright.sync_api import sync_playwright

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue, decode, object_value

type Engine = Literal['chromium', 'firefox', 'webkit']


@dataclass(frozen=True, slots=True)
class Case:
    engine: Engine
    version: str
    width: int
    font_size: int

    def representation(self) -> dict[str, JsonValue]:
        return dict(engine=self.engine, version=self.version, width=self.width, font_size=self.font_size)


@dataclass(frozen=True, slots=True)
class Report:
    cases: tuple[Case, ...]

    def representation(self) -> dict[str, JsonValue]:
        return dict(passed=len(self.cases), cases=[case.representation() for case in self.cases])


def fragment_from(corpus: str) -> str:
    marker = "MATH_RECURSIVE_HTML "
    records = [line.split(marker, 1)[1].strip()
               for line in corpus.splitlines()
               if marker in line]
    if len(records) != 1:
        raise ValueError("expected one recursive Math HTML record")
    return bytes.fromhex(records[0]).decode("utf-8")



@dataclass(frozen=True, slots=True)
class Rect:
    x: float
    y: float
    width: float
    height: float

    @staticmethod
    def read(value: JsonValue) -> "Rect":
        fields = object_value(value)
        def number(name: str) -> float:
            item = fields.get(name)
            if isinstance(item, bool) or not isinstance(item, (int, float)):
                raise ValueError(f"invalid rectangle {name}")
            try:
                result = float(item)
            except OverflowError as error:
                raise ValueError(f"nonfinite rectangle {name}") from error
            if not -float("inf") < result < float("inf"):
                raise ValueError(f"nonfinite rectangle {name}")
            return result
        return Rect(number("x"), number("y"), number("width"), number("height"))


def check_geometry(base: Rect, reading: Rect) -> None:
    for box in (base, reading):
        if not all(-float("inf") < v < float("inf") for v in (box.x, box.y, box.width, box.height)):
            raise ValueError("nonfinite rectangle")
        if box.width <= 0 or box.height <= 0:
            raise ValueError("empty rectangle")
    # Preserve the existing separation and centering tolerances unchanged.
    if not reading.y + reading.height <= base.y + 0.5:
        raise AssertionError(("annotation overlap", base, reading))
    if not abs((base.x + base.width / 2) - (reading.x + reading.width / 2)) < 1:
        raise AssertionError(("annotation center", base, reading))


MEASURE = """() => {
  const ruby = document.querySelector('math math munder mtext .nepl-ruby');
  if (!ruby) throw Error('missing recursive Ruby');
  const measure = node => {
    if (!node) throw Error('missing annotation node');
    const rect = node.getBoundingClientRect(), style = getComputedStyle(node);
    return {rect: {x:rect.x, y:rect.y, width:rect.width, height:rect.height},
      style: {display:style.display, fontFamily:style.fontFamily,
        fontSize:style.fontSize, lineHeight:style.lineHeight,
        paddingTop:style.paddingTop, paddingBottom:style.paddingBottom,
        verticalAlign:style.verticalAlign, borderCollapse:style.borderCollapse,
        baselineSource:style.getPropertyValue('baseline-source'),
        mathDepth:style.getPropertyValue('math-depth'),
        mathStyle:style.getPropertyValue('math-style')}};
  };
  // Both boxes are sampled in one browser task, without an intervening Playwright call.
  return JSON.stringify({base:measure(ruby.querySelector(':scope > .nepl-base')),
    reading:measure(ruby.querySelector(':scope > .nepl-reading')),
    ruby:measure(ruby), mtext:measure(ruby.closest('mtext')),
    fonts:document.fonts.status,
    baselineSourceSupported:CSS.supports('baseline-source','first') && CSS.supports('baseline-source','last')});
}"""


class GeometryPage(Protocol):
    def evaluate(self, expression: str) -> object: ...


def observe(page: GeometryPage) -> tuple[Rect, Rect, JsonValue]:
    raw = page.evaluate(MEASURE)
    if not isinstance(raw, str):
        raise ValueError("expected JSON geometry observation")
    value = decode(raw, reject_duplicates=True, reject_nonfinite=True)
    fields = object_value(value)
    base = Rect.read(object_value(fields.get("base")).get("rect"))
    reading = Rect.read(object_value(fields.get("reading")).get("rect"))
    return base, reading, value

def run(corpus: Path, css: Path) -> Report:
    fragment = fragment_from(corpus.read_text(encoding="utf-8-sig"))
    stylesheet = css.read_text(encoding="utf-8")
    results: list[Case] = []
    with sync_playwright() as p:
        engines: tuple[Engine, ...] = ('chromium', 'firefox', 'webkit')
        for engine, implementation in zip(engines, (p.chromium, p.firefox, p.webkit), strict=True):
            browser = implementation.launch()
            try:
                for width in [375, 1280]:
                    context = browser.new_context(java_script_enabled=False,
                                                  viewport={"width": width, "height": 900})
                    try:
                        page = context.new_page()
                        for size in [16, 32]:
                            page.set_content('<!doctype html><meta charset="utf-8"><style>'
                                             + stylesheet + f".nepl-doc{{font-size:{size}px}}"
                                             + '</style><article class="nepl-doc">' + fragment + '</article>')
                            assert page.locator("math").count() == 2
                            assert page.locator("math > munder").count() == 2
                            assert page.locator(".nepl-ruby").count() == 1
                            assert page.locator("script").count() == 0
                            ruby = page.locator("math math munder mtext .nepl-ruby")
                            assert ruby.count() == 1
                            base = ruby.locator(":scope > .nepl-base")
                            reading = ruby.locator(":scope > .nepl-reading")
                            assert base.inner_text() == "字"
                            assert reading.inner_text() == "じ"
                            case = Case(engine, browser.version, width, size)
                            a, b, observation = observe(page)
                            try:
                                check_geometry(a, b)
                            except (AssertionError, ValueError) as error:
                                raise AssertionError(dict(case=case.representation(), observation=observation)) from error
                            assert page.locator("math math mn").text_content() == "7"
                            results.append(case)
                    finally:
                        context.close()
            finally:
                browser.close()
    return Report(tuple(results))


class Arguments(argparse.Namespace):
    corpus: Path = Path()
    css: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("--corpus", type=Path, required=True)
    _ = parser.add_argument("--css", type=Path, required=True)
    args = parser.parse_args(namespace=Arguments())
    print(json.dumps(run(args.corpus, args.css).representation()))


if __name__ == "__main__":
    main()
