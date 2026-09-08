# Independent review: Doc inventory and reference catalog drafts

Result: no additional blocking manuscript finding.

This is a content and authoring review of another author's drafts. No manuscript, production, original Markdown, registry or Git state was changed. Review scratch only was written here.

## Fixed inputs

- Original commit: 337057d71838c85a6fb9a304e3ec4164436bb310.
- doc/doc-inventory.md SHA256: 13637103c1e5581238cc68a099f6e28553fbee21cc0bfd33fe557ca461093264.
- doc/spec/references.md SHA256: c9ba86ece67d1bfe46b420d8b91b2dce95eb6e57a6b63d8c14e17534cafe9217.
- Inventory draft: 074af5f822e873ae0d8b40a273b02dad1052ad09, doc/migration/authored/guide/doc-inventory.nepld.
- Reference draft: 9fa716d23e44b5d5a8a288d0564911311ad1c755, doc/migration/authored/references.nepld.
- AGENTS, authoring guidance, spec05, Doc signatures, forms and structural audit implementation were separately frozen from the original commit. Source bytes and SHA256 are in sources.json.

## Direct content review

Both original Markdown texts and the complete 136-line / 39-line manuscripts were read directly, not inferred from an author's description. The annotated Japanese, sentence boundaries, table cells and explicit code/link constructors were checked against the source and authoring guidance. Ruby bases contain only Kanji; okurigana and other characters remain outside. No missing narrative Ruby or meaning-changing reading was found. No new translation or unnecessary annotation was introduced.

Inventory retains the fixed historical baseline 25a096bc183c2b71200902884084cd4082d0aae3, the 57-page / 10-source classifications, all observation counts and exclusions, original bytes and UTF-8 ranges, and the distinction between early design input and R014/J01-J04 acceptance. Its four tables preserve all 36 rows including headers and both right-aligned count columns. The inline projection profile, soft versus hard breaks, math versus Text, nested-image once-only projection, HTML-fragment noninterpretation, DG01-DG09 gaps and R006/R009/R014 conditions remain complete. Historical claims about missing constructors or implementations were not rewritten as current status. The check/check-current/write modes, delta failure, Git-history requirement and unverified rendering/meaning boundaries remain intact.

References retains the 2026-09-06 Asia/Tokyo acquisition date, the recorded 2026-09-05 Library provenance, the explicit statement that Library contents were not supplied or directly checked, the 74-file basis and separation of NEPL3's own designs from external sources. All 18 catalog entries remain in source order. Turning bare URLs into explicit External links preserves each URI exactly in both target and visible label. WIT is not conflated with the NDF ABI, and OpenType MATH retains its conditional backend-boundary description.

## Executed checks

`python .tmp/review-b-inventory-references/check.py` completed successfully. It uses independently frozen Git blobs and the formal structural parser plus a review-only literal/Ruby normalizer. It does not generate or rewrite drafts.

- Full base text equals the original text, after removing Markdown structural syntax and joining paragraph source lines. Whitespace inside individual source lines and RawCode is preserved by the comparison.
- Inventory: 175 Sentence nodes, 17 Paragraphs, 5 exact section titles, 30 exact inline code values, one exact 235-byte RawCode block with the original sh hint, 3 exact link targets, 4 tables / 36 exact rows and matching alignments, 600 Ruby occurrences.
- References: 29 Sentence nodes, 2 exact section titles, 1 exact inline code value, 18 exact list items, 18 External targets whose labels equal the same original URLs, 72 Ruby occurrences.
- Both: formal Doc/Article structure, UTF-8 LF without BOM, no unannotated narrative Kanji. The complete normalized structures and reading lists are retained alongside the final check log.
- Finalization rechecks every fixed source file against its specified Git blob before creating the evidence manifest.

## Explicit limits

This review does not execute the production parser, lower, prepare, render, native/WASI tests, the historical inventory commands or browser display. It does not check URL reachability, external-document contents, relative-link registry routing, generated HTML IDs, accessibility or source-position equivalence. Exact original relative path strings are preserved; their migration binding remains separate. Historical statements have been checked for faithful retention, not revalidated as present-day implementation facts. Neither manuscript is declared canonical, runtime-accepted or migration-complete by this review.
