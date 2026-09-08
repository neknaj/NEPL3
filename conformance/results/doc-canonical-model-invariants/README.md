# Model invariants canonical source

Fixed source checkpoint: `2b0bbf4b888ea18ef5e97eaabe2391008e86fe69`.
The reviewed manuscript moves unchanged to `doc/spec/12-model-invariants.nepld`.
The article keeps thirty-two paragraphs, seventeen data rows plus the table
header, nine inline code values, eight headings and one link to chapter19.
The page-context renderer resolves that link using the actual registered Doc
page, producing the appropriate Markdown or HTML route. Eight legacy GitHub
aliases and seven semantic Section identities remain distinct.

## Recorded generation

The initial clean `3a1a80c` eight-page Markdown generation stopped at
`AllocationLimit`, before creating output. The command, source identity,
executable SHA-256 and raw logs are retained. The same fixed executable completed
HTML generation, including parsing and lowering chapter12. A separate operation
selects finite aggregate Markdown Work600M / Allocation1.5B instead of400M /1B.
Nodes20M, all other resources, per-page parse/lower and core defaults, and the
independent HTML allowance are unchanged. This is an explicit allowance, not a
reset of a stopped Budget or a physical-memory guarantee.

The successful receipt records Source167,228 / Work396,100,237 /
Nodes16,319,372 / Allocation1,041,890,584 / Output432,840 / Depth14 before
receipt serialization. Both nine-file Markdown stages and ten-file HTML bundles
are byte-identical. Earlier legacy Markdown files are unchanged. Chapter01
updates only its context header. Initial and subsequent HTML files are identical;
changing the Markdown allowance does not change the HTML artifact.

The guarded fixed-checkpoint run passed fourteen canonical tests, canonical
checking, HTML regeneration, format, Clippy and repository checks. Regenerated
HTML matches all ten original files. The unchanged three-draft parse/lower test
is excluded only from this repeat; its execution remains in the prerequisite
HTML-budget archive. The400M Work limit was not independently shown insufficient;
600M is a selected allowance, not a measured minimum.

Independent manuscript review checked the entire source and every table entry.
Root and independent browser runs each passed twelve JavaScript-disabled cases
across Chromium, Firefox and WebKit,375/1280 widths and two non-root paths.
They checked eighty-four semantic-anchor movements and twelve actual link
transitions to chapter19. The fixed GitHub projection has fifteen unique and
navigable legacy/semantic targets and exact server rawLines. The independent
review compared all base paragraphs, headings, code and table cells; narrow
table text rectangles remain inside their cells. Long identifiers wrap densely,
so these checks do not claim ideal readability at every viewport or zoom.

Root executed the exact restoration snippet below, verified ten original files
byte-for-byte, and displayed the restored article and followed its chapter19 link
in all three engines with JavaScript disabled and HTTP(S) blocked. Independent
bootstrap review repeated the verbatim fence in a fresh directory, matched all
ten files and all701 Ruby annotations, and followed the local chapter19 link in
three engines without network attempts. Its original pre-bootstrap manifest and
the actual README/fence bytes used for execution are preserved.

Independent code review rebuilt the fixed source, checked both generations and
used a separate Markdown parser test for paragraph, table, code and link
preservation. It reproduced the old400M/1B allocation stop with no output. A
negative case retained chapter19's physical Doc file but removed its registry
entry: page-context generation rejected the unresolved reference as MissingPage
without creating an output stage. Thus a nearby file does not substitute for a
registered page. An initial reviewer PowerShell setup quoting failure is recorded
as a tool diagnostic summary, before any production build/test/generation ran;
it is not fabricated as a raw execution log. Archive integration review remains
separate from these execution results.
This chapter-level migration does not complete T21/T16, all runtime semantics,
Playground, KaTeX, Pages publication or human assistive-technology testing.

## Read the saved HTML without a compiler

Run the exact snippet from the repository root to restore a new directory.
No Rust, Node or generation service is needed to read this snapshot.

```python
from pathlib import Path
import hashlib, json

archive = Path("conformance/results/doc-canonical-model-invariants")
output = Path("dist/canonical12-snapshot")
entries = json.loads((archive / "payloads.json").read_text(encoding="utf-8"))["files"]
selected = [e for e in entries if e["owner"] == "root-html-first"]
assert len(selected) == 10
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

Open `dist/canonical12-snapshot/docs/spec/12-model-invariants.html` with adjacent
assets and pages. The index records original paths, lengths and SHA-256 values.
Strict reviewer manifests exclude caches, executables and temporary workspaces.
Any oversized payload is split into ordered binary parts in `chunked_files`;
concatenation must reproduce its complete original bytes and digest. The ten
HTML and asset files restored above are unchunked. `.fixture` preserves bytes.
