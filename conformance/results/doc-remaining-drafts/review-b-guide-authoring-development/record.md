# Independent authoring/development guide review

Fixed drafts: authoring 98eb0c23fb10459d35c180bfe36746e25fcba571 and development 446000e65619bf6ddad5032094bd747ddf330d8f. Both originals were read directly from main 90f4552d9d8de662bada4d76c688c8cf30afa595. SHA-256 values are respectively 0659615a44794990f1c6356a660516e4940f8e8efe272903aff349907deeae08 and 9fd123a202fb3700af89aaaa46a6ef34b17a38312b1f10492ee4bfab8322ece2. Production and manuscripts were not edited.

No new manuscript correction was identified. The known stale portable-status paragraph in development remains a source-level hold, described below; this review does not approve it as a statement of current implementation status.

## Authoring guide

Direct full reading against the original confirms all rules and exceptions: default sentence literals for Text/Ruby/Anno, explicit construction for additional Inline kinds and Break, editing/teaching exceptions, no interpretation of annotation syntax inside ordinary text strings, Sentence versus Inline category, intentional Sentence boundaries, per-language parallel correspondence, mixed variant construction, Kanji-only Japanese Ruby and whole-phrase Anno, preservation of source text/CR/LF and explicit Break, and the separation between structural and actual execution evidence.

Independent structural checks found 65 Sentences in 17 Paragraphs, six Sections, 20 InlineCode values and six links. All paragraph base text equals the original after removing only Markdown inline formatting; sentence splits are retained in the typed structure. All four RawCode bodies and their text language hints are byte-exact. Three examples pass the fixed formal Doc/Sentence constructor/category audit; the parallel example passes Doc/Flow and contains per-language Sentences. It is not itself a Sentence. All 20 inline code values, including escaped quote/backslash/annotation examples, are exact and ordered.

The same-page link is represented as an empty relative path plus the explicit fragment 明示的な改行はbreak; that exact Unicode Section ID occurs once. Reconstructing the URI from path/fragment yields the original target. The existing design audit accepts ASCII identifiers only, so the review subclass explicitly admits this one directly inspected spelling without changing the frozen audit source. This establishes the content/structure correspondence, not general Unicode XID or production parser success. Actual page-registry and browser anchor resolution are unverified.

All 345 Ruby occurrences have Kanji-only bases and nonempty readings, and no narrative Kanji omission was detected. The complete annotated prose and prefix labels were directly read for context and okurigana. No artificial Anno count was required; code examples are preserved as code.

## Development guide

Direct full reading covered all five sections: portable execution scope; local toolchain/MSRV and UTF-8/source preservation; commands, allocation probes and their narrow unsafe/physical-memory scope; schema/bootstrap generation; native/WASI target distinctions; task versus group evidence; byte-exact identity inputs; page-set candidate generation; distribution and branch/CI protections; finite Pages recovery and unfinished deployment; and independent review/inventory duties.

The independent check found 175 Sentences in 47 Paragraphs. Every paragraph's base text exactly matches the original after joining source line wraps and removing inline Markdown formatting. All 55 inline code values, 19 link targets/order, and seven RawCode bodies/language hints are exact. This includes shell, PowerShell and JSON examples with their original newlines and bytes; no commands were executed as part of this content review. All 913 Ruby occurrences and narrative annotation coverage passed the static check and the full annotated text was read.

The inherited paragraph in CIと配布 beginning WASI・ブラウザWasm・LSP・operation providerは目標仕様です remains pending correction of the Markdown source. Earlier paragraphs already describe actual WASI/Pulley execution and browser compilation. Its exact preservation is confirmed, but the outdated blanket status must not be counted as an independently approved current-state claim. Root explicitly requested retaining the draft unchanged until the source correction is available. The document's other dated runtime/CI descriptions are likewise preserved source statements, not freshly rerun evidence.

## Verification limits

check.log/results.json and the manifest record independently executed static comparisons, fixed source hashes, RawCode equality, and structural audits. The helper preserves actual Sentence boundaries and other constructors while projecting Ruby to base for content comparison; this is not full annotated semantic identity. No production parser/lower/HTML, native/WASI, capacity, deployment, canonical switch or separate human migration review was executed. These documents remain drafts. Initial scratch assumptions about ASCII-only names and relative link fragments were corrected explicitly before the successful run and are not production findings.
