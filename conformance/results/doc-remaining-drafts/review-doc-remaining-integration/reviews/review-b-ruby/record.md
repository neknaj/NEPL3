# Independent review of chapter 05 and 10 Ruby repairs

Reviewed chapter 05 at f840e5195973bb254eb06b9d30066dfa3dcf7a5d and chapter 10 at e3a4de2b3ebc12834a47e51833a979da168f2275. The fixed before/after manuscripts, same-commit original Markdown, authoring policy and formal Doc structure definitions are identified by byte hashes in sources.json. This review did not modify production files or either author's manuscripts.

No additional blocking finding was identified. Chapter 05's previously reported omissions of Ruby in ordinary Japanese text are resolved in this fixed revision. Chapter 10's additions likewise retain the underlying prose and document structure.

## Independent comparisons

The review script parses both complete manuscripts using the fixed formal structure audit and independently expands sentence literals. Its comparison projection replaces Ruby with its base, flattens Concat and merges adjacent Text while preserving Sentence boundaries and all remaining constructors, fields and sequence order. Both complete before/after projections match exactly. This is a proof of preserved underlying prose and non-Ruby structure, not a claim that adding pronunciation annotations leaves the full Doc meaning unchanged.

- Chapter 05: 288 Sentences, 1,250 Ruby occurrences and 525 distinct base/reading pairs. All 82 inline code values retain the original Markdown order. Both RawCode bodies and their language hints match the original Markdown exactly; the one link is retained. No ordinary narrative Text containing unannotated Kanji was detected.
- Chapter 10: 127 Sentences, 423 Ruby occurrences and 260 distinct base/reading pairs. All 23 inline code values, the one link, the header and five data rows of the four-column bridge table, and the ten CLI list items retain their content and order. No ordinary narrative Text containing unannotated Kanji was detected.

All Ruby bases were checked for Kanji-only content (including the iteration mark), with nonempty readings. The distinct readings were read in context; no concrete incorrect pronunciation or okurigana split requiring correction was found. Existing annotations are retained by the full structural comparison; no arbitrary quantity of new Anno annotations is required by the authoring policy. Code and code examples remain byte-preserved rather than acquiring prose Ruby.

Chapter 05 was reviewed as a repair delta, with the complete before/after comparison and original Markdown code checks above. Its prior full meaning review at 50c6fde is preserved as prior-review.md and supplies separate evidence about the pre-repair manuscript. Chapter 10 received a direct full reading against its original Markdown: Profile identity and host registration, purity and capability boundaries, the bridge table, nested guest environments, operation cycles, artifact dependencies, CLI commands/options/exit codes, and platform limits remain represented without an identified omission or reversal.

## Limits and execution evidence

check.log records the independently executed static check and execution.json records its actual exit status. Source blobs were rechecked against their stated Git commits before the final manifest was generated. The scripts are review helpers, not production Doc parser implementations.

This review did not execute the production parser, lowerer, HTML renderer, native/WASI runtime or page registry. It does not establish successful full-document resource admission, canonical migration, old heading compatibility, or completion of the separate human meaning review required by the migration contract. The author's own scratch checks and the root's earlier resource-limit results are not counted as independent execution here.

An initial review-helper kind-name mismatch was corrected locally before the successful run. Intermediate debug projections from that helper error are excluded from the evidence manifest and are not a production finding.
