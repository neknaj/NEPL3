"""Typed corpus and DOM measurements for visual-tree equivalence checks."""
from dataclasses import dataclass
from typing import Literal

from tools.serialization.json import JsonValue, array, decode, integer, object_value, string

type Engine = Literal['chromium', 'firefox', 'webkit']


def engine(value: str) -> Engine:
    match value:
        case 'chromium' | 'firefox' | 'webkit':
            return value
        case _:
            raise ValueError('unknown browser engine: ' + value)


@dataclass(frozen=True, slots=True)
class Input:
    original: str
    html: str
    css: str


def corpus(text: str) -> tuple[Input, ...]:
    rows: list[Input] = []
    for line in text.splitlines():
        marker = 'MATH_VISUAL_CASE '
        if marker in line:
            value = object_value(decode(line.split(marker, 1)[1], reject_duplicates=True))
            rows.append(Input(string(value['original']), string(value['html']), string(value['css'])))
    if len(rows) < 30 or not any('<svg' in row.original for row in rows):
        raise ValueError('missing actual constructor/SVG corpus')
    return tuple(rows)


@dataclass(frozen=True, slots=True)
class Asset:
    source: str
    size: int
    sha256: str


def assets(raw: bytes) -> tuple[Asset, ...]:
    manifest = object_value(decode(raw, reject_duplicates=True))
    result: list[Asset] = []
    for item in array(manifest['files']):
        value = object_value(item)
        result.append(Asset(string(value['source']), integer(value['bytes']), string(value['sha256'])))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class Node:
    tag: str
    text: str | None
    rect: tuple[float, float, float, float]
    style: tuple[str, ...]


def coordinate(value: JsonValue) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError('DOM coordinate must be numeric')
    return float(value)


def nodes(value: JsonValue) -> tuple[Node, ...]:
    result: list[Node] = []
    for item in array(value):
        record = object_value(item)
        rect = array(record['rect'])
        if len(rect) != 4:
            raise ValueError('DOM rectangle requires four coordinates')
        text = record['text']
        result.append(Node(string(record['tag']), None if text is None else string(text),
            (coordinate(rect[0]), coordinate(rect[1]), coordinate(rect[2]), coordinate(rect[3])),
            tuple(string(prop) for prop in array(record['style']))))
    return tuple(result)


def compare(original: tuple[Node, ...], rewritten: tuple[Node, ...], engine: Engine, index: int) -> None:
    assert len(original) == len(rewritten), (engine, index, 'node count')
    for before, after in zip(original, rewritten, strict=True):
        assert (before.tag, before.text) == (after.tag, after.text), (engine, index, 'content')
        assert before.style == after.style, (engine, index, before, after)
        assert all(abs(x-y) < 0.05 for x, y in zip(before.rect, after.rect, strict=True)), (engine, index, before, after)


@dataclass(frozen=True, slots=True)
class Report:
    engines: tuple[Engine, ...]
    cases: int
    comparisons: int

    def representation(self) -> dict[str, JsonValue]:
        return dict(engines=list(self.engines), cases=self.cases, comparisons=self.comparisons,
            scope='fixed CSS, visual tree and computed style lowering; not Doc/Math accessibility acceptance')
