# Independent Markdown Break projection review

Fixed review-only snapshot: HEAD d0e9267 plus three changed files, captured in files.json; 630 total source/support files are hashed in workspace-files.json. No production changes or Git mutation. Only a scratch independent test module and its parent module registration were added to the copied workspace.

## Scope and findings

No blocking issue has been found in the fixed changes. Existing Doc Break is projected only inside supported body sentences. Heading/edge/consecutive breaks and surrounding Text whitespace are explicitly rejected; this restriction applies to the compatibility projection, not the Doc language. Text line feeds are not silently converted. List continuation indentation corresponds to the emitted '- ' marker. Existing narrow shape restrictions remain. Renderer metadata advances markdown/1 to markdown/2.

Official rule checked: https://spec.commonmark.org/0.31.2/#hard-line-breaks . CommonMark supports backslash line endings inside a block; block-final breaks are not hard breaks, and whitespace at the next line start is ignored. The implementation's explicit refusal boundaries agree with this constraint.

## Executed checks

Native: six managed projection cases, three independent cases, and the two existing explicit-break HTML and prefix/compact printer regressions pass (11 distinct tests). The 'break' and 'projection' filters overlap; their counts are not added without deduplication. WASI: the same 11 distinct tests passed. No additional blocking issue remains in this snapshot.

Independent actual Doc parser/lower/prepare/projection -> pulldown-cmark cases:
- Five left/right payload combinations, each in a paragraph and a two-item list (10 cases): trailing literal backslash before Break; backtick code on both sides; all-space Code; punctuation/tag-like Text; Japanese scalars. Exact Text and Code values, HardBreak count, absence of SoftBreak, heading/paragraph/list/item counts are checked. Next list item remains separate.
- Two separated Breaks in one Sentence remain two HardBreak events.
- Eight body rejection cases: first/last/repeated Break, ASCII whitespace before/after, NBSP after, ideographic space before, and literal Text line feed. Two heading cases cover Article and Section. Every result must be projection Unsupported/Text, not just an arbitrary parse failure.
- Source-less typed Break document: output equals the independently written Markdown bytes; total OutputBytes equals separate preparation OutputBytes plus final output length. Work/Allocation/Output budgets each one below successful use stop and remain stopped on retry. Depth zero preserves DepthLimit. Original typed document remains equal.

The Rust probe is ASCII-only and uses Unicode escapes. Native raw UTF-8 stdout was checked for U+65E5/U+672C, independently of terminal presentation. No scratch fixture failure or production finding occurred in this slice.

## Limits

These checks establish the restricted Markdown view and its explicit refusals. They do not establish general Doc/Markdown roundtrip, legacy anchors, source-map restoration, canonical-source migration, browser presentation, or deployment. CLI file I/O was not rerun; only its renderer-version line changed. No resource caps or expected semantic sequences were increased or copied from observed Markdown.
