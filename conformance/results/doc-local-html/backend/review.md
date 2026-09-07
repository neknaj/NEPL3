# Independent review: current local Doc HTML

The fixed Rust/portable scope passes. One separate audit-helper defect is reproduced: browser.py cannot consume the unmodified Cargo nocapture log it documents. Root has been notified; no production code or shared documentation was edited.

## Exact source and scope

This is the 909-file snapshot in workspace/, identified by files.json, with base 1b7d400c72a670dc2123883ced146e84bdb19eb6 plus the current overlay. The base commit alone does not identify the tested implementation. source-verification.json rechecks all 909 original SHA values after execution: zero mismatches. Source storage SHA is e2d301f1640bba50c99991feba9836ca84f032be174877d8383928f153f707da. Builds depend only on this snapshot; subsequent live shell/CLI edits are excluded.

Reviewed spec20, Doc requirement discovery, local preparation, iterative render jobs and paragraph/block scheduling, typed markup validation, portable request/replay, interfaces/doc-html.json, generated descriptor/value codec, Cargo/dependency metadata and CI entries. The no_std backend depends on core, Doc core and markup, and performs no platform I/O or guest evaluation. The schema operations table remains empty: helper transport does not claim a full Report operation.

prepare_local first inspects the whole Article, labels and source closure. External Link/Asset/Foreign requests return the entire discovery plan. Private preparation binds the borrowed document and options; no raw packet mints preparation or external authority. The rendered packet includes document digest, options and ordered element causes, and receiving it re-renders and compares canonical values. Identical HTML under different options is insufficient.

## Executed evidence

Rust 1.97.0, Cargo 1.97.0, Wasmtime 44.0.1. Commands, working directory, deadlines and exit status are in managed-runs.json, independent-runs.json, sweep-runs.json and metadata-runs.json. Native executables and WASI components are retained next to full execution logs. No limit increase or production instrumentation was used.

- Existing fixed production tests: tools Doc html:: 9 and doc-html local 2 pass on both native and wasm32-wasip2. This includes the actual 13,757-byte linear-combination source through parse/lower/prepare/render/markup validation/serialization, with the existing section/table/Parallel assertions. This is a new completion result for this fixed pipeline, not a rewrite of earlier cost-profile failures. The helper uses stage budgets; it is not evidence of one global budget across all stages.
- Original independent main.rs and bin/source.rs from review-doc-local-html are byte-for-byte unchanged (original-probes.json). Only the Cargo dependency paths and helper extraction use current production. Both pass native/WASI. The helper comes from the current actual compiler/profile/parser support; original source strings and expected output assertions were not updated from observed results.
- Exact escaping of Unicode, CR/LF/TAB and XML delimiters; exact element causes; source-less values; XML-invalid NUL remains a typed error; native request/reply plus actual first CBOR receiver; six schema-preserving digest/origin/options mutations, stale document and source omission reject.
- Recursive Paragraph preserves order and closes p around child Blocks. Ruby/Anno preserve base, reading and all notes. Forward labels use UTF-8 hex IDs. Rows/Columns preserve variant order; Single uses explicit fallback order and ASCII case folding, accepts an existing empty variant, and rejects missing/duplicate/invalid choices. Hidden unresolved labels still fail.
- Tables preserve header/cell alignment and order, empty table, ordered start zero and unchecked item, escaped RawCode hint and CRLF/TAB payload, and heading levels seven/eight use accessible div headings. External Link/Asset/Code returns the original full three-requirement plan.
- Output depths 255 and 256 succeed, 257 fails with OutputDepth without changing input. Shared DAG expansion hits the Node budget; caller depth seven composes and restores. Five render stop reasons, seven preparation stops, typed Output0 versus actual serialization OutputLimit are separately tested.
- New sweep.rs covers valid request decoding and prepared rendered-value replay over Work/Allocation/Nodes/Depth/Output caps. Each target records 249 exact sticky stops and 20 complete equal values, plus cancellation in both phases. Input is unchanged, consumed resource never exceeds the cap, and no nested semantic error substitutes for an actual stop. This is sampled-cap coverage, not exhaustive fault injection or all eight resource dimensions.
- Non-writing doc_html.py generation check passes. Actual managed/independent registry construction, finalization and first CBOR exercise the descriptor/codec closure. A whole-workspace metadata gate was not independently rerun here; root owns those gates.

## Browser audit and concrete helper defect

The fixed actual HTML corpus and fixed production CSS pass 126 baseline checks in Chromium and Firefox, with document JavaScript disabled, after extracting the exact DOC_HTML_CASE records from the Cargo log. Browser identities, corpus/CSS hashes and results are in browser-result.json. This bounded baseline test does not claim universal browser/accessibility/print/responsive support or distribution-shell readiness.

The original unmodified tools-native.log starts its first corpus record with `test html::browser_layout_corpus_from_real_doc_source ... DOC_HTML_CASE ruby ...`. browser.py only accepts lines starting with DOC_HTML_CASE, drops ruby, and exits 1 with `missing or unexpected corpus cases`. browser-raw-corpus-run.json and .log retain the actual failed invocation. normalized-corpus.log removes only the Cargo status prefix, preserves each HTML hex value, and succeeds through the same unmodified script. A robust record extractor or a dedicated corpus export should replace reliance on line-start placement; required names/duplicates must still be checked. This is a developer audit input bug, not a Rust rendering failure. No production fix has been applied by this reviewer.

An earlier default-Python attempt failed before corpus parsing because Playwright was not on sys.path. The actual reproduction and browser success use the already installed review-markup-html/python modules via PYTHONPATH. This is an environment adjustment, not an installed package or modified production script.

## Limits retained

This approves only local Article-to-checked-fragment behavior exercised above. Full PreparedArticle, external resource existence/resolution, guest evaluation, general renderer authority, complete Report envelope, shell/CLI/distribution and later live changes remain outside this snapshot. The existing annotation-heavy Article now completes the tested production pipeline, but this does not imply every large document succeeds under its limits. Old profiling logs and their failures remain unchanged.
