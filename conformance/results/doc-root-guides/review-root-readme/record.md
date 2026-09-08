# Independent review: root README Doc manuscript

Reviewed d022e9bf84003e7272df86147a7afbee9220d316 README.nepld against
b83716cd14fe7d6d57579e9d6a9ec4ad2877dba2 README.md, AGENTS.md and
doc/authoring.md. No blocking issue found. CODEX.nepld in the same commit was
authored by this reviewer and is deliberately excluded from this independent
review. No manuscript, original source or production file was edited.

The source change from 3ab81c3 to b83716c is one paragraph's last status clause:
the former unimplemented description is replaced by local HTML and migration
manuscript work in progress, with canonical cutover and Web publication still
incomplete. I directly read the real tools main command dispatch, export generate
pipeline (real source parse, lower::document, prepare_local, render, shell), the
local preparation's NeedsResolution behavior, implementation-status and spec16.
These justify the narrower in-progress description; they do not establish a
complete compiler/CLI/Web product. The existing committed authored manuscripts
and root draft work also substantiate authoring in progress. No actual command
or CI badge status was executed/requeried for this content review.

The seven paragraph texts match the original exactly, including the badge alt
paragraph. Body sentence counts are 1/2/5/4/2/3/1; the badge is intentionally a
label, not a prose sentence ending in punctuation. The natural prose statements,
all numeric/task/design identifiers, qualifiers, historical target architecture,
future TEA/Playground/Pages plans and non-completion assertions were read in full.
The strong span remains precisely the common runtime implementation phase.

The original two-column table remains a typed Table with an explicit header and
five body rows, in the same order. All twelve cells and the five embedded links
match. No table is flattened into prose. Three InlineCode contents match exact
bytes. RawCode retains the sh hint and all 149 bytes, including the final LF, of
the four original shell commands; it is not evaluated or represented as guest
language Code.

All fifteen Link targets preserve their original bytes and ordering. The badge
is explicitly Link(External) containing InlineImage, with the exact original SVG
URL as AssetRef.id, no supplied digest, and alt Sentence CI. This preserves
resource intent without claiming an authenticated/resolved asset. The current
local renderer rejects unresolved requirements, so this manuscript is not
advertised as a complete local HTML export. Relative targets retain the logical
root README namespace, not the physical authored/project directory. Link/resource
resolution and deployment remain separate validation.

Normal Text/Ruby sentences use literals; explicit Sentence constructors are used
for strong, links and inline code. The table's short cells are valid Sentence
units. There are no inserted sentence breaks, translations, unnecessary Anno or
decoration. All 99 Ruby occurrences were read in context and checked for Kanji-only
bases: 他実装向/たじっそうむ + け, 型付/かたつ + き, 満/み + たして,
and 読/よ + み順 are consistent with the surrounding sentence. Numbers, Latin
identifiers, punctuation and kana remain outside Ruby.

check.py freezes the actual Git files, parses the complete draft using the
same-commit structure helper and independently compares prose, table cells,
emphasis, code and targets. It rehashes all ten author payloads and verifies their
source/draft bytes against Git. Helper success is structural evidence only;
production parse/lower/native/WASI/HTML, asset loading, Web publication, canonical
switch and human acceptance were not performed or inferred.

The initial generic check traversed Image.alt's leaf as if it were a second body
Sentence, causing only the checker's count assertion to fail. It also initially
named the helper node Image instead of the actual InlineImage. The final check
counts each Paragraph's direct Flow items and uses the actual frozen form kind.
initial-check.py and initial-failure.log retain this checker failure; no draft
change was required. The failed abbreviated Git revision `3ab` was resolved to
the actual parent 3ab81c3 before the original-source diff was assessed.
