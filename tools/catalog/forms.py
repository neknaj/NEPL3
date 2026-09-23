"""Validate the constructor catalog at its JSON boundary.

These records describe the existing design/forms.json format. They do not parse
language source or implement the production Grammar model.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Literal

from tools.serialization.json import JsonValue, array, decode, object_value, string


@dataclass(frozen=True, slots=True)
class ListRead:
    element: Read


type Read = str | ListRead
type Leaf = Literal["sentence", "name", "math-number-or-name"] | None


@dataclass(frozen=True, slots=True)
class Field:
    name: str
    read: Read


@dataclass(frozen=True, slots=True)
class Form:
    kind: str
    fields: tuple[Field, ...]


@dataclass(frozen=True, slots=True)
class Category:
    forms: Mapping[str, Form]
    leaf: Leaf


type Categories = Mapping[str, Category]


def read(value: JsonValue) -> Read:
    if isinstance(value, str):
        return value
    record = object_value(value)
    if set(record) != {"list"}:
        raise ValueError("read must be a category, builtin, or single list element")
    return ListRead(read(record["list"]))


def field(value: JsonValue) -> Field:
    record = object_value(value)
    if set(record) != {"name", "read"}:
        raise ValueError("field requires exactly name and read")
    return Field(string(record["name"]), read(record["read"]))


def form(value: JsonValue) -> Form:
    record = object_value(value)
    if set(record) != {"kind", "fields"}:
        raise ValueError("form requires exactly kind and fields")
    return Form(string(record["kind"]), tuple(field(item) for item in array(record["fields"])))


def category(value: JsonValue) -> Category:
    record = object_value(value)
    if "forms" not in record or set(record) - {"forms", "leaf"}:
        raise ValueError("category requires forms and an optional leaf")
    leaf = record.get("leaf")
    # Explicit narrowing also excludes arbitrary JSON containers and scalars.
    match leaf:
        case None | "sentence" | "name" | "math-number-or-name":
            return Category(MappingProxyType({
                head: form(item) for head, item in object_value(record["forms"]).items()
            }), leaf)
        case _:
            raise ValueError("unsupported catalog leaf")


def categories(value: JsonValue) -> Categories:
    return MappingProxyType({name: category(item) for name, item in object_value(value).items()})


def load(root: Path, *, reject_duplicates: bool = False) -> Categories:
    value = decode((root / "design/forms.json").read_text(encoding="utf-8"),
                   reject_duplicates=reject_duplicates)
    return categories(object_value(value)["categories"])
