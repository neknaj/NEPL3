# Independent adjacent lists in annotated Markdown

Implementation checkpoint: `6e47d57c874e53a894154b4a349132c34526b9b9`.
The new production-source regression fails before the implementation with
`Unsupported { node: 27 }`; the original output and build log are retained.
A fixed metered comment now separates sibling lists following CommonMark
0.31.2 example308 / GFM example288. Ordinary comment-shaped text remains text.
This extends the annotated profile's accepted inputs; it does not add RawHtml
or alter existing Doc schema/List semantics or core resource defaults.

Root and independent native executions each pass 19 annotated tests.
WASI under Wasmtime44.0.1 passes18; the file-writing host-only test is excluded
by its existing target condition. Cases include separate numbering, same and
different starts,0/nine-digit boundaries, task markers, Break, notes/code,
three sibling lists, intervening blocks/sections, page-set link resolution,
invalid second lists and unsupported shapes, consumed/exact/insufficient
budgets, cancellation and sticky stops without partial artifacts.

Production canonical --check confirms all10 existing Markdown projections
remain exact. Format, Clippy and repository checks pass. Repository checks
do not establish current inventory completeness or whole-runtime acceptance.

The real production host generates the retained six-list fixture. GitHub's
GFM API preserves six sibling lists, starts7/7/42, two task markers and one
Break. This API HTML check is not an interactive browser/accessibility test.
Its source, Markdown, HTML, command/result and hashes are retained separately.

`manifest.json` hashes all raw `.fixture` payloads. Source changes and their
formal contract are reviewed independently in the retained review. This is
a migration prerequisite, not chapter11/17 canonical cutover, T21 completion,
Pages deployment or full required conformance. Exact-head CI is a separate
integration gate.
