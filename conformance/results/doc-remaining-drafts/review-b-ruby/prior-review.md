# Authored chapters 05–09: independent content review

Reviewed commit: `50c6fde11fa2c5ab62edaf7c8edecdef4d25ae5e` in `C:/projects/NEPL3-doc-authoring-b`.
The five authored documents, their five original Markdown chapters, authoring.md, AGENTS.md, actual Doc Grammar source and forms.json are fixed Git blobs under snapshot/. See sources.json for exact bytes and SHA-256.

## Result

Changes required: the authored Japanese prose does not consistently apply Kanji Ruby. This affects ordinary Sentence literals as well as prefix Text. No concrete omission or reversal of a mandatory semantic condition or exception was found in the complete five-chapter comparison. These are independent conclusions: semantic coverage does not waive the authoring requirement.

The reviewer read the original Markdown and authored text directly, including all sections and the tables/code examples. The lexical scripts are supplemental checks, not a generated manuscript used as the standard of correctness. No manuscript, production implementation, tracked specification or Git state was edited.

## Required correction: Kanji Ruby

authoring.md, “RubyとAnnoの対象”, requires Japanese readings on Kanji portions; kana suffixes, particles, numbers and punctuation must remain outside the Ruby base. Its “明示的なsentence構築” section also specifies that prefix Text does not reinterpret literal annotation syntax.

Representative frozen locations:

- 05-document.nepld:5–7: normal introductory Sentence literals contain unannotated 本文/階層/文単位/翻訳対応/構造/意味/保持. Line 24 has prefix Text ` の子は通常Textであり、その内部の ` and ` を注釈と解釈しない。`.
- 06-math.nepld:5–6: most Kanji in the ordinary introductory prose remain unannotated despite a Ruby on 厳密計算. Line 17 has prefix Text `任意有理数から式を作るconstructor helper `.
- 07-circuit.nepld:37 and :40: prefix Text `: exprの値を共有する組合せ信号。` and `: 初期値を持つ状態。`.
- 08-editor.nepld:5–7: ordinary opening prose remains unannotated; line 25 has prefix Text ` に終端zero byteを付ける。`.
- 09-portability.nepld:5–7: ordinary prose remains unannotated; line 52 has prefix Text ` と各言語constructor表からcodecの網羅性検査を生成する。`; line 98 includes an unannotated link label `実装記録`. Table explanation cells also require the same review.

lexical.json reports 78 / 13 / 11 / 11 / 26 prefix Text occurrences containing Kanji without an immediately preceding Ruby constructor, for chapters 05 through 09. This narrow lexical aid is NOT a count of every omission in the chapter: it excludes ordinary Sentence literals and does not constitute a complete semantic Ruby validator. The examples above were read in their actual grammatical context.

Repair all affected prose, including ordinary literals, prefix Text, headings, cells and displayed link labels, while retaining the body, required conditions, examples, code bytes and link targets. Use actual Ruby/Concat/Text constructors in prefix positions; inserting `[漢字/reading]` into ordinary `text` would display those delimiters literally. Literal positions can use the documented annotation syntax. Do not reduce the text to meet a runtime resource cap.

The author and root were notified during review. This record covers the original fixed commit; it does not certify an unreceived or unreviewed correction. The authoring guide places phrase translation/explanation in Anno but does not prescribe a quota of annotations for every ASCII identifier. This review does not invent such a quota.

## Semantic comparison coverage

- 05 Document: recursive Flow/Paragraph, typed arena/helper roots, DAG depth and Foreign owner closure, DG01–06 element constraints, ordered requirement discovery and identity, literal delimiter/escape/nonempty rules, Break versus LF, prefix normalization and token-bound payload locations, explicit Parallel selection/fallback, forward labels and duplicate display paths, Code non-evaluation and Doc alias isolation, safe HTML and paragraph runs, operation separation, complete plain_text and print request/digest/host-assertion/budget/failure boundaries were retained. The ordinary prose requires Ruby, but these conditions were not omitted.
- 06 Math: finite decimal versus arbitrary rational; canonical denominator and exact conversion failure; preservation of authored fractions; lexical symbol/scope rules; scripts versus exponentiation; vector/matrix/range shape checks; every exact-operator branch including zero/negative/non-integer and symbolic cases; simultaneous distinctions between structural, binding and evaluation proof; MathML precedence and notation order; canonical source/Origin/View/Foreign closure; KaTeX generation policy and operation separation were retained.
- 07 Circuit: width and no-truncation conditions; separate namespaces and no bare-output lookup; forward wire references, unique next and instance state; operator bit ordering; six elaboration steps including unused-module checks and correct state-cycle treatment; simultaneous step/observe semantics; testcase and unsupported-value boundaries; deterministic NOR lowering, independent vector comparison and SVG layout/identity/security constraints were retained.
- 08 Editor: all three AnalysisKey digests and actual package identity; effective-limits/source authority boundaries; owner-local regions and exact/transformed mapping distinctions; Capture versus Presentation/Relation and full trivia replay; priority/depth/DFS and recovery/UTF-8 rules; typed native versus CBOR output charging; RegionQuery and Custom owner ledger; final resolution/sparse and source-less IDs; two-stage sealed rename with exclusive budget, complete source comparison, per-owner inverse/forward mappings, capture/collision rejection and explicit Report limitations; incremental invalidation, LSP encoding and trust rules were retained.
- 09 Portability: all NDF tags/fields and canonical CBOR restrictions, rational/integer encodings, intrinsic versus ordinary SchemaRef representations, descriptor syntax and nominal closure, canonical descriptor generation and validation, continuation/limits/trace boundaries, Transform outcome-to-outer mapping and retry/report preservation, native versus wire ABI, host framing, purity/retry, WASI and replacement conformance boundaries were retained.

