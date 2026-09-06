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
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOKEN = re.compile(r'"(?:\\[^\r\n]|[^"\\\r\n])*"|#[^\r\n]*|[^\s"#]+')
NAME = re.compile(r"[A-Za-z_][A-Za-z_0-9]*")
NAT = re.compile(r"0|[1-9][0-9]*")
MAX_BYTES = 1024 * 1024
MAX_DEPTH = 256


class SeedError(ValueError):
    pass


def decode_text(raw):
    if len(raw) < 2 or raw[0] != '"' or raw[-1] != '"':
        raise SeedError("Text requires complete quotes")
    out = []
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
    def __init__(self, raw, categories):
        if len(raw) > MAX_BYTES:
            raise SeedError("Seed input exceeds 1 MiB")
        self.raw = raw
        self.source = raw.decode("utf-8", errors="strict")
        self.categories = categories
        offsets = [0]
        for char in self.source:
            offsets.append(offsets[-1] + len(char.encode("utf-8")))
        self.tokens = []
        cursor = 0
        for match in TOKEN.finditer(self.source):
            if self.source[cursor:match.start()].strip():
                raise SeedError(f"Unrecognized lexical gap at byte {offsets[cursor]}")
            if not match.group().startswith("#"):
                self.tokens.append((match.group(), offsets[match.start()], offsets[match.end()]))
            cursor = match.end()
        if self.source[cursor:].strip():
            raise SeedError(f"Unrecognized lexical tail at byte {offsets[cursor]}")
        self.pos = 0

    def parse(self, category, depth=0):
        if depth > MAX_DEPTH:
            raise SeedError("Seed constructor depth exceeds 256")
        if self.pos >= len(self.tokens):
            raise SeedError(f"Incomplete seed while reading {category}")
        raw, start, end = self.tokens[self.pos]
        self.pos += 1
        if isinstance(category, dict):
            if raw == "nil":
                return {"list": [], "span": [start, end], "heads": [[start, end]]}
            if raw != "cons":
                raise SeedError("List requires explicit cons/nil")
            item = self.parse(category["list"], depth + 1)
            tail = self.parse(category, depth + 1)
            return {"list": [item] + tail["list"], "span": [start, tail["span"][1]], "heads": [[start, end]] + tail["heads"]}
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
            return {"literal": kind, "value": value, "raw": raw, "span": [start, end]}
        form = self.categories.get(category, {}).get("forms", {}).get(raw)
        if form is None:
            raise SeedError(f"Unknown seed constructor {raw} in {category}")
        fields = {field["name"]: self.parse(field["read"], depth + 1) for field in form["fields"]}
        cover_end = self.tokens[self.pos - 1][2]
        return {"kind": form["kind"], "category": category, "head": [start, end], "span": [start, cover_end], "fields": fields}

    def complete(self):
        result = self.parse("Grammar/Root")
        if self.pos != len(self.tokens):
            raise SeedError("Trailing seed input")
        return result


def source_input(raw, source_id, uri, categories=None):
    if categories is None:
        categories = json.loads((ROOT / "design/forms.json").read_text(encoding="utf-8"))["categories"]
    parser = SeedParser(raw, categories)
    return {"schema": "nepl3.grammar-seed-input/1", "source": {
        "sourceId": source_id, "revision": 0, "uri": uri,
        "digest": hashlib.sha256(raw).hexdigest(), "text": parser.source,
    }, "root": parser.complete()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path)
    parser.add_argument("--source-id", required=True)
    parser.add_argument("--uri", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = source_input(args.path.read_bytes(), args.source_id, args.uri)
    encoded = json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8", newline="\n")
    else:
        print(encoded, end="")


if __name__ == "__main__":
    main()
