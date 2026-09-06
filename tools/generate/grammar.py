"""Generate the typed Grammar AST shape from the normative complete form catalog.

This generator does not parse a Grammar source or construct a bootstrap package.
The seed adapter and the production compiler retain separate responsibilities.
"""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "crates/languages/grammar/core/src/model/generated.rs"
SCHEMA = ROOT / "interfaces/grammar.json"


def descriptor(root=ROOT):
    categories = json.loads((root / "design/forms.json").read_text(encoding="utf-8"))["categories"]
    named = lambda name, package="nepl3.grammar": {"named": {"package": package, "revision": 1, "name": name}}
    record = lambda fields, constraints=[]: {"record": fields, "constraints": constraints}
    types = {
        "NodeId": record([["index", "U64"]]),
        "NameLiteral": record([["value", "Text"], ["span", named("Span", "nepl3.foundation")]]),
        "TextLiteral": record([["value", "Text"], ["span", named("Span", "nepl3.foundation")]]),
        "NatLiteral": record([["value", "Integer"], ["span", named("Span", "nepl3.foundation")]], ["grammar.natural"]),
        "NodeList": record([["items", {"list": named("NodeId")}], ["span", named("Span", "nepl3.foundation")]], ["grammar.list_source"]),
        "Node": record([["kind", named("NodeKind")], ["span", named("Span", "nepl3.foundation")]], ["grammar.node_source"]),
        "Document": record([["sources", {"list": named("SourceContent", "nepl3.foundation")}], ["nodes", {"list": named("Node")}], ["root", named("NodeId")]], ["grammar.constructor_graph"]),
    }
    variants = {}
    for category, body in categories.items():
        if not category.startswith("Grammar/"):
            continue
        for form in body["forms"].values():
            fields = []
            for field in form["fields"]:
                read = field["read"]
                ty = named("NodeList") if isinstance(read, dict) else named(read[1:] + "Literal") if read.startswith("@") else named("NodeId")
                fields.append([field["name"], ty])
            variants[form["kind"]] = fields
    types["NodeKind"] = {"variant": variants, "constraints": ["grammar.constructor_categories"]}
    return {"package": "nepl3.grammar", "revision": 1, "types": types, "operations": {}}


def generate(root=ROOT):
    categories = json.loads((root / "design/forms.json").read_text(encoding="utf-8"))["categories"]
    categories = {name.split("/")[1]: value for name, value in categories.items() if name.startswith("Grammar/")}
    lines = ["// Generated from design/forms.json by tools/generate/grammar.py. Do not edit.",
             "use super::*;", "#[rustfmt::skip]", "#[derive(Clone,Copy,Debug,Eq,PartialEq)]", "pub enum Category {"]
    lines += [name + "," for name in categories]
    lines += ["}", "#[rustfmt::skip]", "#[derive(Clone,Debug,Eq,PartialEq)]", "pub enum NodeKind {"]
    forms = [(category, head, form) for category, body in categories.items() for head, form in body["forms"].items()]
    # Rust reserves Self; this spelling change is only a native variant name.
    forms = [(category, head, {**form, "kind": "SelfSelector" if form["kind"] == "Self" else form["kind"]}) for category, head, form in forms]
    def field_name(name):
        return "r#" + name if name in {"self", "type", "ref"} else name
    def ty(read):
        if isinstance(read, dict):
            return "NodeList"
        return {"@Name": "NameLiteral", "@Text": "TextLiteral", "@Nat": "NatLiteral", "@Lang": "LangLiteral"}.get(read, "NodeId")
    for _, _, form in forms:
        fields = [f'{field_name(f["name"])}: {ty(f["read"])}' for f in form["fields"]]
        lines += [form["kind"] + (" { " + ", ".join(fields) + " }" if fields else "") + ","]
    lines += ["}", "#[rustfmt::skip]", "impl NodeKind {", "pub fn category(&self) -> Category { match self {"]
    for category, _, form in forms:
        lines += [f'Self::{form["kind"]}' + (" {..}" if form["fields"] else "") + f' => Category::{category},']
    lines += ["} }", "pub fn spelling(&self) -> &'static str { match self {"]
    for _, head, form in forms:
        lines += [f'Self::{form["kind"]}' + (" {..}" if form["fields"] else "") + f' => "{head}",']
    lines += ["} }", "pub(super) fn visit(&self, visitor: &mut impl Visitor) -> Result<(), ModelError> { match self {"]
    for _, _, form in forms:
        names = [field_name(f["name"]) for f in form["fields"]]
        lines += [f'Self::{form["kind"]}' + (" { " + ", ".join(names) + " }" if names else "") + " => {"]
        for f, name in zip(form["fields"], names):
            read = f["read"]
            if isinstance(read, dict):
                lines += [f'visitor.list({name}, Category::{read["list"].split("/")[1]})?;']
            elif read.startswith("@"):
                lines += [f'visitor.literal(LiteralRef::{read[1:]}({name}))?;']
            else:
                lines += [f'visitor.node(*{name}, Category::{read.split("/")[1]})?;']
        lines += ["Ok(()) },"]
    lines += ["} }", "}"]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    from lowering import update
    update(args.write)
    expected = generate()
    schema = json.dumps(descriptor(), ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.write:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(expected, encoding="utf-8", newline="\n")
        SCHEMA.write_text(schema, encoding="utf-8", newline="\n")
    elif not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != expected or not SCHEMA.exists() or SCHEMA.read_text(encoding="utf-8") != schema:
        raise SystemExit("Grammar AST is stale; run python tools/generate/grammar.py --write")


if __name__ == "__main__":
    main()
