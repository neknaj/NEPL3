"""Check actual recursive Math/Sentence output in the pinned browser engines.

Input is the --nocapture log of math_sentence_math_printing_preserves_recursive_source.
This verifies DOM ownership and Ruby geometry with document JavaScript disabled.
It does not establish general Math accessibility or Playground acceptance.
"""
import argparse
import json
from pathlib import Path
from dataclasses import dataclass
from typing import Literal
import sys

from playwright.sync_api import sync_playwright

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue

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
                            a, b = base.bounding_box(), reading.bounding_box()
                            assert a and b and a["width"] > 0 and b["width"] > 0
                            assert b["y"] + b["height"] <= a["y"] + 0.5, (engine, a, b)
                            assert abs((a["x"] + a["width"] / 2)
                                       - (b["x"] + b["width"] / 2)) < 1, (engine, a, b)
                            assert page.locator("math math mn").text_content() == "7"
                            results.append(Case(engine, browser.version, width, size))
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
