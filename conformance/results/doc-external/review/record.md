# Independent Doc external HTML links review

Review-only snapshot after bf7db0b, 630 copied production/support files. No production edits or Git mutations. Eight changed paths are recorded in files.json, including the migration index source present at capture. All runtime tests use this private workspace and isolated targets.

## Results

Native focused: 6 passed (new managed 2, independent 3, existing local unresolved-input regression 1). Native complete tools Doc integration: 55 passed. WASI complete tools Doc integration: 54 passed (the existing host-only test remains excluded). Complete Markup regression: 19 passed on each native and WASI target. Distinct combined totals are 74 native and 73 WASI; the focused six are included, not added again.

No implementation defect has been reproduced. The initial static specification inconsistency (every link described as BetweenArtifacts) was corrected by root to page-set internal links. The final spec-only delta was independently read and hashed separately; no production runtime change was required.

## Independent coverage

- Direct external_uri: 5 accepted and 27 rejected strings under the existing constrained profile. Cases include http/https/mailto, punycode and percent-encoded path, query ampersand/apostrophe, malformed/overflow/signed ports, whitespace/control/backslash/quote/tag characters, credentials, malformed percent escapes, relative/protocol-relative references, data/javascript/mixed-case scheme, and malformed mail addresses. Work allowance exactly 4*len succeeds; smaller allowances stop before processing; repeated stopped calls remain stopped. Empty input also observes cancellation. No allocation charged by this lexical helper.
- Actual production Doc parse/lower -> page renderer with external and internal self-link together. Independent Python HTMLParser checks exact href values after entity decoding, exact visible text, balanced DOM, and no injected extra attributes. It does not make a browser URL-parser or remote availability claim.
- Actual request NDF -> CBOR -> fresh FoundationCodec receiver -> render equals native RenderedPages. Safe alternate and javascript URI mutations of the response NDF are rejected on first CBOR receive/replay. A legitimate response is also rejected against a changed semantic request URI.
- Single-language selection hides the nonselected valid external link from output but does not suppress its validation. Three targeted schema-valid LinkTarget.External request mutations, leaving source snapshots/digests untouched, survive structural request decode and then produce InvalidExternalUri for the hidden node during rendering. Raw request decoding is not falsely treated as URI authorization.
- Five Work allowances, including baseline minus one, produce typed sticky WorkLimit with a fresh admission/codec per run. Managed cases additionally retain an image plus external-link requirement as NeedsResolution, validate eight hidden unsafe URIs, preserve cancellation, and test query escaping/mailto output.
- The complete Doc integration run includes the existing local renderer, page links/hidden-anchor rules, portable operations, projection, printer/text, and actual-source cases.

## Review fixture corrections

Initial scratch stop sweep reused a populated SourceAdmission after baseline measurement, so its baseline-minus-one expectation was invalid. The test was corrected to fresh admission/codec per run; all five original caps then stopped as expected. This was not a production defect. Initial generic request text mutation matched two strings (semantic URI and the declared generated source text); the assertion caught this before decode. The final mutation targets only the named LinkTarget.External NDF variant, preserving source data. Both initial probes/logs are retained. Expectations for production behavior were not changed.

## Limits

The new helper retains the preexisting deliberately restricted lexical URI profile, not the full browser URL grammar. No network request, credentials grant, target availability, navigation, or remote content execution was tested or certified. Remaining image/foreign requirements still prevent a successful page result. No full operation Report envelope or deployment claim is inferred. Production source hashes remain fixed; the only private workspace production-path modification is the scratch parent test module registration.

## Final host metadata delta

Read the final two-line export/pages.rs diff separately. The renderer identifier advances pages/1 to pages/2; the scope now includes checked external http/https/mailto hrefs and explicitly excludes network/destination availability, assets/foreign rendering, and Pages deployment evidence. This matches the reviewed runtime support without changing manifest format, options, viewer_scripts=false, resource usage, files, or route identity. Static review only; no additional runtime test count. Exact source and diff are retained under host-delta/.
