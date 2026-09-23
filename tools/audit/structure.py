"""Bounded design-source structure audit with the first-seed Text decoder.

Only ASCII identifiers and the supplied quoted-string subset are recognized.
No Unicode 16 XID, BCP47, sentence semantics, reader execution, bootstrap,
recovery, provider, or language operation conformance is established here.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import json
import re
import sys
from pathlib import Path
from types import MappingProxyType

ROOT = Path(__file__).resolve().parents[2]
# Also support direct execution from outside the repository root.
sys.path.insert(0, str(ROOT))
from tools.bootstrap.grammar import SeedError, decode_text
from tools.catalog.forms import Categories, Field, Form, ListRead, Read, load
from tools.serialization.json import JsonValue

TOKEN = re.compile(r'"(?:\\[^\r\n]|[^"\\\r\n])*"|#[^\r\n]*|[^\s"#]+')
NAME = re.compile(r"[A-Za-z_][A-Za-z_0-9]*")
NUMBER = re.compile(r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?")
MAX_BYTES = 1024 * 1024
MAX_DEPTH = 256


class AuditError(ValueError):
    """A structural mismatch, unsupported audit input, or resource limit."""


@dataclass(frozen=True, slots=True)
class Constructor:
    kind: str
    fields: Mapping[str, Node]


@dataclass(frozen=True, slots=True)
class OpaqueLeaf:
    token: str


type Node = Constructor | OpaqueLeaf | str | tuple[Node, ...]


def constructor(node: Node) -> Constructor:
    if not isinstance(node, Constructor):
        raise AuditError("Expected constructor")
    return node


def node_list(node: Node) -> tuple[Node, ...]:
    if not isinstance(node, tuple):
        raise AuditError("Expected constructor list")
    return node


def text(node: Node) -> str:
    if not isinstance(node, str):
        raise AuditError("Expected scalar")
    return node


def load_forms(root: Path = ROOT) -> Categories:
    try:
        return load(root, reject_duplicates=True)
    except ValueError as error:
        raise AuditError(str(error)) from error


class Parser:
    def __init__(self, source: str, categories: Categories) -> None:
        if len(source.encode("utf-8")) > MAX_BYTES:
            raise AuditError("Audit input exceeds 1 MiB")
        self.categories: Categories = categories
        self.tokens: list[str] = []
        cursor = 0
        for match in TOKEN.finditer(source):
            if source[cursor:match.start()].strip():
                raise AuditError(f"Unrecognized lexical gap at {cursor}")
            if not match.group().startswith("#"):
                self.tokens.append(match.group())
            cursor = match.end()
        if source[cursor:].strip():
            raise AuditError(f"Unrecognized lexical tail at {cursor}")
        self.pos: int = 0

    def parse(self, category: Read, depth: int = 0) -> Node:
        if depth > MAX_DEPTH:
            raise AuditError("Audit nesting limit exceeded")
        if self.pos >= len(self.tokens):
            raise AuditError(f"EOF reading {category}")
        token = self.tokens[self.pos]
        self.pos += 1
        if isinstance(category, ListRead):
            if token == "nil":
                return ()
            if token != "cons":
                raise AuditError(f"Expected cons/nil: {token}")
            item = self.parse(category.element, depth + 1)
            tail = self.parse(category, depth + 1)
            assert isinstance(tail, tuple)
            return (item,) + tail
        if category.startswith("@"):
            match category:
                case "@Text":
                    valid = token.startswith('"')
                case "@Nat":
                    valid = re.fullmatch(r"0|[1-9][0-9]*", token) is not None
                case "@Name":
                    valid = NAME.fullmatch(token) is not None
                case "@Lang":
                    valid = re.fullmatch(r"[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*", token) is not None
                case _:
                    valid = False
            if not valid:
                raise AuditError(f"Unsupported audit token for {category}: {token}")
            if category == "@Text":
                try:
                    return decode_text(token)
                except SeedError as error:
                    raise AuditError(f"Invalid NEPL3 Text: {error}") from error
            return token
        definition = self.categories.get(category)
        if definition is None:
            raise AuditError(f"Unknown category: {category}")
        form = definition.forms.get(token)
        if form is not None:
            return Constructor(form.kind, MappingProxyType({
                field.name: self.parse(field.read, depth + 1) for field in form.fields
            }))
        leaf = definition.leaf
        valid = (
            (leaf == "sentence" and token.startswith('"'))
            or (leaf == "name" and NAME.fullmatch(token) is not None)
            or (leaf == "math-number-or-name" and (NAME.fullmatch(token) is not None or NUMBER.fullmatch(token) is not None))
        )
        if not valid:
            raise AuditError(f"Unexpected token {token} for {category} at {self.pos}")
        return OpaqueLeaf(token)

    def complete(self, category: Read) -> Node:
        tree = self.parse(category)
        if self.pos != len(self.tokens):
            raise AuditError(f"Trailing tokens at {self.pos}")
        return tree


def readspec(node: Node, language: str) -> Read:
    form = constructor(node)
    fields = form.fields
    match form.kind:
        case "Builtin":
            return "@" + text(fields["reader"])
        case "Local":
            return language + "/" + text(fields["category"])
        case "Foreign":
            return text(fields["alias"]) + "/" + text(fields["category"])
        case "ListOf":
            return ListRead(readspec(fields["element"], language))
        case _:
            raise AuditError(f"Readspec outside audit subset: {form.kind}")


def signatures(tree: Node, language: str) -> Mapping[tuple[str, str], Form]:
    actual: dict[tuple[str, str], Form] = {}
    for node in node_list(constructor(tree).fields["declarations"]):
        declaration = constructor(node)
        if declaration.kind != "Form":
            continue
        fields = declaration.fields
        key = (language + "/" + text(fields["category"]), text(fields["spelling"]))
        if key in actual:
            raise AuditError(f"Duplicate form: {key}")
        members: list[Field] = []
        for item in node_list(fields["fields"]):
            field = constructor(item)
            members.append(Field(text(field.fields["name"]), readspec(field.fields["read"], language)))
        actual[key] = Form(text(fields["kind"]), tuple(members))
    return MappingProxyType(actual)


@dataclass(frozen=True, slots=True)
class SourceCheck:
    path: str
    # Examples have no form catalog to compare.
    form_signatures: int | None = None

    def representation(self) -> dict[str, JsonValue]:
        result: dict[str, JsonValue] = {"path": self.path}
        if self.form_signatures is not None:
            result["form_signatures"] = self.form_signatures
        return result


@dataclass(frozen=True, slots=True)
class Audit:
    sources: tuple[SourceCheck, ...]

    def representation(self) -> dict[str, JsonValue]:
        return {"scope": "design-source structure only; not runtime conformance",
                "sources": [source.representation() for source in self.sources]}


def audit(root: Path = ROOT) -> Audit:
    categories = load_forms(root)
    results: list[SourceCheck] = []
    for language, folder in [("Grammar", "grammar"), ("Doc", "doc"), ("Math", "math"), ("Circuit", "circuit")]:
        path = root / "languages" / folder / "syntax.neplg"
        tree = Parser(path.read_text(encoding="utf-8"), categories).complete("Grammar/Root")
        expected = {(category, head): form for category, definition in categories.items()
                    if category.startswith(language + "/") for head, form in definition.forms.items()}
        actual = signatures(tree, language)
        if actual != expected:
            raise AuditError(f"Form/source mismatch: {language}")
        results.append(SourceCheck(path.relative_to(root).as_posix(), len(actual)))
    extensions = {".neplg": "Grammar/Root", ".nepld": "Doc/Article", ".neplm": "Math/Expr", ".neplc": "Circuit/Design"}
    for path in sorted((root / "examples").rglob("*")):
        if path.suffix in extensions:
            _ = Parser(path.read_text(encoding="utf-8"), categories).complete(extensions[path.suffix])
            results.append(SourceCheck(path.relative_to(root).as_posix()))
    return Audit(tuple(results))


if __name__ == "__main__":
    try:
        print(json.dumps(audit().representation(), ensure_ascii=False, indent=2))
    except (AuditError, OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise SystemExit(f"Structure audit failed: {error}") from error
