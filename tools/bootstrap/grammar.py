"""Bounded first-seed input adapter, not the production Grammar parser.

Reads the complete constructor shape from design/forms.json, retaining every
reader/mode/binding/style/extension declaration and original UTF-8 byte positions.
Identifiers are deliberately limited to the supplied ASCII seed subset. Text
supports NEPL3's documented escapes, never Python/JSON's different escape syntax.
The resulting AST is input to the production compiler; it is not a LanguagePackage
and is not bootstrap conformance. P1/P2 must use the production reader and engine.
"""
import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from types import MappingProxyType

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.catalog.forms import Categories, ListRead, Read, load
from tools.bootstrap.model import Constructor, LiteralNode, Node, NodeList, SeedInput, Source, Span

TOKEN = re.compile(r'"(?:\\[^\r\n]|[^"\\\r\n])*"|#[^\r\n]*|[^\s"#]+')
NAME = re.compile(r"[A-Za-z_][A-Za-z_0-9]*")
NAT = re.compile(r"0|[1-9][0-9]*")
MAX_BYTES = 1024 * 1024
MAX_DEPTH = 256


class SeedError(ValueError):
    pass


def decode_text(raw: str) -> str:
    if len(raw) < 2 or raw[0] != '"' or raw[-1] != '"':
        raise SeedError("Text requires complete quotes")
    out: list[str] = []
    i = 1
    while i < len(raw) - 1:
        char = raw[i]
        i += 1
        if char in "\r\n":
            raise SeedError("Direct line break in Text")
        if char == "\\":
            if i >= len(raw) - 1:
                raise SeedError("Truncated Text escape")
            escape = raw[i]
            i += 1
            simple = {'"': '"', "\\": "\\", "n": "\n", "r": "\r", "t": "\t"}
            if escape in simple:
                char = simple[escape]
            elif escape == "u" and i < len(raw) - 1 and raw[i] == "{":
                end = raw.find("}", i + 1)
                digits = raw[i + 1:end] if end >= 0 else ""
                if not re.fullmatch(r"[0-9a-fA-F]{1,6}", digits):
                    raise SeedError("Invalid NEPL3 scalar escape")
                scalar = int(digits, 16)
                if scalar > 0x10FFFF or 0xD800 <= scalar <= 0xDFFF:
                    raise SeedError("Invalid Unicode scalar")
                char = chr(scalar)
                i = end + 1
            else:
                raise SeedError("Unsupported Text escape; JSON escapes are not NEPL3 Text")
        out.append(char)
    return "".join(out)


class SeedParser:
    def __init__(self, raw: bytes, categories: Categories) -> None:
        if len(raw) > MAX_BYTES:
            raise SeedError("Seed input exceeds 1 MiB")
        self.source: str = raw.decode("utf-8", errors="strict")
        self.categories: Categories = categories
        offsets = [0]
        for char in self.source:
            offsets.append(offsets[-1] + len(char.encode("utf-8")))
        self.tokens: list[tuple[str, int, int]] = []
        cursor = 0
        for match in TOKEN.finditer(self.source):
            if self.source[cursor:match.start()].strip():
                raise SeedError(f"Unrecognized lexical gap at byte {offsets[cursor]}")
            if not match.group().startswith("#"):
                self.tokens.append((match.group(), offsets[match.start()], offsets[match.end()]))
            cursor = match.end()
        if self.source[cursor:].strip():
            raise SeedError(f"Unrecognized lexical tail at byte {offsets[cursor]}")
        self.pos: int = 0

    def parse(self, category: Read, depth: int = 0) -> Node:
        if depth > MAX_DEPTH:
            raise SeedError("Seed constructor depth exceeds 256")
        if self.pos >= len(self.tokens):
            raise SeedError(f"Incomplete seed while reading {category}")
        raw, start, end = self.tokens[self.pos]
        self.pos += 1
        if isinstance(category, ListRead):
            if raw == "nil":
                return NodeList((), Span(start, end), (Span(start, end),))
            if raw != "cons":
                raise SeedError("List requires explicit cons/nil")
            item = self.parse(category.element, depth + 1)
            tail = self.parse(category, depth + 1)
            assert isinstance(tail, NodeList)
            return NodeList((item,) + tail.items, Span(start, tail.span.end), (Span(start, end),) + tail.heads)
        if category.startswith("@"):
            kind = category[1:]
            if kind == "Text":
                value = decode_text(raw)
            elif kind == "Name" and NAME.fullmatch(raw):
                value = raw
            elif kind == "Nat" and NAT.fullmatch(raw):
                value = raw  # Exact arbitrary precision decimal, not host float/u64.
            else:
                raise SeedError(f"Outside the seed literal subset: {category}")
            assert kind in ("Text", "Name", "Nat")
            return LiteralNode(kind, value, raw, Span(start, end))
        definition = self.categories.get(category)
        form = definition.forms.get(raw) if definition is not None else None
        if form is None:
            raise SeedError(f"Unknown seed constructor {raw} in {category}")
        fields = MappingProxyType({field.name: self.parse(field.read, depth + 1) for field in form.fields})
        cover_end = self.tokens[self.pos - 1][2]
        return Constructor(form.kind, category, Span(start, end), Span(start, cover_end), fields)

    def complete(self) -> Constructor:
        result = self.parse("Grammar/Root")
        if self.pos != len(self.tokens):
            raise SeedError("Trailing seed input")
        assert isinstance(result, Constructor)
        return result


def source_input(raw: bytes, source_id: str, uri: str, categories: Categories | None = None) -> SeedInput:
    if categories is None:
        categories = load(ROOT)
    parser = SeedParser(raw, categories)
    return SeedInput(Source(source_id, uri, hashlib.sha256(raw).hexdigest(), parser.source), parser.complete())


class Arguments(argparse.Namespace):
    path: Path = Path()
    source_id: str = ""
    uri: str = ""
    output: Path | None = None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("path", type=Path)
    _ = parser.add_argument("--source-id", required=True)
    _ = parser.add_argument("--uri", required=True)
    _ = parser.add_argument("--output", type=Path)
    args = parser.parse_args(namespace=Arguments())
    result = source_input(args.path.read_bytes(), args.source_id, args.uri)
    encoded = json.dumps(result.representation(), ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    if args.output:
        _ = args.output.write_text(encoded, encoding="utf-8", newline="\n")
    else:
        print(encoded, end="")


if __name__ == "__main__":
    main()
