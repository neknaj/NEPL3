"""Generate explicit native Doc adapters from the normative operation schema."""
import argparse
from pathlib import Path
import sys
from types import MappingProxyType

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.catalog.forms import ListRead, Read, load
from tools.generate import adapters

OUTPUT = ROOT / "crates/languages/doc/core/src/portable/value/generated.rs"


def signatures() -> str:
    forms = load(ROOT)
    out = ["# Doc：構文signatureの全表", "", "この表は `design/forms.json` と同じ規範データから生成した。`List<T>` は `cons T List<T>` / `nil`、`@` は基礎readerの識別である。", ""]

    def read(value: Read) -> str:
        return "List<" + read(value.element) + ">" if isinstance(value, ListRead) else value

    for category, definition in forms.items():
        if not category.startswith("Doc/"):
            continue
        out.extend(["## " + category, "", "| 綴り | kind | 子（順序固定） | arity |", "|---|---|---|---:|"])
        for spelling, form in definition.forms.items():
            fields = ", ".join(field.name + ": " + read(field.read) for field in form.fields) or "なし"
            out.append(f'| `{spelling}` | `Doc.{form.kind}` | {fields} | {len(form.fields)} |')
        out.append("")
        if definition.leaf:
            out.extend(["葉の認識規則：`" + definition.leaf + "`。", ""])
    return "\n".join(out)


def tuple_case(name: str, _case: str) -> bool:
    return name == "DocRoot"


def generate() -> str:
    options = adapters.Options(
        frozenset(("DocumentSyntax", "SentencePayload", "PlainTextRequest", "PrintRequest", "PageDocument", "PageSet")),
        MappingProxyType({"languageHint": "language_hint", "sourceMaps": "source_maps",
                          "documentDigest": "document_digest", "guestDigest": "guest_digest"}),
        tuple_case,
    )
    return adapters.generate(ROOT, "doc", "doc", options)


class Arguments(argparse.Namespace):
    write: bool = False


def main() -> None:
    parser = argparse.ArgumentParser()
    _ = parser.add_argument("--write", action="store_true")
    args = parser.parse_args(namespace=Arguments())
    for output, text in [(OUTPUT, generate()), (ROOT / "doc/spec/doc-signatures.md", signatures())]:
        if args.write:
            output.parent.mkdir(parents=True, exist_ok=True)
            _ = output.write_text(text, encoding="utf-8", newline="\n")
        elif not output.exists() or output.read_bytes() != text.encode("utf-8"):
            raise SystemExit(f"{output.relative_to(ROOT)} is stale; run python tools/generate/doc.py --write")


if __name__ == "__main__":
    main()
