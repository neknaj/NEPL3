# Circuit specification canonical source

Fixed checkpoint: `e600c9325193fce64e4c4a7116a8cecca31f6e75`.
The reviewed Circuit manuscript becomes `doc/spec/07-circuit.nepld`. Its only
authoring changes clarify two headings as NOR IR conversion and SVG placement,
with Kanji-only Ruby. Reversing those two headings reproduces the original draft
bytes. All requirements, including simultaneous state updates and independent
NOR evaluation, remain specification requirements rather than claimed runtime.
Ten original GitHub anchors are retained alongside nine semantic Section IDs.

## Generation and finite resources

At clean `ba00264`, the six-page Markdown operation stopped with `NodeLimit`
before making its output directory. The command, source and registry identity,
executable SHA-256 and original failure log are preserved. HTML succeeded in a
separate operation with the same executable and source set. The canonical
Markdown output profile then explicitly selected Nodes20M instead of10M before
a new operation. All other limits, HTML limits, parsing/lowering limits and core
defaults are unchanged. This is not continuation of the stopped Budget.

The successful Markdown receipt records Nodes11,060,708, Work267,063,756 and
Allocation712,384,053 before receipt serialization. Allocation is a logical
counter, not measured physical RAM. Two seven-file Markdown stages and two
eight-file HTML bundles are byte-identical. Old legacy Markdown bytes remain
unchanged; the architecture page changes only its input-context header.

A guarded clean-checkpoint run passed fourteen canonical tests, canonical
checking, HTML regeneration, format, Clippy and repository checks. All eight
HTML files match the earlier bundle. The unchanged three-draft parse/lower test
is excluded only from this repeat and is recorded in the prerequisite HTML-budget
archive. The real-page test explicitly uses the same finite20M-node allowance.

Root Chromium/Firefox/WebKit checks passed twelve JavaScript-disabled displays
at375/1280 under two non-root paths, with108 real semantic fragment moves. The
unordered declaration list has five items; the ordered elaboration list has six.
Original inline-code values remain present, with ten headings and no duplicate
HTML IDs or horizontal viewport overflow. Root inspected the narrow WebKit image.
Nineteen actual GitHub targets passed unique-target navigation with exact server
rawLines and stable visible positions. Legacy Markdown aliases are not claimed
as HTML anchors. Independent preflight and heading reviews are separate records;
independent code review rebuilt and regenerated both formats twice, repeated the
10M-node failure with the same executable, and checked the original text, twelve
code values and both lists through a separate Markdown parser test.

Independent browser review repeated all twelve displays and108 semantic moves,
nineteen GitHub targets, and compared all33 body paragraphs, ten headings and
both lists in order. It confirmed that the two clarified headings have automatic
GitHub IDs distinct from the preserved aliases. Root and independent reviewers
restored the eight exact files using the snippet below and opened them in all
three engines with JavaScript disabled and HTTP(S) blocked. The reviewer's first
paragraph-count probe included nineteen empty alias paragraphs; the initial
failure and corrected explicit treatment are retained. UTF-16 process logs are
preserved as raw bytes with separately hashed UTF-8 copies, not relabeled UTF-8.

This page migration does not implement Circuit runtime, T21/T16, Playground,
KaTeX, live Pages deployment or human assistive-technology testing.

## Read the saved HTML without a compiler

Run this exact snippet from the repository root to restore a new directory.
Neither Rust, Node nor a generation service is needed.

```python
from pathlib import Path
import hashlib, json

archive = Path("conformance/results/doc-canonical-circuit")
output = Path("dist/canonical07-snapshot")
entries = json.loads((archive / "payloads.json").read_text(encoding="utf-8"))["files"]
selected = [e for e in entries if e["owner"] == "root-html-first"]
assert len(selected) == 8
output.mkdir(parents=True, exist_ok=False)
for entry in selected:
    relative = Path(entry["restore"])
    assert not relative.is_absolute() and ".." not in relative.parts
    data = (archive / entry["file"]).read_bytes()
    assert len(data) == entry["bytes"]
    assert hashlib.sha256(data).hexdigest() == entry["sha256"]
    destination = output / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("xb") as stream:
        stream.write(data)
```

Open `dist/canonical07-snapshot/docs/spec/07-circuit.html` with the adjacent
assets and pages. This snapshot does not replace regeneration checks. The index
retains original paths, byte lengths and SHA-256. Reviewer payloads use strict
manifests, excluding build caches/workspaces. `.fixture` preserves original bytes;
reserved archive path components are renamed with `saved-`.
The large independent GitHub screenshot is stored in ordered binary chunks to
respect the repository file-size bound. `chunked_files` lists their order and
the original size/SHA-256; concatenating their bytes restores the exact PNG.
This does not rescale or edit the image. The eight HTML/assets files used by the
snippet are ordinary, unchunked payloads.
