"""Typed representation of the existing grammar-seed-input/1 contract."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Literal

from tools.serialization.json import JsonValue


@dataclass(frozen=True, slots=True)
class Span:
    start: int
    end: int

    def representation(self) -> list[JsonValue]:
        return [self.start, self.end]


@dataclass(frozen=True, slots=True)
class Constructor:
    kind: str
    category: str
    head: Span
    span: Span
    fields: Mapping[str, Node]

    def representation(self) -> dict[str, JsonValue]:
        return {"kind": self.kind, "category": self.category,
                "head": self.head.representation(), "span": self.span.representation(),
                "fields": {name: node.representation() for name, node in self.fields.items()}}


@dataclass(frozen=True, slots=True)
class LiteralNode:
    literal: Literal["Text", "Name", "Nat"]
    value: str
    raw: str
    span: Span

    def representation(self) -> dict[str, JsonValue]:
        return {"literal": self.literal, "value": self.value,
                "raw": self.raw, "span": self.span.representation()}


@dataclass(frozen=True, slots=True)
class NodeList:
    items: tuple[Node, ...]
    span: Span
    heads: tuple[Span, ...]

    def representation(self) -> dict[str, JsonValue]:
        return {"list": [node.representation() for node in self.items],
                "span": self.span.representation(),
                "heads": [head.representation() for head in self.heads]}


type Node = Constructor | LiteralNode | NodeList


@dataclass(frozen=True, slots=True)
class Source:
    source_id: str
    uri: str
    digest: str
    text: str

    def representation(self) -> dict[str, JsonValue]:
        return {"sourceId": self.source_id, "revision": 0, "uri": self.uri,
                "digest": self.digest, "text": self.text}


@dataclass(frozen=True, slots=True)
class SeedInput:
    source: Source
    root: Constructor

    def representation(self) -> dict[str, JsonValue]:
        return {"schema": "nepl3.grammar-seed-input/1",
                "source": self.source.representation(), "root": self.root.representation()}