Natural sentence boundaries were reviewed in context: ordinary prose is separated into Sentence values within paragraphs; code/link-containing sentences use explicit construction. Short headings/table cells and original technical declaration phrases were not incorrectly required to become full prose sentences. No introduced automatic whitespace, fake translation variant, arbitrary RawHtml or guest evaluation was found in the reviewed authoring structure. This is source review, not a claim that every file has passed a production parser/render run.

## Tables, code and links

All 139 original single-backtick InlineCode occurrences are present with exactly the same decoded string, with multiplicity: 82 / 12 / 7 / 11 / 27 by chapter. Authored counts are 82 / 12 / 12 / 11 / 39; additions mark existing declaration/formula/table text as code. details.py/details.json preserve the comparison. This lexical equality is supplemental to the contextual prose review; it is not a proof of the whole document's semantics.

05 has both original fenced `text` blocks represented as RawCode with `some "text"`. Their decoded UTF-8 bytes, whitespace and final LF match the Markdown contents exactly:

- EBNF: 251 bytes, SHA-256 `b8c2eb12c18379cf28091438d654577c1c179778f45785337250e95d378b76a9`.
- Prefix example: 156 bytes, SHA-256 `364c8ec981b98fca95e7d6aa32cd5016b93fd0cf93d92d0ebce94e5da603136f`.

The other four originals have no fenced code blocks. Ruby syntax visible inside the quoted code examples remains code data and is not to be added as display annotations that would alter the code bytes.

09's NDF table retains all 12 rows (tags 0–11), the columns and explicit numeric right alignment. Its Transform table retains all three rows and the Complete / Failed-or-Invalid / Stopped distinctions, including partial presence. Cell explanation Kanji still need the Ruby correction. Other reviewed chapters contain no original Markdown tables requiring a table conversion.

All three original Markdown link target strings are preserved: 05 and 06 use `17-math-html.md`; 09 uses `../progress/foundation-runtime.md`. They are explicit Relative targets with no fabricated fragment. Displayed labels remain 17章 / 実装記録 (and require Kanji Ruby). Preserving lexical targets is not a proof that registration/public routes resolve them: the authored files live under migration/authored, so the host must use the intended source registration/base or an explicitly reviewed mapping. Root's unresolved-link runtime reports remain separate; this review does not claim those links currently resolve or that old GitHub/Pages URLs are migrated.

## Actual grammar and limits of evidence

The fixed languages/doc/syntax.neplg was read directly: SentenceLiteral is registered at Sentence/Flow, Text and InlineCode take builtin Text, Ruby takes two Inline operands, Anno takes an Inline and a list of Inline notes, Concat takes an Inline list, table cells take Sentence, list items take Body, Link takes LinkTarget plus Inline label, RawCode takes OptionalText plus Text, and Doc guest uses `foreign Doc Article`. The manuscript choices inspected above agree with those category distinctions. design/forms.json's Doc categories are retained in details.json for reproducibility.

This task did not execute native/WASI parser or HTML acceptance on the five full manuscripts. Root's 05 Allocation stop, investigation-only raised Nodes limit for 07/08, and 06/09 unresolved routes are neither independent passes nor reasons to discard content. No resource limit was changed here. Structural audit, content review, actual parser/render acceptance, source-of-truth migration and the separate required human meaning review remain distinct. No task was marked complete and no human acceptance was inferred.

## Reproduction

Run `python lexical.py` and `python details.py` from this evidence folder (scripts resolve the fixed snapshot by their own path). sources.json identifies exact Git provenance. verify.py rechecks all fixed bytes against both recorded SHA-256 and the same commit's Git blobs, then writes verification.json and a manifest of the review evidence. The scripts only read the repository and write under this .tmp evidence directory.
