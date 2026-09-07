# Independent HTML anchor identifier review

No remaining blocking discrepancy found within this first-stage scope. Aliases, old GitHub anchors, Doc label mapping, canonical migration and human meaning review are not implemented or certified by these results. Doc backend n-hex remains unchanged.

## Source and stages

sources.json binds 2,283 files copied from feat/html-anchor-identifiers at the recorded head plus working changes. All fixed workspace bytes were verified unchanged after execution. The main tests and browser outputs use this original fixed workspace. final-delta.json separately binds the subsequent serialize.rs capacity guard (SHA 6cafc9b897a921ba825da8b0f0f94a63e592d0e38f07f5d56e7263ce2bcb4878), spec19 and AGENTS. The spec19 file was already identical in the copied workspace; its captured diff is empty. AGENTS' authoring/delegation instruction does not change production behavior. No production/Git edits were performed by the reviewer.

The original fixed complete managed suites passed: markup 21 + Doc HTML 2 on both native and wasm32-wasip2. Commands, isolated target directory, exit codes, process deadlines and full logs are preserved. The final 3-line capacity guard is independently executed as the exact extracted private function on native/WASI. The entire first-stage suite/browser was not rebuilt after that guard-only delta; normal output had no further production change, and any root final integration rerun is separate from this evidence.

## Actual production API probe

probe/main.rs uses public markup validate/serialize, actual generated descriptors, portable to/from value, real CBOR and fresh receiver store/admission. Sixteen decoded IDs times Fragment/Artifact/BetweenArtifacts cover Japanese, digit-leading, uppercase/underscore, percent-encoded-looking literal, literal A versus %41, quotes/apostrophe/angle brackets/ampersand/backtick, fragment-directive-looking text, emoji, NFC/decomposed strings, NBSP, U+2028, U+10000 and U+10FFFF. These are distinct exact values, not normalization inputs.

All 48 cases roundtrip and reserialize exactly; native/WASI complete result JSON is equal. Each fresh serialization's OutputBytes equals the actual emitted UTF-8 length. Semantic strings do not acquire SourceBytes merely by being markup data. The original typed request is unchanged.

The negative matrix has 219 cases: all U+0000..U+0020, U+007F..U+009F and XML-invalid U+FFFE/U+FFFF across all three Href branches, plus empty/uppercase/digit/non-ASCII/underscore DataId/DataGroup/CSS policy strings. Invalid values remain errors; widening IDs does not widen CSS/data tokens. A composed ID and decomposed Href fail MissingFragment. The old genuine revision-1 descriptor was extracted from base bc1d5d3, registered alongside revision 2, and used to validate a fully revision-1 stamped HtmlRequest structurally. After actual CBOR decode, the new public receiver rejects it. This is stronger than merely mutating a revision number to an unknown schema.

Long multi-byte/percent identifiers exercise Work/Allocation/Output sampling: 18 exact sticky stops and 6 exact successful outputs per target, plus Cancelled. No partial successful output or input mutation occurred. These are focused new encoding-boundary samples, not an exhaustive every-limit/allocator-failure proof. Existing managed suites cover the broader tree and portable boundaries.

## Independent HTML and browser checks

Python HTMLParser checks each actual HTML output's decoded ID and Href. urllib.parse.quote with explicit unreserved safe bytes supplies an independent UTF-8 uppercase-percent oracle. All 48 outputs agree. HTML escaping and URL escaping remain separate: DOM IDs retain literal characters; Hrefs percent-encode once, then undergo normal attribute escaping. In particular %41 remains distinct from A and quote/angle-bracket IDs do not create attributes or elements.

Chromium 151.0.7922.34 and Firefox 153.0 each completed 192 real link navigations with JavaScript disabled, viewport 800x420. Inputs were generated from actual Rust output and placed under /NEPL3/ and /acceptance/nested/, each tested via HTTP and file URLs. After clicking, the expected URL, exact :target ID, destination text and positive scroll position were asserted, with no script element. BetweenArtifacts tests use real sibling-directory paths; target documents use the corresponding production Fragment output. This demonstrates actual fragment navigation, not only URL construction or element existence. Browser software was already installed under the preserved review Python path. An initial default Python import lacked Playwright; no new package was installed. There was no outer browser process deadline; Playwright's default per-action timeout was used. WebKit and physical devices were not tested.

## Code/spec/schema review and capacity correction

anchor_id is separate from the pre-existing ASCII id predicate. Attribute validation still performs XML scalar validation before accepting the new decoded, nonempty profile, excluding U+0000..20 and U+007F..9F. DataId/DataGroup and class allowlists retain the original grammar. Equality and duplicate detection remain exact; XML or Unicode acceptance does not imply CSS selector syntax.

Fragment, Artifact and BetweenArtifacts all call the same encoding function before attribute escaping. Expansion lengths are checked and Work is charged before scans, Allocation before String capacity, with Output charged by final output production. Interface self references and Doc HTML's markup reference all point to revision 2; portable selects/checks revision 2. safe-markup/2 and the spec19 explanation make the semantic change explicit. Structural schema Text fields remain dynamic boundaries with semantic validation by the owning operation.

Independent source review noticed that fragment_id checked its own encoded length against isize::MAX, but later '#'/path concatenation went through allocate without a final signed-capacity bound (BetweenArtifacts already had one). This was an inherited helper boundary, not proven to be a newly introduced runtime regression. No impossible huge allocation or panic was attempted. Root added an early shared allocate guard. The exact final helper probe tests isize::MAX+1 and usize::MAX: both return sticky AllocationLimit before Work or Allocation usage; isize::MAX is accepted for accounting only; Cancelled remains sticky. Both 64-bit native and 32-bit WASI pass. This closes the local capacity contract without claiming the large real String exists.

## Limits of approval

No old Markdown URL or legacy anchor correspondence is proved by accepting Unicode IDs. Alias maps, all promised target coverage, hidden-variant handling, historical route registration and generated Markdown/GitHub compatibility remain next steps. The spec16 authoring requirement remains a separate condition; a candidate with combined paragraphs is not automatically canonical. Only bounded first-stage markup semantics and the separately identified final capacity guard are reviewed here.
