# Math HTML specification canonical source

Source checkpoint: `2f3545ecad601bc19e19c0d3495ee6fd11eedb36`.
Chapter17 uses `doc/spec/17-math-html.nepld` as its source and generated
Markdown as its projection. Its relative chapter18 link is resolved by the
existing page-set profile. This is documentation migration, not completion
of the KaTeX, MathML, Playground or Pages implementations.

The manuscript preserves all21 original inline-code values, the two tables
with3 and8 data rows, five external links and the chapter18 reference.
Independent full-prose/authoring reviews and production output comparisons
found no contract loss. Ordinary prose uses sentence literals; structural
inline content uses explicit sentences. The703 Ruby elements retain their
reviewed readings. GitHub's old8 anchors and new7 semantic anchors each
resolve uniquely and preserve their intended destinations.

## Reproduction and resource scope

Initial generation at `63ade49f2a288a4bcfaa74794214ff9a1bc110af` passed
Markdown but stopped HTML serialization at AllocationLimit749,999,977 of
750,000,000. No HTML output directory was published. The original failed
command, exit status and logs are retained; the second generation was not
executed in that failed run.

A separate invocation at `c5a79411b60a34a4e74d161b560dc31ec0f521ad`
selects1,000,000,000 allocation units for the expanded eleven-page HTML
operation. HTML Work400M/Nodes20M, all other resources, Markdown allowances
and core/per-page defaults remain unchanged. HTML usage before manifest
serialization is Work362,735,172, Allocation762,304,126, Nodes13,880,291.
These are logical operation counters, not measured heap size or minimum
allowances. No stopped budget was reset or converted into success.

Two invocations per format reproduce all12 Markdown and13 HTML bundle
files byte-for-byte. Existing ten HTML documents and CSS are unchanged.
Only chapter17 and the chapter01/12 Markdown context headers were adopted.
The archived prior chapter20 bundle remains in
`../doc-canonical-doc-html/`; cleanup of old worktrees does not remove it.
`adopt17-original.py` preserves the historical invocation helper, while
`adopt17.py` uses a local baseline restored from that committed archive.

Root checks at the source checkpoint pass format,14 canonical unit tests,
canonical --check, Clippy and repository checks. The unchanged expensive
three-draft corpus test is excluded from this local repeat and remains in
mandatory CI. Repository checks do not prove current inventory completeness
or any runtime acceptance group.

## Display and bootstrap

Independent JavaScript-disabled Chromium, Firefox and WebKit probes use
375px and1280px viewports at non-root paths. Code, tables, CSS,703 Ruby,
chapter18 navigation and all seven section destinations are checked without
horizontal document overflow. Direct file viewing succeeds on all three.
WebKit's explicit offline emulation failed internally; a normal file context
with HTTP(S) aborted succeeds and is recorded separately. Route-fulfilled
tests are also labeled separately. No failure is counted as a pass.
Human screen-reader/device QA is unperformed.

`payloads.json` hashes every raw `.fixture` payload and identifies the adopted
source bytes. One complete bundle per format is retained. To restore HTML
without compiling, copy `scripts/restore17.py.fixture` to a temporary script:

```sh
python restore17.py conformance/results/doc-canonical-math-html /fresh/destination
```

The script verifies13 payloads and the manifest's12 document/CSS hashes.
Open `docs/spec/17-math-html.html` with the directory layout intact.
Exact-head required CI and archive review are separate integration gates.
This record does not mark T21/T16 or any whole acceptance group complete.
