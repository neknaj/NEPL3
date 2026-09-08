# HTML fragment specification canonical source

Fixed source checkpoint: `e8c396ada1e3d48d981f7a43276889e691e1be1b`.
The unchanged reviewed manuscript becomes `doc/spec/19-html-fragment.nepld`.
All eighteen paragraphs, seventeen inline-code values and five external links
retain the original meaning and constraints. The title's legacy GitHub alias is
preserved. This page has no semantic Sections; no section-navigation count is
claimed. The structural markup contract does not imply complete document,
asset, renderer or provider validation.

## Checks and resources

The initial clean `6d391fa` seven-page Markdown operation stopped at
`AllocationLimit` before creating output. Its command, executable SHA-256 and
source/registry identities are retained. HTML succeeded separately with the same
executable and source set. A new Markdown operation explicitly selected finite
Work400M / Allocation1B, replacing300M /750M before entry. Nodes20M and all other
limits, HTML limits, core defaults and per-page parsing/lowering are unchanged.
The preceding six-page usage and additional manuscript size motivated the
allowance; it is not a prediction, a Budget reset or a physical memory guarantee.

The successful receipt records Work325,596,451 / Allocation862,487,830 /
Nodes13,375,139 before receipt serialization. Two eight-file Markdown stages and
two nine-file HTML bundles are byte-identical. Legacy Markdown stays unchanged;
the architecture page updates only its input-context header. The real-corpus
test now reads the registry's explicitly typed output allowance instead of
duplicating constants. Dedicated small-budget failure tests are unchanged.

A guarded clean-checkpoint run passed fourteen canonical tests, canonical
checking, HTML regeneration, format, Clippy and repository checks. All nine
regenerated HTML files match the earlier bundle. The unchanged three-draft
parse/lower test is excluded only from this repeat; its full execution remains
in the prerequisite HTML-budget archive.

Root Chromium/Firefox/WebKit checks passed twelve JavaScript-disabled displays
at375/1280 under two non-root paths, preserving exact code values and all five
external hrefs, including fragments. The fixed GitHub projection's one legacy
title target is unique and navigable; server rawLines match the source exactly.
Root inspected the narrow WebKit screenshot. URI preservation does not assert
the destination site's availability or revision. Independent manuscript review
checked every paragraph and constraint. Code, browser and archive integration
reviews are separate and required before merging.

Independent code review rebuilt and repeated both generations, verified the old
page bytes, and checked body/code/URI preservation with a separate Markdown
parser test. It reproduced `AllocationLimit` using the former300M/750M limits
and `WorkLimit` using300M/1B in separate operations with the same fixed executable
and seven sources; neither created output. Its initial body-paragraph count
included the one alias-only HTML paragraph. That failure and the corrected
eighteen-body-paragraph plus one exact alias check are retained separately.

Independent browser review repeated the twelve displays, comparing all eighteen
base paragraphs,463 Ruby annotations, seventeen code values and five exact hrefs.
It verified the actual GitHub alias and all-notes text against fixed rawLines.
Root and independent reviewers restored the same nine files using the exact
snippet below and opened them in all three engines with JavaScript disabled and
HTTP(S) blocked. The independent run recorded no network attempts. Its optional
GitHub screenshot reload timed out after semantic/navigation checks had passed;
that log is retained, followed by a bounded DOM-ready viewport capture. This
does not change an unsuccessful request into evidence of site availability.

This does not complete T21/T16, a Web Playground, KaTeX, live Pages deployment or
human assistive-technology testing.

## Read the saved HTML without a compiler

Run the exact snippet from the repository root to restore a new directory.
No Rust, Node or generation service is required.

```python
from pathlib import Path
import hashlib, json

archive = Path("conformance/results/doc-canonical-html-fragment")
output = Path("dist/canonical19-snapshot")
entries = json.loads((archive / "payloads.json").read_text(encoding="utf-8"))["files"]
selected = [e for e in entries if e["owner"] == "root-html-first"]
assert len(selected) == 9
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

Open `dist/canonical19-snapshot/docs/spec/19-html-fragment.html` with adjacent
assets and pages. This snapshot does not replace regeneration checks. The index
retains original paths, sizes and SHA-256; strict reviewer manifests exclude
build caches/workspaces. `.fixture` preserves original bytes. Any oversized
payload is stored in ordered binary parts under `chunked_files`; concatenation
must recover its exact original size and SHA-256. The nine saved HTML/assets
files restored by the snippet are unchunked.
