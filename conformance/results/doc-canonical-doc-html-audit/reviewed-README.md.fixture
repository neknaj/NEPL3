# Doc HTML specification canonical source

Source checkpoint: `f459596e87dfe28b24954cfad82472626033ea91`.
Chapter20 is now authored in `doc/spec/20-doc-html.nepld`; its Markdown
is a generated projection. The ten-page registry preserves the three old
GitHub anchors and adds the two semantic section anchors. The chapter keeps
16 paragraphs, 13 inline code values, four external links and 467 Ruby pairs.
The opening distinguishes the pure local fragment API from the development
host which already supplies a document shell and fixed CSS.

## Generation and limits

The historical failed runs and incorrect borrowed-binary attribution remain
in `../doc-canonical-doc-html-checkpoint/`; they are not successful evidence.
Correct local production generation originally stopped at AllocationLimit
for Markdown and WorkLimit for HTML without publishing output.

The registry selects finite aggregate output allowances: Markdown Work800M
and Allocation2B, HTML Work400M. Other resources, core defaults and per-page
parse/lower allowances are unchanged. These are operation allowances, not
minimum budgets or physical heap measurements. No stopped budget is reset.
Successful Markdown usage before receipt serialization is Work607,160,055
and Allocation1,633,873,615; HTML usage is Work319,420,660 and
Allocation677,108,703. The old affected allowances are below these values.

Two independent invocations per output format reproduce all bytes: 11 files
for Markdown and 12 for HTML. One complete bundle per format is retained;
the generation logs and comparisons bind both invocations. All nine prior
HTML documents and CSS remain byte-identical. Only chapter01/12 Markdown
context headers change alongside chapter20's newly generated projection.

## Verification scope

Root fixed-source checks pass format, 14 canonical tests, canonical --check,
Clippy and repository validation. Only the unchanged three-draft corpus test
(`synchronized_context_spec_drafts_parse_lower_and_check_labels`) is excluded
from this local repeat; its earlier execution is retained in the
HTML-budget archive and required CI still runs it. Repository validation
does not establish current inventory completeness or runtime acceptance.

Independent reviewers read the original and revised whole manuscript,
generated Markdown, output manifests and actual production source. Root
and independent display probes each exercise 12 JavaScript-disabled
contexts in Chromium, Firefox and WebKit, with two widths and two non-root
paths. Each probe verifies 24 section moves. Fixed-commit GitHub tests
verify the three legacy and two semantic anchors and exact rawLines.

Independent compiler-free restoration preserves all 12 files and verifies
six section moves over three file:// browser engines with JavaScript off.
WebKit's Playwright offline emulation failed internally; its successful
case instead blocks HTTP(S) routes in a normal context, with no network URLs
observed. Initial harness failures are preserved rather than counted as passes.

The execution reviewer separately extracts the exact Git source checkpoint,
builds there (52.32s), and runs canonical --check (82.94s). Restoring the old
registry only in that scratch copy reproduces Markdown AllocationLimit
(77.22s) and HTML WorkLimit (42.19s), both exit1 with no output directory.
The registry is restored and all 9,597 archived source files rechecked.
Initial Windows long-path extraction failure is recorded; a shorter fresh
destination is used for successful execution. No borrowed executable is used.

`payloads.json` identifies source bytes, all archived payload hashes and
losslessly split oversized files. Files are stored with a `.fixture` suffix
to prevent evidence from becoming active source or generated documentation.
Review manifests retain their original narrower scopes and chronological
pending statements; later reports supplement rather than rewrite them.

This is evidence for this chapter cutover, not T21/T16 completion, four
language product completion, Wasm/Playground validation, assistive-technology
testing or Pages publication. Required exact-head CI is a separate merge gate.

## Compiler-free restoration

Copy `scripts/restore-final20.py.fixture` to a temporary `.py` file and run:

```sh
python restore-final20.py conformance/results/doc-canonical-doc-html /fresh/destination
```

It restores the 12-file HTML bundle and verifies both index and artifact
hashes without invoking a compiler. Open `docs/spec/20-doc-html.html` from
that destination. Retain the CSS and relative directory layout.
