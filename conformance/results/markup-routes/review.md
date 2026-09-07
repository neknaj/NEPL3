# Independent review: BetweenArtifacts route serialization

The fixed bounded slice passes; no new implementation blocker was found. No production, shared documentation or Git edits were made. This is route structure/serialization/first-receiver review, not Doc PageRegistry or site completion.

## Source and formal boundary

workspace/ is the recorded 1,478-file snapshot of C:/projects/NEPL3-doc-page-links at the base in snapshot.json plus its frozen overlay. files.json identifies every byte; source-verification.json confirms no mismatch after all runs. Builds and probes depend only on that snapshot, not root's subsequent Doc page work.

Reviewed HtmlHref native enum, check.rs, serialize.rs, interfaces/markup.json, generated descriptor/value adapters, design/markup.json, spec19 and the route/base requirements in spec15. The variant is exactly BetweenArtifacts with ordered fields source:Text, target:Text, fragment:Option<Text>. Schema and generated native conversion agree. It is additive to the existing typed markup vocabulary; no raw HTML or arbitrary attribute path bypass was added. The non-writing markup generator check passes.

Both source and target use the existing closed artifact path grammar: nonempty ASCII alphanumeric/-_./ segments, excluding empty, dot and dot-dot segments. Percent escapes, slash-root paths, schemes/colon, query/fragment syntax, backslash, controls and non-ASCII are rejected. Optional fragment uses the existing id grammar. Complete directory segments are compared, so a/ and ab/ do not alias. The target filename remains a filename even when its spelling equals a source parent directory.

The serializer's generated parent traversal is derived from two validated artifact-root paths. The host must still bind the declared source to the actual output shell and verify target/fragment existence, edition and artifact ownership. A typed/raw declaration is not that authority. The independent ghost target/source example is accepted structurally, while an ordinary Fragment with a missing local ID is rejected. This matches the explicitly bounded spec19 contract.

Official reference checked: RFC 3986 section 5.2.3 merges a relative path with the base path excluding its final segment; section 5.2.4 handles generated dot segments. https://www.rfc-editor.org/rfc/rfc3986#section-5.2.3 . Browser URL resolution below is independent executable evidence, not an inference from the standard alone.

## Executed tests

Commands, targets, deadlines, full logs and retained native/WASI binaries/components are in runs.json and receive-runs.json. Rust/Cargo 1.97.0 and Wasmtime 44.0.1 identities are retained.

- All four managed route tests pass native and wasm32-wasip2.
- Independent main probe tests 14 distinct paths crossed with each other and fragment None/Some: 392 routes per target. Cases include root files, siblings, deeper directories, complete-segment prefixes, case/underscore/dash, leading dot, trailing dot characters, and a three-dot directory. Every route traverses actual native encode -> CBOR -> fresh NDF/codec receiver with an empty SourceStore/SourceAdmission, then reproduces the complete HtmlRequest and serialized bytes. Literal BetweenArtifacts tag and source/target field order are asserted.
- Forty-six invalid source/target combinations are rejected both native and through schema-valid raw Text mutations followed by actual CBOR receive. Native fragment-invalid examples reject. A second independent probe adds seven unknown tag, field-count/type, optional-fragment type and malformed fragment packets, then checks that the original packet still decodes correctly.
- Valid combined validate/serialize sampled caps cover Work, Allocation, Output, Nodes and Depth: 105 exact sticky stops and 10 exact outputs per target, plus cancellation. First-receiver sampled caps cover Work, Allocation, Nodes and Depth: 97 exact sticky stops and 8 exact values per target, plus cancellation. These are sampled boundaries, not all possible allocation failure positions or all resource types.
- Input requests remain unchanged. The new path algorithm charges Work before its source/target scans, computes the generated length with checked arithmetic and an isize bound, charges allocation before String::with_capacity, and only returns a complete serialized result. The output path text is still escaped/charged through the existing attribute serializer. SourceBytes is not source admission for semantic path strings; this slice does not create SourceSnapshots or grant source authority.

## Independent URL expectations

routes.json preserves all 392 actual serialized hrefs. The independent expected path uses a filesystem path routine only for relative *directory* computation and appends the explicit target filename. The first draft mistakenly used a generic full-path relpath for source a/b, target a; that returned dot (a directory) instead of ../a (the target file). This was an oracle setup error, not a production failure. urls-before-oracle.py and oracle-setup-failure.log preserve it; no production code or actual route output was changed to resolve that mistake.

Both Chromium and Firefox then resolve every href using their real URL interface against five deployment bases: /NEPL3/, /acceptance/project/, another nested prefix, root, and an offline file:///C:/offline/site/ base. Each performs 1,960 resolutions and matches exactly base + explicit target + optional fragment. This catches scheme/root escape, parent-directory mistakes and loss of non-root site prefixes. browser-url.json records browser versions and scope. These tests exercise URL resolution, not HTTP availability or target-file existence, which remains host preparation work.

## Limits of approval

This supports the new closed route value, native serializer, resource handling and raw first receiver. The receiver can validate spelling and derive a safe relative reference without knowing actual output placement. It cannot prove that the host used the claimed source route. PageRegistry, cross-page anchor resolution, same-revision output existence, redirects, deployment and the whole site remain outside this slice. Root's wider markup/Doc/generation/CI runs are separate evidence; their results are not inferred here.
