"""Typed observations of Doc annotation layout; JSON is an external boundary."""
from dataclasses import dataclass
from typing import Literal, assert_never

from tools.serialization.json import JsonValue, array, decode, integer, object_value, string

type CaseName = Literal['ruby', 'anno', 'anno-ruby', 'ruby-anno', 'ruby-ruby', 'table-ruby',
                        'list-ruby', 'ruby-multiline', 'anno-multiline', 'reading-ruby',
                        'notes-ruby', 'anno-anno', 'line-reservation']
type Engine = Literal['chromium', 'firefox', 'webkit']
type LineHeight = Literal['normal', '1.2', '2']


def case_name(value: str) -> CaseName:
    match value:
        case ('ruby' | 'anno' | 'anno-ruby' | 'ruby-anno' | 'ruby-ruby' | 'table-ruby' |
              'list-ruby' | 'ruby-multiline' | 'anno-multiline' | 'reading-ruby' |
              'notes-ruby' | 'anno-anno' | 'line-reservation'):
            return value
        case _:
            raise ValueError('unexpected corpus case: ' + value)


def number(value: JsonValue) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError('layout measurement must be numeric')
    return value


@dataclass(frozen=True, slots=True)
class Geometry:
    difference: float
    baseline_source_supported: bool
    annotation_gaps: tuple[float, ...]
    line_gaps: tuple[float, ...]
    multiline_gap: float | None
    annotation_text_fragments: tuple[int, ...]
    display: str
    scripts: int


def geometry(raw: object) -> Geometry:
    if not isinstance(raw, str):
        raise ValueError('layout measurement must be JSON text')
    record = object_value(decode(raw, reject_duplicates=True, reject_nonfinite=True))
    support = record['baseline_source_supported']
    if not isinstance(support, bool):
        raise ValueError('CSS property support must be boolean')
    multiline = record['multiline_gap']
    return Geometry(number(record['difference']), support,
        tuple(number(gap) for gap in array(record['annotation_gaps'])),
        tuple(number(gap) for gap in array(record['line_gaps'])),
        None if multiline is None else number(multiline),
        tuple(integer(count) for count in array(record['annotation_text_fragments'])),
        string(record['display']), integer(record['scripts']))


@dataclass(frozen=True, slots=True)
class Row:
    case: CaseName
    width: int
    size: int
    line_height: LineHeight
    geometry: Geometry

    def representation(self) -> dict[str, JsonValue]:
        g = self.geometry
        return dict(case=self.case, width=self.width, size=self.size, line_height=self.line_height,
            difference=g.difference, baseline_source_supported=g.baseline_source_supported,
            annotation_gaps=list(g.annotation_gaps), line_gaps=list(g.line_gaps), multiline_gap=g.multiline_gap,
            annotation_text_fragments=list(g.annotation_text_fragments), display=g.display, scripts=g.scripts)


def valid_measurement(row: Row) -> bool:
    g = row.geometry
    if len(g.line_gaps) != (2 if row.case == 'line-reservation' else 0):
        return False
    if row.case in {'ruby-multiline', 'anno-multiline'}:
        if g.multiline_gap is None or g.multiline_gap <= .1:
            return False
    return (g.scripts == 0 and abs(g.difference) < .1
            and bool(g.annotation_text_fragments)
            and all(count == 1 for count in g.annotation_text_fragments)
            and bool(g.annotation_gaps)
            and all(gap >= -.1 for gap in g.annotation_gaps)
            and all(gap >= -.1 for gap in g.line_gaps))


@dataclass(frozen=True, slots=True)
class Browser:
    name: Engine
    version: str
    measurements: tuple[Row, ...]


@dataclass(frozen=True, slots=True)
class Incomplete:
    pass


@dataclass(frozen=True, slots=True)
class Passed:
    pass


@dataclass(frozen=True, slots=True)
class Failed:
    error: str


type Outcome = Incomplete | Passed | Failed


@dataclass(frozen=True, slots=True)
class Report:
    corpus_sha256: str
    css_sha256: str
    playwright: str
    browsers: tuple[Browser, ...]
    outcome: Outcome

    def representation(self) -> dict[str, JsonValue]:
        browsers: dict[str, JsonValue] = {browser.name: dict(version=browser.version,
            measurements=[row.representation() for row in browser.measurements]) for browser in self.browsers}
        result: dict[str, JsonValue] = dict(corpus_sha256=self.corpus_sha256, css_sha256=self.css_sha256,
                                           playwright=self.playwright, browsers=browsers, result='failed')
        match self.outcome:
            case Incomplete():
                pass
            case Passed():
                result['result'] = 'passed'
            case Failed(error):
                result['error'] = error
            case _:
                assert_never(self.outcome)
        return result
