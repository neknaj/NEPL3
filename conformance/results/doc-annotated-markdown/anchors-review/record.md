# Independent chapter 13 legacy-anchor mapping review

PASS, no blocking mapping issue. Scope is the manually supplied eight-row correspondence, not implementation of aliases, a new Markdown projection or canonical migration acceptance. No production, mapping or manuscript edits were made. The author's original nine payloads and manifest remain byte unchanged.

Fixed source commit: 2b05110490238656d088c33bbad601784777d657. Direct Git blobs match canonical.md (14,922 bytes, SHA 81d032e6f04730fd4c52a15c11464bff0ed879c1030a059be19d10b4d8c9a16d) and draft.nepld (30,905 bytes, SHA 31486ad75e0776d55cc12b2a9b5af2840780ad7812456d9c222a90fd8ab495a8). Author manifest SHA 1a391a5580db30fbbc7433b2e00e631cf50002905d4d53bc307dc63b5c635c88 was checked against all payload hashes and lengths. The full original HTML, correspondence, source and author rationale are preserved in author/.

Read the original headings and section bodies against the corresponding authored sections, rather than assigning targets from a slug algorithm. Independently parsed the complete outer Doc structure with the fixed repository structural auditor and forms. This confirms actual ownership, not indentation alone: semantic and execution are children of digest; all other Sections are children of Article. The title is Article.title with no invented Section ID. All eight title literal base strings equal their original Markdown headings; existing Ruby does not alter the base text. The table follows the full original heading order and levels.

| Target | Parent | Original public fragment |
| --- | --- | --- |
| Article title (no Section ID) | none | `13-再現性schema識別契約の判定` |
| `policy` | Article | `方針` |
| `digest` | Article | `1-digest` |
| `semantic` | `digest` | `package意味正規形` |
| `execution` | `digest` | `具体実行identity` |
| `wire` | Article | `2-native値とwire値` |
| `artifacts` | Article | `3-normal-formとartifact` |
| `limits` | Article | `4-limitsとproviderの互換` |

The target meanings are respectively chapter-wide contract; meaning versus implementation and hash cycles; content/schema/package digest; package semantic normal form; concrete execution/provenance identity; native/wire identity and ownership; language/HTML/XML output normalization; sufficient-budget compatibility and deterministic same-implementation behavior. No target crosses into a different meaning subsection. No code, external links, source paragraphs or annotations were rewritten in this task.

Used an independent HTMLParser state machine on the preserved server HTML, not the author's regex/helper output. Every actual heading-element text/level and following permalink href/id matches the manually transcribed row. Also independently fetched the exact fixed-commit GitHub URL over HTTPS: status 200, same final URL, 300,711 bytes, SHA ff8c864e79764a262dc7068adc0696c9936c660a387bdc591369e040a6a15555. All eight fresh heading/anchor rows equal the preserved page's rows despite non-content HTML bytes differing. Source and fresh retrieval metadata are retained; there is no assumption that whole GitHub page chrome is immutable.

Official reference checked: [GitHub section links](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links). Its case/spacing/punctuation rules and repeated-heading warning are consistent with these observed values. [Custom anchors](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#custom-anchors) are separately documented and omitted from the automatic outline. Fixed rendered source: [GitHub chapter 13 at the reviewed commit](https://github.com/neknaj/NEPL3/blob/2b05110490238656d088c33bbad601784777d657/doc/spec/13-reproducibility.md).

The public fragment is the href suffix, not the DOM implementation prefix user-content-. The eight old fragments and seven semantic IDs are individually unique and disjoint on this page. This does not establish that future annotated headings, aliases and other explicit output IDs cannot collide: the complete new emitted anchor set still needs validation. The mapping must remain bound to the reviewed document/page identity and full Unicode spelling. It does not authorize arbitrary heading guesses, route aliases or attaching the Article title alias to policy. No browser click/scroll test was run; HTTP retrieval excludes URL fragments. No Rust parse/lower, projection, portable boundary, accessibility, deployment or canonical-switch claim is made. The Python structural check is explicitly narrower than runtime conformance.

Command: python .tmp/review-chapter13-anchors/check.py, exit 0. No checker failure. The independent fresh request and direct official documentation lookup supplement, rather than replace, the frozen source/HTML evidence.
