# Independent RawCode Markdown projection review

Review-only fixed snapshot after HEAD 42093f9: 630 source/support files, three changed production paths in files.json. A later managed test-only delta adds RawCode to the existing stop fixture; its exact bytes are retained in test-delta/ and test-delta.json, and the prior snapshot test bytes in initial/. No production or Git changes by reviewer. Only an independent scratch module and its parent registration are added to the copied workspace.

## Findings

No runtime defect has been found in this fixed slice. During the initial static read, a newly introduced test .unwrap() was flagged against workspace quality policy; root changed it to a typed test error before snapshot. No independent failure of the old test was claimed. The corrected line and subsequent RawCode stop-fixture delta are included in the executed source.

## Independent validation

Native: 12 distinct tests pass (eight managed projection tests, three independent tests, one existing HTML RawCode regression). The raw_code and projection filters overlap in two tests, which are not counted twice. WASI: the same 12 distinct tests passed. No additional blocking issue remains in this fixed scope.

Sixteen actual-source cases: eight text/hint combinations, each placed in an Article body and in a Section body. Each has preceding/following Paragraphs, one list before and another after RawCode, plus an adjacent empty RawCode. Production parser/lower/prepare/projection output is fed to pulldown-cmark. Exact RawCode text bytes and hint strings, two distinct code blocks, two lists/two items, paragraph/heading counts, and the outside text are compared to independent source expectations. Html/InlineHtml/indented-code events are rejected. Text cases include empty body, one/two blank lines, leading/trailing TAB/spaces, Markdown and HTML-looking text, several backtick runs, tildes, backslash, Japanese and U+2028. Hint cases include None, Rust, c++, x.y_z-1, hyphen, and a digit. List-contained RawCode is required to fail as Unsupported rather than a parser error.

Source-less typed negatives require Error::Text for nine byte-normalization/control cases (missing final LF, CR, CRLF, interior CR, NUL, VT, FF, DEL, NEL) and twelve hint cases (empty, surrounding/interior whitespace, backtick/tilde, attribute-like syntax, entity text, backslash, non-ASCII). These tests distinguish projection rejection from parser admission.

A separate typed fixture contains 260 consecutive backticks. The independently constructed expected output uses 261-character fences, keeps its hint and all payload bytes, and has no extra code content. Total OutputBytes equals separately measured prepare OutputBytes plus emitted Markdown UTF-8 length. Work/Allocation/Output limits one below the successful usage produce sticky typed stops with no returned partial String; cancellation stays Cancelled and input Doc remains equal. Existing managed stop testing also includes actual parsed RawCode after the test delta.

Official CommonMark rules were read at https://spec.commonmark.org/0.31.2/#fenced-code-blocks : closing fences must use the same marker and at least the opening length; code contents are literal; opening indentation can remove content indentation, so emitting at column zero matters; backtick info strings cannot contain backticks. The restricted nonempty ASCII hint and newline refusal policies are explicit additional projection constraints.

## Scope

This remains a restricted compatibility view, not a canonical-source switch or general Doc/Markdown roundtrip. RawCode is not evaluated. No change to HTML RawCode support, long-term Doc migration requirements, assets, CSS, deployment, source mappings, or legacy anchors is inferred. The CLI renderer-version change to markdown/3 was read; CLI file I/O was not rerun. All independent probe source is ASCII with Rust Unicode escapes. No test expectation or resource cap was raised to obtain passes.
