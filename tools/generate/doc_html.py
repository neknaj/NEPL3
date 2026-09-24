"""Generate explicit native Doc HTML adapters from the normative operation schema."""
import argparse
from pathlib import Path
import sys
from types import MappingProxyType

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.generate import adapters

OUTPUT = ROOT / "crates/languages/doc/html/src/portable/value/generated.rs"


def tuple_case(name: str, _case: str) -> bool:
    return name == "MarkupRoot"


def generate() -> str:
    options = adapters.Options(
        frozenset(("MarkupSyntax",)),
        MappingProxyType({"languageHint": "language_hint", "sourceMaps": "source_maps",
                          "documentDigest": "document_digest", "guestDigest": "guest_digest",
                          "firstElement": "first_element"}),
        tuple_case,
        adapters.Codec.REGISTRY,
    )
    return adapters.generate(ROOT, "doc-html", "doc_html", options)


class Arguments(argparse.Namespace):
    write: bool = False


def main() -> None:
    parser = argparse.ArgumentParser()
    _ = parser.add_argument("--write", action="store_true")
    args = parser.parse_args(namespace=Arguments())
    text = generate()
    if args.write:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        _ = OUTPUT.write_text(text, encoding="utf-8", newline="\n")
    elif not OUTPUT.exists() or OUTPUT.read_bytes() != text.encode("utf-8"):
        raise SystemExit(f"{OUTPUT.relative_to(ROOT)} is stale; run python tools/generate/doc_html.py --write")


if __name__ == "__main__":
    main()
