"""Bounded design-source structure audit, independent of the future Rust parser.

Only ASCII identifiers and the supplied quoted-string subset are recognized.
No Unicode 16 XID, BCP47, sentence semantics, reader execution, bootstrap,
recovery, provider, or language operation conformance is established here.
"""

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TOKEN = re.compile(r'"(?:\\[^\r\n]|[^"\\\r\n])*"|#[^\r\n]*|[^\s"#]+')
NAME = re.compile(r"[A-Za-z_][A-Za-z_0-9]*")
NUMBER = re.compile(r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?")
MAX_BYTES = 1024 * 1024
MAX_DEPTH = 256


class AuditError(ValueError):
    """A structural mismatch, unsupported audit input, or resource limit."""


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise AuditError(f"Duplicate JSON key: {key}")
        result[key] = value
    return result


def load_forms(root=ROOT):
    return json.loads(
        (root / "design/forms.json").read_text(encoding="utf-8"),
        object_pairs_hook=unique_object,
    )["categories"]


class Parser:
    def __init__(self, source, categories):
        if len(source.encode("utf-8")) > MAX_BYTES:
            raise AuditError("Audit input exceeds 1 MiB")
        self.categories = categories
        self.tokens = []
        cursor = 0
        for match in TOKEN.finditer(source):
            if source[cursor:match.start()].strip():
                raise AuditError(f"Unrecognized lexical gap at {cursor}")
            if not match.group().startswith("#"):
                self.tokens.append(match.group())
            cursor = match.end()
        if source[cursor:].strip():
            raise AuditError(f"Unrecognized lexical tail at {cursor}")
        self.pos = 0

    def parse(self, category, depth=0):
        if depth > MAX_DEPTH:
            raise AuditError("Audit nesting limit exceeded")
        if self.pos >= len(self.tokens):
            raise AuditError(f"EOF reading {category}")
        token = self.tokens[self.pos]
        self.pos += 1
        if isinstance(category, dict):
            if token == "nil":
                return []
            if token != "cons":
                raise AuditError(f"Expected cons/nil: {token}")
            return [self.parse(category["list"], depth + 1)] + self.parse(category, depth + 1)
        if category.startswith("@"):
            valid = {
                "@Text": token.startswith('"'),
                "@Nat": re.fullmatch(r"0|[1-9][0-9]*", token),
                "@Name": NAME.fullmatch(token),
                "@Lang": re.fullmatch(r"[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*", token),
            }
            if not valid.get(category):
                raise AuditError(f"Unsupported audit token for {category}: {token}")
            return json.loads(token) if token.startswith('"') else token
        if category not in self.categories:
            raise AuditError(f"Unknown category: {category}")
        definition = self.categories[category]
        if token in definition["forms"]:
            form = definition["forms"][token]
            return {"kind": form["kind"], "fields": {
                field["name"]: self.parse(field["read"], depth + 1)
                for field in form["fields"]
            }}
        leaf = definition["leaf"]
        valid = (
            (leaf == "sentence" and token.startswith('"'))
            or (leaf == "name" and NAME.fullmatch(token))
            or (leaf == "math-number-or-name" and (NAME.fullmatch(token) or NUMBER.fullmatch(token)))
        )
        if not valid:
            raise AuditError(f"Unexpected token {token} for {category} at {self.pos}")
        return {"kind": "leaf", "token": token}

    def complete(self, category):
        tree = self.parse(category)
        if self.pos != len(self.tokens):
            raise AuditError(f"Trailing tokens at {self.pos}")
        return tree


def readspec(node, language):
    fields = node["fields"]
    match node["kind"]:
        case "Builtin":
            return "@" + fields["reader"]
        case "Local":
            return language + "/" + fields["category"]
        case "Foreign":
            return fields["alias"] + "/" + fields["category"]
        case "ListOf":
            return {"list": readspec(fields["element"], language)}
        case _:
            raise AuditError(f"Readspec outside audit subset: {node['kind']}")


def signatures(tree, language):
    actual = {}
    for declaration in tree["fields"]["declarations"]:
        if declaration["kind"] != "Form":
            continue
        fields = declaration["fields"]
        key = (language + "/" + fields["category"], fields["spelling"])
        if key in actual:
            raise AuditError(f"Duplicate form: {key}")
        actual[key] = {"kind": fields["kind"], "fields": [
            {"name": field["fields"]["name"], "read": readspec(field["fields"]["read"], language)}
            for field in fields["fields"]
        ]}
    return actual


def audit(root=ROOT):
    categories = load_forms(root)
    results = []
    for language, folder in [("Grammar", "grammar"), ("Doc", "doc"), ("Math", "math"), ("Circuit", "circuit")]:
        path = root / "languages" / folder / "syntax.neplg"
        tree = Parser(path.read_text(encoding="utf-8"), categories).complete("Grammar/Root")
        expected = {(category, head): form for category, definition in categories.items()
                    if category.startswith(language + "/") for head, form in definition["forms"].items()}
        actual = signatures(tree, language)
        if actual != expected:
            raise AuditError(f"Form/source mismatch: {language}")
        results.append({"path": path.relative_to(root).as_posix(), "form_signatures": len(actual)})
    extensions = {".neplg": "Grammar/Root", ".nepld": "Doc/Article", ".neplm": "Math/Expr", ".neplc": "Circuit/Design"}
    for path in sorted((root / "examples").rglob("*")):
        if path.suffix in extensions:
            Parser(path.read_text(encoding="utf-8"), categories).complete(extensions[path.suffix])
            results.append({"path": path.relative_to(root).as_posix()})
    return {"scope": "design-source structure only; not runtime conformance", "sources": results}


if __name__ == "__main__":
    try:
        print(json.dumps(audit(), ensure_ascii=False, indent=2))
    except (AuditError, OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise SystemExit(f"Structure audit failed: {error}") from error
