# Editor specification canonical source

Fixed source checkpoint: `5c4ce6184f6d5fa817f472d202101aaf86af3195`.
The reviewed manuscript moves to `doc/spec/08-editor.nepld`. Four headings
clarify their subject to avoid collisions between preserved GitHub aliases
and automatically generated heading IDs. Reversing only those four literal
changes reproduces the entire original Doc manuscript byte for byte.
The article retains55 paragraphs,11 InlineCode values,10 Sections,11 headings
and1,330 Ruby nodes. There are no links, tables, lists or raw code blocks.
The registry now contains nine canonical chapters.

## Generation and failed observations

Both initial Markdown and HTML generation stopped with NodeLimit and created
no output. The first preparation had CRLF JSON bytes in the working tree while
Git stored LF; the original bytes, digests and normalization record are saved.
That first observation is not represented as an exact checkout-byte baseline.
The separate LF invocation used exact Git blobs and reproduced both stops.
Its unchanged HTML allowance stopped at10,000,000 nodes during resolve/render,
after parsing/lowering, with Work217,292,905 and Allocation565,174,117.

A separate generation selects finite aggregate Markdown Nodes30M (from20M)
and HTML Nodes20M (from10M). Markdown Work600M/Allocation1.5B and HTML
Work300M/Allocation750M, all other resources and per-page parse/lower/core
defaults remain unchanged. No stopped budget was reset. These are selected
operation allowances, not measured minimums or physical heap guarantees.
The successful Markdown receipt records Source219,832 / Work551,648,649 /
Nodes23,239,293 / Allocation1,485,863,605 / Output623,324 / Depth14 before
receipt serialization. The HTML receipt records Work291,681,123 /
Nodes11,289,472 / Allocation619,413,526 / Output645,210 / Depth14.

Both ten-file Markdown generations and eleven-file HTML generations match
byte for byte. Existing Markdown projections remain exact except chapter01
and12 context headers, which bind the changed registry. All eight previous
HTML documents and their stylesheet remain exact; the manifest changes.
The guarded fixed-source run passed14 canonical tests, canonical checking,
HTML regeneration, format, Clippy and repository checks. The three-draft test
is excluded only from this repeat; its unchanged execution is retained in the
prerequisite HTML-budget archive. The root test invocation took14,769 seconds;
the subsequent canonical check took73.33 seconds. No cause for that large
elapsed-time difference is asserted from these logs alone.

Independent content review read the entire original and adopted manuscripts.
Root and independent display tests each passed12 JavaScript-disabled cases
in Chromium, Firefox and WebKit at375/1280 widths and two non-root paths,
including120 semantic-anchor movements. The independent review compares all
paragraphs, headings, code values and Ruby pairs. Root directly viewed the
narrow WebKit screenshot; the large title wraps without clipped readings.

The fixed GitHub projection has11 legacy and10 semantic targets, all unique.
Root's first navigation helper failed its offscreen precondition; a second
invocation timed out waiting for networkidle. Both partial records are retained,
with tool errors explicitly summarized rather than fabricated as raw stderr.
A corrected root probe uses one loaded document and stable offscreen geometry,
then requires native fragment movement and stable visible geometry. All21
targets pass. The independent review also passes all21 targets and exact
server rawLines, and separately checks this stable precondition. These results
do not convert the two earlier failed invocations into successes or establish
GitHub availability guarantees.

Independent code review rebuilt the fixed source and compiled Doc tests. Its
first canonical check exceeded900 seconds; the initial helper failed to persist
child output or PID/CPU on that exception. The missing instrumentation is
explicitly recorded, not reconstructed as raw logs. A later invocation retained
the same source, binary, resource limits and900-second deadline, with raw logs
and CPU sampling. It passed in74.459 seconds. Two independently generated
Markdown/HTML bundles match, and separate former-node-limit operations reproduce
both NodeLimit stops with no output. The original timeout remains a failed
invocation; low available memory observed afterward does not prove its cause.

Root and independent bootstrap review executed the restoration fence below,
restored eleven original files byte for byte, and displayed the saved chapter
in three browsers with JavaScript disabled and HTTP(S) blocked. Independent
checks compare all55 paragraphs,11 headings,11 code values and1,330 Ruby pairs,
with no network attempts. The earlier reviewer manifest and exact README/fence
used for its invocation are retained; this prose update does not change the fence.
Archive integration review is separate from execution. This chapter migration does not complete
editor runtime semantics, T21/T16, Playground, Pages publication, KaTeX or
human assistive-technology testing.

## Read the saved HTML without a compiler

Run this exact snippet from the repository root to restore a new directory.
No compiler, Node or generation service is needed to read the saved result.

```python
from pathlib import Path
import hashlib, json

archive = Path("conformance/results/doc-canonical-editor")
output = Path("dist/canonical08-snapshot")
entries = json.loads((archive / "payloads.json").read_text(encoding="utf-8"))["files"]
selected = [e for e in entries if e["owner"] == "root-html-first"]
assert len(selected) == 11
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

Open `dist/canonical08-snapshot/docs/spec/08-editor.html` with adjacent assets.
The index records original paths, lengths and SHA-256 values. Strict reviewer
manifests exclude caches, executables and temporary workspaces. Any oversized
payload is split into ordered binary parts with a whole-file digest in
`chunked_files`; concatenate those parts to restore its exact bytes.
The eleven HTML/asset files above are unchunked. `.fixture` preserves bytes.
