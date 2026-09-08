"""Reproduce the fixed pre-migration chapter-zero converter fixture.

Only this page's headings, paragraphs, flat lists and single-backtick code are
accepted. A new Markdown feature is an error, never silently flattened.
The historical Markdown input is not the current specification. Current source
ownership belongs to doc/canonical.json; this converter does not rewrite it.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tools/migration/fixtures/00-contract.md"
SOURCE_COMMIT = "a163764e93e2809f8013d7bbe16721082f979143"
SOURCE_SHA256 = "4ca8d780d32696bf102774c346ffdf18d01452085f93a2daae9e49acf16b003f"
TARGET = ROOT / "doc/migration/00-contract.nepld"


def quoted(text, literal=False):
    escaped = text.replace("\\", "\\\\").replace('"', '\\"')
    if literal:
        for character in "[]{}":
            escaped = escaped.replace(character, "\\" + character)
    return '"' + escaped + '"'


def sentence(text):
    parts = text.split("`")
    if len(parts) % 2 == 0 or any(not p or p != p.strip() for p in parts[1::2]):
        raise ValueError("unsupported backtick sequence")
    for plain in parts[::2]:
        if re.search(r"[\[\]<>*_\\&$~|#]|^[-+>]|!", plain):
            raise ValueError("unsupported inline Markdown")
    if len(parts) == 1:
        return quoted(text, literal=True)
    inlines = [f'{"code" if i % 2 else "text"} {quoted(part)}'
               for i, part in enumerate(parts) if part]
    return "sentence " + " ".join("cons " + item for item in inlines) + " nil"


def generate(source):
    if "\r" in source or "\t" in source:
        raise ValueError("unexpected line ending/indentation")
    # This deliberately narrow migration does not accept Markdown hard breaks,
    # indented code, setext headings or whitespace-normalized code spans.
    for line in source.splitlines():
        if line.startswith(" ") or line.endswith("  ") or re.fullmatch(r"=+", line):
            raise ValueError("unsupported Markdown whitespace/block")
    chunks = source.strip().split("\n\n")
    if not chunks[0].startswith("# ") or "\n" in chunks[0]:
        raise ValueError("expected one article title")
    title = chunks.pop(0)[2:]
    sections = []
    for chunk in chunks:
        if chunk.startswith("## ") and "\n" not in chunk:
            sections.append((chunk[3:], []))
            continue
        if not sections:
            raise ValueError("expected section before body")
        lines = chunk.split("\n")
        if all(line.startswith("- ") for line in lines):
            if sections[-1][1] and sections[-1][1][-1].startswith("list "):
                raise ValueError("unsupported loose Markdown list")
            items = ["cons item none body cons paragraph cons " + sentence(line[2:]) + " nil nil"
                     for line in lines]
            block = "list unordered\n      " + "\n      ".join(items) + "\n      nil"
        else:
            if any(re.match(r"\s*([-+>#]|\d+[.)]\s|```|~~~|\|)", line) for line in lines):
                raise ValueError("unsupported block Markdown")
            block = "paragraph cons " + sentence(" ".join(lines)) + " nil"
        sections[-1][1].append(block)
    if len(sections) != 6:
        raise ValueError("contract section inventory changed; review migration")
    out = ["article ja " + sentence(title), "body"]
    for index, (heading, blocks) in enumerate(sections):
        out.append(f"  cons section contract_{index} " + sentence(heading))
        out.append("    body")
        out.extend("      cons " + block for block in blocks)
        out.append("      nil")
    out.append("  nil")
    return "\n".join(out) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    raw = SOURCE.read_bytes()
    # Git's canonical repository text uses LF, independent of checkout settings.
    text = raw.decode("utf-8").replace("\r\n", "\n")
    if hashlib.sha256(text.encode()).hexdigest() != SOURCE_SHA256:
        raise SystemExit("historical converter input changed; do not replace it with current spec")
    generated = generate(text)
    audit = json.dumps({"source": "tools/migration/fixtures/00-contract.md", "candidate": "doc/migration/00-contract.nepld",
                       "source_commit": SOURCE_COMMIT, "source_path_at_commit": "doc/spec/00-contract.md",
                       "source_lf_sha256": SOURCE_SHA256,
                       "candidate_sha256": hashlib.sha256(generated.encode()).hexdigest(),
                       "status": "historical-converter-fixture", "human_meaning_review": "not-run",
                       "markdown_projection": "restricted-candidate-view", "legacy_anchor_compatibility": "not-implemented"},
                      indent=2) + "\n"
    for path, data in [(TARGET, generated), (TARGET.with_suffix(".json"), audit)]:
        if args.write:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(data, encoding="utf-8", newline="\n")
        elif not path.exists() or path.read_bytes() != data.encode():
            raise SystemExit(f"stale migration candidate: {path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
